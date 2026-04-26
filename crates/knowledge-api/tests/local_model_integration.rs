//! 本地嵌入模型集成测试
//!
//! 测试本地模型（GEMMA 4.0 E4B via Candle）的完整调用链路：
//! - [`HashEmbedding`] 快速验证
//! - [`CandleModelLoader`] 模型加载
//! - [`GemmaEmbedding`] 嵌入推理
//!
//! # 运行方式
//!
//! ```bash
//! cargo test -p knowledge-api --test local_model_integration test_hash
//! cargo test -p knowledge-api --test local_model_integration
//! ```

use knowledge_api::{
    CandleModelLoader, DeviceType, EmbeddingConfig, EmbeddingError, EmbeddingModelTrait,
    EmbeddingModelType, GemmaEmbedding, HashEmbedding, LoaderConfig, ModelBackend, ModelLoader,
    PoolingStrategy, Quantization,
};

fn model_cache_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join("models")
}

fn gemma_model_path() -> std::path::PathBuf {
    model_cache_dir().join("google_gemma-4-e4b-it")
}

fn gemma_model_available() -> bool {
    let path = gemma_model_path();
    path.join("config.json").exists()
        && path.join("model.safetensors").exists()
        && path.join("tokenizer.json").exists()
}

fn default_loader_config() -> LoaderConfig {
    LoaderConfig {
        model_id: "google/gemma-4-e4b-it".to_string(),
        backend: ModelBackend::Candle,
        device: DeviceType::Cpu,
        quantization: Quantization::Fp16,
        max_seq_length: 128,
        batch_size: 1,
        cache_dir: Some(model_cache_dir()),
        use_flash_attention: false,
        num_threads: 2,
        extra_params: std::collections::HashMap::new(),
    }
}

// ========== HashEmbedding 测试 ==========

#[test]
fn test_hash_embedding_basic_inference() {
    let hash = HashEmbedding::with_dimension(256);
    let result = EmbeddingModelTrait::embed(&hash, "Hello, world!").expect("Hash嵌入应成功");

    assert_eq!(result.vector.len(), 256, "嵌入维度应为256");
    assert!(result.inference_time_ms > 0.0, "推理时间应大于0");
    assert!(result.token_count > 0, "token数量应大于0");

    let norm: f32 = result.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.01,
        "L2归一化后范数应接近1.0，实际为 {norm}"
    );
}

#[test]
fn test_hash_embedding_empty_input_returns_error() {
    let hash = HashEmbedding::with_dimension(128);
    let result = EmbeddingModelTrait::embed(&hash, "");
    assert!(matches!(result, Err(EmbeddingError::EmptyInput)));

    let result = EmbeddingModelTrait::embed(&hash, "   ");
    assert!(matches!(result, Err(EmbeddingError::EmptyInput)));
}

#[test]
fn test_hash_embedding_deterministic() {
    let hash = HashEmbedding::with_dimension(256);
    let r1 = EmbeddingModelTrait::embed(&hash, "deterministic test").expect("嵌入应成功");
    let r2 = EmbeddingModelTrait::embed(&hash, "deterministic test").expect("嵌入应成功");

    for (a, b) in r1.vector.iter().zip(r2.vector.iter()) {
        assert!((a - b).abs() < f32::EPSILON, "相同输入应产生相同嵌入向量");
    }
}

#[test]
fn test_hash_embedding_different_inputs_differ() {
    let hash = HashEmbedding::with_dimension(256);
    let r1 = EmbeddingModelTrait::embed(&hash, "apple").expect("嵌入应成功");
    let r2 = EmbeddingModelTrait::embed(&hash, "orange").expect("嵌入应成功");

    let diff_count = r1
        .vector
        .iter()
        .zip(r2.vector.iter())
        .filter(|&(&a, &b)| (a - b).abs() > f32::EPSILON)
        .count();
    assert!(diff_count > 0, "不同输入应产生不同嵌入向量");
}

#[test]
fn test_hash_embedding_multiple_dimensions() {
    for dim in [64, 128, 256, 512, 768, 1024] {
        let hash = HashEmbedding::with_dimension(dim);
        let result = EmbeddingModelTrait::embed(&hash, "multi-dim test").expect("嵌入应成功");
        assert_eq!(result.vector.len(), dim, "维度{dim}嵌入长度不匹配");
    }
}

#[test]
fn test_hash_embedding_model_name() {
    let hash = HashEmbedding::with_dimension(128);
    assert_eq!(EmbeddingModelTrait::model_name(&hash), "hash");
    assert_eq!(EmbeddingModelTrait::name(&hash), "hash-embedding");
    assert_eq!(EmbeddingModelTrait::embedding_dim(&hash), 128);
}

#[test]
fn test_hash_embedding_initialize() {
    let mut hash = HashEmbedding::with_dimension(128);
    assert!(!EmbeddingModelTrait::is_initialized(&hash));
    EmbeddingModelTrait::initialize(&mut hash).expect("初始化应成功");
    assert!(EmbeddingModelTrait::is_initialized(&hash));
}

// ========== EmbeddingConfig 测试 ==========

#[test]
fn test_embedding_config_default() {
    let config = EmbeddingConfig::default();
    assert_eq!(config.model_type, EmbeddingModelType::Gemma4E4b);
    assert_eq!(config.embedding_dim, 2560);
    assert_eq!(config.max_seq_length, 8192);
    assert!(config.use_gpu);
    assert_eq!(config.model_id.as_deref(), Some("google/gemma-4-e4b-it"));
    assert_eq!(config.batch_size, 32);
}

#[test]
fn test_embedding_model_type_parsing() {
    use std::str::FromStr;

    assert_eq!(
        EmbeddingModelType::from_str("hash").unwrap(),
        EmbeddingModelType::Hash
    );
    assert_eq!(
        EmbeddingModelType::from_str("gemma-4-e4b").unwrap(),
        EmbeddingModelType::Gemma4E4b
    );
    assert_eq!(
        EmbeddingModelType::from_str("gemma4e4b").unwrap(),
        EmbeddingModelType::Gemma4E4b
    );
    assert_eq!(
        EmbeddingModelType::from_str("openai-ada-002").unwrap(),
        EmbeddingModelType::OpenAIAda002
    );
    assert!(EmbeddingModelType::from_str("unknown-model").is_err());
}

// ========== CandleModelLoader 测试（需要模型文件）==========

#[tokio::test]
async fn test_candle_loader_model_detection() {
    let loader = CandleModelLoader::new(LoaderConfig::default());

    assert!(loader.supports_model("google/gemma-4-e4b-it").unwrap());
    assert!(loader.supports_model("meta-llama/Llama-3-8B").unwrap());
    assert!(loader.supports_model("Qwen/Qwen2-7B").unwrap());
    assert!(loader.supports_model("mistralai/Mistral-7B").unwrap());
    assert!(loader.supports_model("microsoft/phi-3-mini").unwrap());
    assert!(loader.supports_model("tiiie/falcon-7b").unwrap());
    assert!(loader.supports_model("unknown/architecture").is_err());
}

#[tokio::test]
async fn test_candle_loader_name_and_backend() {
    let loader = CandleModelLoader::new(LoaderConfig::default());
    assert_eq!(loader.name(), "candle-loader");
    assert_eq!(loader.supported_backend(), ModelBackend::Candle);
}

#[tokio::test]
async fn test_candle_loader_load_and_infer() {
    if !gemma_model_available() {
        eprintln!(
            "跳过: GEMMA模型文件未找到，请先下载模型到 {}",
            gemma_model_path().display()
        );
        return;
    }

    let loader = CandleModelLoader::new(default_loader_config());
    let model = loader.load().await.expect("模型加载应成功");

    let info = model.info();
    assert_eq!(info.architecture, "gemma4");
    assert!(info.embedding_dim > 0);
    assert!(info.vocab_size > 0);

    let result = model.infer("Hello, world!").await.expect("推理应成功");
    assert!(!result.output.is_empty(), "输出向量不应为空");
    assert!(result.token_count > 0, "token数量应大于0");
    assert!(result.inference_time_ms > 0.0, "推理时间应大于0");

    let norm: f32 = result.output.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.01,
        "L2归一化后范数应接近1.0，实际为 {norm}"
    );
}

#[tokio::test]
async fn test_candle_loader_embed_eos_pooling() {
    if !gemma_model_available() {
        eprintln!("跳过: GEMMA模型文件未找到");
        return;
    }

    let loader = CandleModelLoader::new(default_loader_config());
    let model = loader.load().await.expect("模型加载应成功");

    let embedding = model
        .embed("测试中文嵌入", PoolingStrategy::Eos)
        .await
        .expect("EOS池化嵌入应成功");

    assert_eq!(
        embedding.len(),
        model.info().embedding_dim,
        "嵌入维度应匹配"
    );
}

#[tokio::test]
async fn test_candle_loader_encode_decode() {
    if !gemma_model_available() {
        eprintln!("跳过: GEMMA模型文件未找到");
        return;
    }

    let loader = CandleModelLoader::new(default_loader_config());
    let model = loader.load().await.expect("模型加载应成功");

    let token_ids = model.encode("Hello").await.expect("编码应成功");
    assert!(!token_ids.is_empty(), "编码结果不应为空");

    let decoded = model.decode(&token_ids).await.expect("解码应成功");
    assert!(!decoded.is_empty(), "解码结果不应为空");
}

// ========== GemmaEmbedding 测试（需要模型文件）==========

#[test]
fn test_gemma_embedding_creation() {
    let config = EmbeddingConfig {
        model_type: EmbeddingModelType::Gemma4E4b,
        embedding_dim: 2560,
        max_seq_length: 512,
        use_gpu: false,
        model_id: Some("google/gemma-4-e4b-it".to_string()),
        cache_dir: None,
        batch_size: 1,
        extra_params: std::collections::HashMap::new(),
    };

    let gemma = GemmaEmbedding::new(config).expect("创建GemmaEmbedding应成功");
    assert!(!gemma.is_loaded());
    assert_eq!(EmbeddingModelTrait::embedding_dim(&gemma), 2560);
    assert_eq!(EmbeddingModelTrait::model_name(&gemma), "gemma-4-e4b");
}

#[test]
fn test_gemma_embedding_not_loaded_error() {
    let config = EmbeddingConfig {
        model_type: EmbeddingModelType::Gemma4E4b,
        embedding_dim: 2560,
        max_seq_length: 512,
        use_gpu: false,
        model_id: Some("google/gemma-4-e4b-it".to_string()),
        cache_dir: None,
        batch_size: 1,
        extra_params: std::collections::HashMap::new(),
    };

    let gemma = GemmaEmbedding::new(config).expect("创建应成功");
    let result = EmbeddingModelTrait::embed(&gemma, "test");
    assert!(matches!(result, Err(EmbeddingError::ModelNotLoaded(_))));
}

#[tokio::test]
async fn test_gemma_embedding_load_and_embed() {
    if !gemma_model_available() {
        eprintln!(
            "跳过: GEMMA模型文件未找到，请先下载模型到 {}",
            gemma_model_path().display()
        );
        return;
    }

    let config = EmbeddingConfig {
        model_type: EmbeddingModelType::Gemma4E4b,
        embedding_dim: 2560,
        max_seq_length: 128,
        use_gpu: false,
        model_id: Some("google/gemma-4-e4b-it".to_string()),
        cache_dir: Some(model_cache_dir().to_string_lossy().to_string()),
        batch_size: 1,
        extra_params: std::collections::HashMap::new(),
    };

    let mut gemma = GemmaEmbedding::new(config).expect("创建应成功");
    assert!(!gemma.is_loaded());

    gemma.load_model().await.expect("模型加载应成功");
    assert!(gemma.is_loaded());

    let result =
        tokio::task::block_in_place(|| EmbeddingModelTrait::embed(&gemma, "Hello, world!"))
            .expect("嵌入推理应成功");

    assert_eq!(result.vector.len(), 2560, "嵌入维度应为2560");
    assert!(result.inference_time_ms > 0.0, "推理时间应大于0");

    let norm: f32 = result.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 0.01,
        "L2归一化后范数应接近1.0，实际为 {norm}"
    );
}

#[tokio::test]
async fn test_gemma_embedding_chinese_text() {
    if !gemma_model_available() {
        eprintln!("跳过: GEMMA模型文件未找到");
        return;
    }

    let config = EmbeddingConfig {
        model_type: EmbeddingModelType::Gemma4E4b,
        embedding_dim: 2560,
        max_seq_length: 128,
        use_gpu: false,
        model_id: Some("google/gemma-4-e4b-it".to_string()),
        cache_dir: Some(gemma_model_path().to_string_lossy().to_string()),
        batch_size: 1,
        extra_params: std::collections::HashMap::new(),
    };

    let mut gemma = GemmaEmbedding::new(config).expect("创建应成功");
    gemma.load_model().await.expect("模型加载应成功");

    let result =
        tokio::task::block_in_place(|| EmbeddingModelTrait::embed(&gemma, "这是一个中文测试句子"))
            .expect("中文嵌入应成功");

    assert_eq!(result.vector.len(), 2560);

    let norm: f32 = result.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 0.01, "中文嵌入也应L2归一化");
}

#[tokio::test]
async fn test_gemma_embedding_semantic_similarity() {
    if !gemma_model_available() {
        eprintln!("跳过: GEMMA模型文件未找到");
        return;
    }

    let config = EmbeddingConfig {
        model_type: EmbeddingModelType::Gemma4E4b,
        embedding_dim: 2560,
        max_seq_length: 128,
        use_gpu: false,
        model_id: Some("google/gemma-4-e4b-it".to_string()),
        cache_dir: Some(gemma_model_path().to_string_lossy().to_string()),
        batch_size: 1,
        extra_params: std::collections::HashMap::new(),
    };

    let mut gemma = GemmaEmbedding::new(config).expect("创建应成功");
    gemma.load_model().await.expect("模型加载应成功");

    let r_similar = tokio::task::block_in_place(|| {
        let a = EmbeddingModelTrait::embed(&gemma, "The cat sat on the mat")?;
        let b = EmbeddingModelTrait::embed(&gemma, "A kitten was sitting on a rug")?;
        Ok::<_, EmbeddingError>((a, b))
    })
    .expect("相似句子嵌入应成功");

    let r_different = tokio::task::block_in_place(|| {
        let a = EmbeddingModelTrait::embed(&gemma, "The cat sat on the mat")?;
        let b = EmbeddingModelTrait::embed(&gemma, "Quantum physics explains subatomic particles")?;
        Ok::<_, EmbeddingError>((a, b))
    })
    .expect("不同句子嵌入应成功");

    let sim_similar = cosine_similarity(&r_similar.0.vector, &r_similar.1.vector);
    let sim_different = cosine_similarity(&r_different.0.vector, &r_different.1.vector);

    assert!(
        sim_similar > sim_different,
        "语义相似句子的余弦相似度({sim_similar:.4})应大于语义不同句子({sim_different:.4})"
    );
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len(), "向量维度必须相同");
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}
