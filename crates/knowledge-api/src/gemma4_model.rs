#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use candle_core::{D, Module, Result, Tensor};
use candle_nn::{VarBuilder, rms_norm};

const fn default_rms_norm_eps() -> f64 {
    1e-6
}

const fn default_num_attention_heads() -> usize {
    8
}

const fn default_num_key_value_heads() -> usize {
    4
}

const fn default_head_dim() -> usize {
    256
}

const fn default_global_head_dim() -> usize {
    512
}

const fn default_rope_theta() -> f64 {
    1_000_000.0
}

const fn default_vocab_size() -> usize {
    262_144
}

const fn default_max_position_embeddings() -> usize {
    131_072
}

const fn default_sliding_window() -> usize {
    512
}

const fn default_sliding_window_pattern() -> usize {
    6
}

const fn default_intermediate_size() -> usize {
    10240
}

const fn default_per_layer_input_dim() -> usize {
    256
}

const fn default_partial_rotary_factor() -> f64 {
    0.25
}

const fn default_num_kv_shared_layers() -> usize {
    18
}

/// Gemma4 文本模型的超参数配置，控制模型架构各维度与行为。
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Gemma4TextConfig {
    /// 隐藏层维度大小，即模型嵌入向量的维度。
    pub hidden_size: usize,
    /// 查询（Query）的注意力头数。
    #[serde(default = "default_num_attention_heads")]
    pub num_attention_heads: usize,
    /// Transformer 解码器层的总数。
    pub num_hidden_layers: usize,
    /// 键（Key）和值（Value）的注意力头数，用于分组查询注意力（GQA）。
    #[serde(default = "default_num_key_value_heads")]
    pub num_key_value_heads: usize,
    /// 滑动注意力层中每个注意力头的维度。
    #[serde(default = "default_head_dim")]
    pub head_dim: usize,
    /// 全局注意力层中每个注意力头的维度。
    #[serde(default = "default_global_head_dim")]
    pub global_head_dim: usize,
    /// 前馈网络（FFN）中间层的维度大小。
    #[serde(default = "default_intermediate_size")]
    pub intermediate_size: usize,
    /// `RMSNorm` 归一化中防止除零的微小常数。
    #[serde(default = "default_rms_norm_eps")]
    pub rms_norm_eps: f64,
    /// 旋转位置编码（RoPE）的基准频率 θ。
    #[serde(default = "default_rope_theta")]
    pub rope_theta: f64,
    /// 词表大小，即嵌入矩阵的行数。
    #[serde(default = "default_vocab_size")]
    pub vocab_size: usize,
    /// 模型支持的最大序列位置长度。
    #[serde(default = "default_max_position_embeddings")]
    pub max_position_embeddings: usize,
    /// 滑动窗口注意力的窗口大小。
    #[serde(default = "default_sliding_window")]
    pub sliding_window: usize,
    /// 滑动窗口注意力层的重复周期模式。
    #[serde(default = "default_sliding_window_pattern")]
    pub sliding_window_pattern: usize,
    /// 逐层输入（per-layer input）投影的维度大小。
    #[serde(default = "default_per_layer_input_dim")]
    pub per_layer_input_dim: usize,
    /// 部分旋转位置编码的因子，控制应用 `RoPE` 的维度比例。
    #[serde(default = "default_partial_rotary_factor")]
    pub partial_rotary_factor: f64,
    /// 共享键值（KV）权重的层数。
    #[serde(default = "default_num_kv_shared_layers")]
    pub num_kv_shared_layers: usize,
    /// 每层注意力类型标识列表，如 `"sliding_attention"` 或 `"full_attention"`。
    pub layer_types: Vec<String>,
    /// 注意力投影层是否使用偏置项。
    #[serde(default)]
    pub attention_bias: bool,
    /// 最终 logits 的软截断值，若为 `Some(val)` 则对输出 logits 做缩放-tanh-缩放处理。
    pub final_logit_softcapping: Option<f64>,
}

impl Gemma4TextConfig {
    /// 从 JSON 值解析 Gemma4 文本模型配置。
    ///
    /// # Errors
    ///
    /// 当 JSON 值不符合 `Gemma4TextConfig` 的反序列化契约时返回错误。
    pub fn from_json(value: &serde_json::Value) -> Result<Self> {
        serde_json::from_value(value.clone())
            .map_err(|e| candle_core::Error::Msg(format!("Gemma4配置解析失败: {e}")))
    }

    fn is_sliding(&self, layer_idx: usize) -> bool {
        self.layer_types
            .get(layer_idx)
            .is_some_and(|s| s == "sliding_attention")
    }
}

#[derive(Debug, Clone)]
enum LayerType {
    SlidingAttention,
    FullAttention,
}

#[derive(Debug)]
struct Gemma4Attention {
    q_proj: candle_nn::Linear,
    k_proj: candle_nn::Linear,
    v_proj: candle_nn::Linear,
    o_proj: candle_nn::Linear,
    q_norm: candle_nn::RmsNorm,
    k_norm: candle_nn::RmsNorm,
    num_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
    layer_type: LayerType,
    sliding_window: usize,
}

impl Gemma4Attention {
    #[allow(clippy::needless_pass_by_value)]
    fn new(
        prefix: &str,
        config: &Gemma4TextConfig,
        vb: VarBuilder,
        layer_idx: usize,
    ) -> Result<Self> {
        let is_sliding = config.is_sliding(layer_idx);
        let layer_type = if is_sliding {
            LayerType::SlidingAttention
        } else {
            LayerType::FullAttention
        };

        let (head_dim, num_kv_heads) = if is_sliding {
            (config.head_dim, config.num_key_value_heads)
        } else {
            (config.global_head_dim, config.num_key_value_heads)
        };

        let hidden_sz = config.hidden_size;
        let num_heads = config.num_attention_heads;
        let bias = config.attention_bias;

        let q_proj = candle_nn::linear_b(
            hidden_sz,
            num_heads * head_dim,
            bias,
            vb.pp(format!("{prefix}.q_proj")),
        )?;
        let k_proj = candle_nn::linear_b(
            hidden_sz,
            num_kv_heads * head_dim,
            bias,
            vb.pp(format!("{prefix}.k_proj")),
        )?;
        let v_proj = candle_nn::linear_b(
            hidden_sz,
            num_kv_heads * head_dim,
            bias,
            vb.pp(format!("{prefix}.v_proj")),
        )?;
        let o_proj = candle_nn::linear_b(
            num_heads * head_dim,
            hidden_sz,
            bias,
            vb.pp(format!("{prefix}.o_proj")),
        )?;

        let q_norm = rms_norm(
            head_dim,
            config.rms_norm_eps,
            vb.pp(format!("{prefix}.q_norm")),
        )?;
        let k_norm = rms_norm(
            head_dim,
            config.rms_norm_eps,
            vb.pp(format!("{prefix}.k_norm")),
        )?;

        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            q_norm,
            k_norm,
            num_heads,
            num_kv_heads,
            head_dim,
            layer_type,
            sliding_window: config.sliding_window,
        })
    }

    fn forward(&self, xs: &Tensor, prefix: &str) -> Result<Tensor> {
        let (bsz, seq_len, _) = xs.dims3()?;
        let q = self.q_proj.forward(xs)?;
        let k = self.k_proj.forward(xs)?;
        let v = self.v_proj.forward(xs)?;

        let q = q
            .reshape((bsz, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = k
            .reshape((bsz, seq_len, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = v
            .reshape((bsz, seq_len, self.num_kv_heads, self.head_dim))?
            .transpose(1, 2)?;

        let q = q.apply(&self.q_norm)?;
        let k = k.apply(&self.k_norm)?;

        let (q, k) = apply_rotary_emb(&q, &k, prefix, &self.layer_type, self.head_dim)?;

        let n_kv_groups = self.num_heads / self.num_kv_heads;
        let k = repeat_kv(k, n_kv_groups)?;
        let v = repeat_kv(v, n_kv_groups)?;

        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let attn_weights = (q.matmul(&k.transpose(2, 3)?)? * scale)?;

        let attn_weights = if matches!(self.layer_type, LayerType::SlidingAttention) {
            let window_size = self.sliding_window;
            apply_sliding_window_mask(&attn_weights, window_size)?
        } else {
            apply_causal_mask(&attn_weights)?
        };

        let attn_output = attn_weights.matmul(&v)?;
        let attn_output =
            attn_output
                .transpose(1, 2)?
                .reshape((bsz, seq_len, self.num_heads * self.head_dim))?;
        self.o_proj.forward(&attn_output)
    }
}

fn repeat_kv(xs: Tensor, n_rep: usize) -> Result<Tensor> {
    if n_rep == 1 {
        return Ok(xs);
    }
    let (bsz, num_kv_heads, seq_len, head_dim) = xs.dims4()?;
    let xs = xs
        .unsqueeze(2)?
        .expand((bsz, num_kv_heads, n_rep, seq_len, head_dim))?
        .reshape((bsz, num_kv_heads * n_rep, seq_len, head_dim))?;
    Ok(xs)
}

fn apply_rotary_emb(
    q: &Tensor,
    k: &Tensor,
    _prefix: &str,
    layer_type: &LayerType,
    head_dim: usize,
) -> Result<(Tensor, Tensor)> {
    let (_bsz, _num_heads, seq_len, _head_dim) = q.dims4()?;
    let rope_theta = match layer_type {
        LayerType::SlidingAttention => 10_000.0_f64,
        LayerType::FullAttention => 1_000_000.0_f64,
    };

    let partial_rotary_factor = match layer_type {
        LayerType::SlidingAttention => 1.0_f64,
        LayerType::FullAttention => 0.25_f64,
    };

    let rotary_dim = (head_dim as f64 * partial_rotary_factor) as usize;
    let half_dim = head_dim / 2;

    let inv_freq: Vec<f64> = (0..half_dim)
        .map(|i| 1.0 / rope_theta.powf(2.0 * i as f64 / head_dim as f64))
        .collect();
    let inv_freq = Tensor::from_vec(inv_freq, (1, half_dim), q.device())?;

    let t = Tensor::arange(0u32, seq_len as u32, q.device())?
        .to_dtype(q.dtype())?
        .reshape((seq_len, 1))?;
    let freqs = t.matmul(&inv_freq)?;
    let cos = freqs.cos()?;
    let sin = freqs.sin()?;

    let q_rot = apply_rotary_emb_single(q, &cos, &sin, rotary_dim)?;
    let k_rot = apply_rotary_emb_single(k, &cos, &sin, rotary_dim)?;
    Ok((q_rot, k_rot))
}

fn apply_rotary_emb_single(
    xs: &Tensor,
    cos: &Tensor,
    sin: &Tensor,
    rotary_dim: usize,
) -> Result<Tensor> {
    let (bsz, num_heads, seq_len, head_dim) = xs.dims4()?;

    let x_rot = xs.narrow(D::Minus1, 0, rotary_dim)?;
    let x_pass = xs.narrow(D::Minus1, rotary_dim, head_dim - rotary_dim)?;

    let x_rot = x_rot.reshape((bsz, num_heads, seq_len, rotary_dim / 2, 2))?;
    let x1 = x_rot.narrow(D::Minus1, 0, 1)?.squeeze(D::Minus1)?;
    let x2 = x_rot.narrow(D::Minus1, 1, 2)?.squeeze(D::Minus1)?;

    let cos =
        cos.unsqueeze(0)?
            .unsqueeze(0)?
            .broadcast_as((bsz, num_heads, seq_len, rotary_dim / 2))?;
    let sin =
        sin.unsqueeze(0)?
            .unsqueeze(0)?
            .broadcast_as((bsz, num_heads, seq_len, rotary_dim / 2))?;

    let r1 = (x1.broadcast_mul(&cos)? - x2.broadcast_mul(&sin)?)?;
    let r2 = (x2.broadcast_mul(&cos)? + x1.broadcast_mul(&sin)?)?;
    let rotated = Tensor::stack(&[&r1, &r2], D::Minus1)?;
    let rotated = rotated.reshape((bsz, num_heads, seq_len, rotary_dim))?;

    if head_dim > rotary_dim {
        Tensor::cat(&[&rotated, &x_pass], D::Minus1)
    } else {
        Ok(rotated)
    }
}

fn apply_causal_mask(attn_weights: &Tensor) -> Result<Tensor> {
    let (_, _, seq_len_q, seq_len_k) = attn_weights.dims4()?;
    if seq_len_q <= 1 {
        return Ok(attn_weights.clone());
    }
    let row_idx = Tensor::arange(0u32, seq_len_q as u32, attn_weights.device())?
        .to_dtype(candle_core::DType::F64)?
        .reshape((seq_len_q, 1))?
        .broadcast_as((seq_len_q, seq_len_k))?;
    let col_idx = Tensor::arange(0u32, seq_len_k as u32, attn_weights.device())?
        .to_dtype(candle_core::DType::F64)?
        .reshape((1, seq_len_k))?
        .broadcast_as((seq_len_q, seq_len_k))?;
    let mask = col_idx
        .le(&row_idx)?
        .to_dtype(attn_weights.dtype())?
        .broadcast_as(attn_weights.shape())?;
    attn_weights.broadcast_add(&(&mask * 1e9)?)
}

fn apply_sliding_window_mask(attn_weights: &Tensor, window_size: usize) -> Result<Tensor> {
    let (_, _, seq_len_q, seq_len_k) = attn_weights.dims4()?;
    if seq_len_q <= 1 {
        return Ok(attn_weights.clone());
    }
    let positions = Tensor::arange(0u32, seq_len_k as u32, attn_weights.device())?
        .to_dtype(candle_core::DType::F64)?;
    let row_indices = Tensor::arange(0u32, seq_len_q as u32, attn_weights.device())?
        .to_dtype(candle_core::DType::F64)?
        .reshape((seq_len_q, 1))?;
    let diff = positions.broadcast_sub(&row_indices)?;
    let mask = diff.gt(0.0_f64)?.to_dtype(candle_core::DType::U8)?;
    let distance = diff.abs()?;
    let window_mask = distance
        .le(window_size as f64)?
        .to_dtype(candle_core::DType::U8)?;
    let combined = (&mask + &window_mask)?;
    let combined = combined
        .eq(&Tensor::zeros(
            (seq_len_q, seq_len_k),
            candle_core::DType::U8,
            attn_weights.device(),
        )?)?
        .to_dtype(attn_weights.dtype())?;
    let combined = combined.broadcast_as(attn_weights.shape())?;
    attn_weights.broadcast_add(&(&combined * 1e9)?)
}

#[derive(Debug)]
#[allow(clippy::struct_field_names)]
struct Gemma4MLP {
    gate_proj: candle_nn::Linear,
    up_proj: candle_nn::Linear,
    down_proj: candle_nn::Linear,
}

impl Gemma4MLP {
    #[allow(clippy::needless_pass_by_value)]
    fn new(prefix: &str, config: &Gemma4TextConfig, vb: VarBuilder) -> Result<Self> {
        let hidden_sz = config.hidden_size;
        let intermediate_sz = config.intermediate_size;
        let bias = config.attention_bias;

        let gate_proj = candle_nn::linear_b(
            hidden_sz,
            intermediate_sz,
            bias,
            vb.pp(format!("{prefix}.gate_proj")),
        )?;
        let up_proj = candle_nn::linear_b(
            hidden_sz,
            intermediate_sz,
            bias,
            vb.pp(format!("{prefix}.up_proj")),
        )?;
        let down_proj = candle_nn::linear_b(
            intermediate_sz,
            hidden_sz,
            bias,
            vb.pp(format!("{prefix}.down_proj")),
        )?;

        Ok(Self {
            gate_proj,
            up_proj,
            down_proj,
        })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let gate = self.gate_proj.forward(xs)?;
        let up = self.up_proj.forward(xs)?;
        let gate = gate.gelu()?;
        self.down_proj.forward(&(gate * up)?)
    }
}

#[derive(Debug)]
struct Gemma4DecoderLayer {
    self_attn: Gemma4Attention,
    mlp: Gemma4MLP,
    input_layernorm: candle_nn::RmsNorm,
    post_attention_layernorm: candle_nn::RmsNorm,
    pre_feedforward_layernorm: candle_nn::RmsNorm,
    post_feedforward_layernorm: candle_nn::RmsNorm,
    post_per_layer_input_norm: candle_nn::RmsNorm,
    per_layer_input_gate: candle_nn::Linear,
    layer_scalar: Tensor,
}

impl Gemma4DecoderLayer {
    #[allow(clippy::needless_pass_by_value)]
    fn new(layer_idx: usize, config: &Gemma4TextConfig, vb: VarBuilder) -> Result<Self> {
        let prefix = format!("model.language_model.layers.{layer_idx}");
        let vb_p = vb.pp(&prefix);

        let self_attn = Gemma4Attention::new("self_attn", config, vb_p.pp("self_attn"), layer_idx)?;
        let mlp = Gemma4MLP::new("mlp", config, vb_p.pp("mlp"))?;

        let input_layernorm = rms_norm(
            config.hidden_size,
            config.rms_norm_eps,
            vb_p.pp("input_layernorm"),
        )?;
        let post_attention_layernorm = rms_norm(
            config.hidden_size,
            config.rms_norm_eps,
            vb_p.pp("post_attention_layernorm"),
        )?;
        let pre_feedforward_layernorm = rms_norm(
            config.hidden_size,
            config.rms_norm_eps,
            vb_p.pp("pre_feedforward_layernorm"),
        )?;
        let post_feedforward_layernorm = rms_norm(
            config.hidden_size,
            config.rms_norm_eps,
            vb_p.pp("post_feedforward_layernorm"),
        )?;
        let post_per_layer_input_norm = rms_norm(
            config.hidden_size,
            config.rms_norm_eps,
            vb_p.pp("post_per_layer_input_norm"),
        )?;

        let per_layer_input_gate = candle_nn::linear_no_bias(
            config.per_layer_input_dim,
            config.hidden_size,
            vb_p.pp("per_layer_input_gate"),
        )?;

        let layer_scalar = vb_p.get(&[1usize], "layer_scalar")?;

        Ok(Self {
            self_attn,
            mlp,
            input_layernorm,
            post_attention_layernorm,
            pre_feedforward_layernorm,
            post_feedforward_layernorm,
            post_per_layer_input_norm,
            per_layer_input_gate,
            layer_scalar,
        })
    }

    fn forward(&self, xs: &Tensor, per_layer_embed: &Tensor) -> Result<Tensor> {
        let normed = xs.apply(&self.input_layernorm)?;
        let attn_output = self.self_attn.forward(&normed, "")?;
        let attn_output = attn_output.apply(&self.post_attention_layernorm)?;
        let xs = (xs + &attn_output)?;

        let normed = xs.apply(&self.pre_feedforward_layernorm)?;
        let mlp_output = self.mlp.forward(&normed)?;
        let mlp_output = mlp_output.apply(&self.post_feedforward_layernorm)?;
        let xs = (xs + &mlp_output)?;

        let gate = self.per_layer_input_gate.forward(per_layer_embed)?;
        let gate = gate.apply(&self.post_per_layer_input_norm)?;
        let gate = candle_nn::ops::sigmoid(&gate)?;
        let layer_scalar = self.layer_scalar.broadcast_as(gate.shape())?;
        let gate_layer = (gate * layer_scalar)?;
        let xs = (xs + &gate_layer)?;

        Ok(xs)
    }
}

#[derive(Debug)]
/// Gemma4 文本模型主体，包含嵌入层、解码器层栈及逐层输入投影。
pub struct Gemma4TextModel {
    embed_tokens: candle_nn::Embedding,
    layers: Vec<Gemma4DecoderLayer>,
    norm: candle_nn::RmsNorm,
    per_layer_projection: candle_nn::Linear,
    per_layer_projection_norm: candle_nn::RmsNorm,
    config: Gemma4TextConfig,
}

impl Gemma4TextModel {
    /// 构造 Gemma4 文本模型。
    ///
    /// # Errors
    ///
    /// 当权重加载失败或层初始化出错时返回错误。
    #[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
    pub fn new(config: &Gemma4TextConfig, vb: VarBuilder) -> Result<Self> {
        let vb = vb.pp("model.language_model");

        let embed_tokens =
            candle_nn::embedding(config.vocab_size, config.hidden_size, vb.pp("embed_tokens"))?;

        let mut layers = Vec::with_capacity(config.num_hidden_layers);
        for layer_idx in 0..config.num_hidden_layers {
            let layer = Gemma4DecoderLayer::new(layer_idx, config, vb.clone())?;
            layers.push(layer);
        }

        let per_layer_projection = candle_nn::linear_no_bias(
            config.per_layer_input_dim,
            config.hidden_size,
            vb.pp("per_layer_projection"),
        )?;
        let per_layer_projection_norm = rms_norm(
            config.per_layer_input_dim,
            config.rms_norm_eps,
            vb.pp("per_layer_projection_norm"),
        )?;

        let norm = rms_norm(config.hidden_size, config.rms_norm_eps, vb.pp("norm"))?;

        Ok(Self {
            embed_tokens,
            layers,
            norm,
            per_layer_projection,
            per_layer_projection_norm,
            config: config.clone(),
        })
    }

    /// 执行 Gemma4 文本模型前向推理。
    ///
    /// # Errors
    ///
    /// 当张量运算（嵌入查找、层前向、归一化等）失败时返回错误。
    pub fn forward(&self, input_ids: &Tensor) -> Result<Tensor> {
        let (_bsz, _seq_len) = input_ids.dims2()?;
        let xs = self.embed_tokens.forward(input_ids)?;

        let per_layer_embeds_raw = self.per_layer_projection_norm.forward(&xs)?;
        let (_bsz_e, _seq_len_e, _total_dim) = per_layer_embeds_raw.dims3()?;
        let per_layer_embed = self.per_layer_projection.forward(&per_layer_embeds_raw)?;

        let mut hidden_state = xs;
        for layer in &self.layers {
            hidden_state = layer.forward(&hidden_state, &per_layer_embed)?;
        }

        hidden_state = hidden_state.apply(&self.norm)?;

        if let Some(softcap) = self.config.final_logit_softcapping {
            hidden_state = hidden_state.broadcast_div(
                &Tensor::new(softcap, hidden_state.device())?.to_dtype(hidden_state.dtype())?,
            )?;
            hidden_state = hidden_state.tanh()?;
            hidden_state = hidden_state.broadcast_mul(
                &Tensor::new(softcap, hidden_state.device())?.to_dtype(hidden_state.dtype())?,
            )?;
        }

        Ok(hidden_state)
    }
}
