use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use knowledge_api::embedding_service::{EmbeddingModel, EmbeddingService};
use knowledge_core::math::cosine_similarity;
use std::hint::black_box;

const EMBEDDING_DIM: usize = 1536;

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn blake3_to_u64(data: &[u8]) -> u64 {
    let hash = blake3::hash(data);
    u64::from_le_bytes(
        hash.as_bytes()[..8]
            .try_into()
            .expect("blake3 输出至少 8 字节"),
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn simhash_embedding(text: &str, dimension: usize) -> Vec<f32> {
    let mut v = vec![0.0f64; dimension];

    let tokens: Vec<&str> = text
        .split_whitespace()
        .chain(text.split(|c: char| !c.is_alphanumeric()))
        .filter(|t| !t.is_empty())
        .collect();

    if tokens.is_empty() {
        v[0] = 1.0;
        let norm = v
            .iter()
            .map(|x| x * x)
            .sum::<f64>()
            .sqrt()
            .max(f64::EPSILON);
        return v.iter().map(|x| (x / norm) as f32).collect();
    }

    for token in &tokens {
        let h1 = blake3_to_u64(token.as_bytes());
        let h2 = blake3_to_u64(format!("{token}:salt2").as_bytes());

        for (i, vec_item) in v.iter_mut().enumerate().take(dimension) {
            let bit_pos = (h1.wrapping_add((i as u64).wrapping_mul(h2))) % 64;
            let sign = if (h1 >> (bit_pos % 64)) & 1 == 1 {
                1.0f64
            } else {
                -1.0f64
            };
            let weight = 1.0 + (h2 % 100) as f64 / 100.0;
            *vec_item += sign * weight;
        }
    }

    let norm = v
        .iter()
        .map(|x| x * x)
        .sum::<f64>()
        .sqrt()
        .max(f64::EPSILON);
    v.iter().map(|x| (x / norm) as f32).collect()
}

fn bench_simhash_embedding_single(c: &mut Criterion) {
    let texts = [
        ("short", "hello world"),
        ("medium", "The quick brown fox jumps over the lazy dog"),
        (
            "long",
            "Rust is a systems programming language that runs blazingly fast, prevents segfaults, and guarantees thread safety.",
        ),
        (
            "code_block",
            "fn process_data<T: Serialize>(input: Vec<T>) -> Result<HashMap<String, T>, Error> { input.into_iter().map(|item| (format!(\"key_{}\", idx), item)).collect() }",
        ),
        (
            "chinese_mixed",
            "Rust 编程语言是一种系统级编程语言，具有内存安全、零成本抽象和并发特性。它由 Mozilla Research 开发。",
        ),
    ];

    let mut group = c.benchmark_group("embedding_simhash_single");
    for (name, text) in &texts {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_with_input(BenchmarkId::new("simhash", name), text, |b, text| {
            b.iter(|| black_box(simhash_embedding(black_box(text), EMBEDDING_DIM)));
        });
    }
    group.finish();
}

fn bench_simhash_embedding_dimensions(c: &mut Criterion) {
    let text =
        "Rust programming language with memory safety guarantees and zero-cost abstractions.";

    let mut group = c.benchmark_group("embedding_simhash_dimension");
    for dim in [64usize, 128, 256, 512, 768, 1024, 1536, 2048, 4096] {
        group.bench_with_input(
            BenchmarkId::new("simhash_dim", format!("d{dim}")),
            &(text, dim),
            |b, (text, dim)| {
                b.iter(|| black_box(simhash_embedding(black_box(text), black_box(*dim))));
            },
        );
    }
    group.finish();
}

fn bench_batch_embedding_throughput(c: &mut Criterion) {
    let texts: Vec<String> = (0..500)
        .map(|i| format!("Benchmark embedding text chunk number {i} for throughput testing"))
        .collect();

    let mut group = c.benchmark_group("embedding_batch_throughput");

    for batch_size in [1usize, 10, 32, 64, 128, 256, 500] {
        group.throughput(Throughput::Elements(batch_size as u64));
        let input = (&texts, batch_size);
        group.bench_with_input(
            BenchmarkId::new("simhash_batch", format!("n{batch_size}")),
            &input,
            |b, (texts, batch_size): &(&Vec<String>, usize)| {
                b.iter(|| {
                    let results: Vec<Vec<f32>> = texts[..*batch_size]
                        .iter()
                        .map(|t| simhash_embedding(black_box(t.as_str()), EMBEDDING_DIM))
                        .collect();
                    black_box(results);
                });
            },
        );
    }

    group.finish();
}

fn bench_cosine_similarity(c: &mut Criterion) {
    let v1 = simhash_embedding("rust programming language memory safety", EMBEDDING_DIM);
    let v2 = simhash_embedding(
        "rust systems programming zero cost abstraction",
        EMBEDDING_DIM,
    );
    let v3 = simhash_embedding(
        "completely unrelated topic about cooking recipes",
        EMBEDDING_DIM,
    );

    let mut group = c.benchmark_group("embedding_similarity");

    group.bench_function("cosine_similar_1536d", |b| {
        b.iter(|| black_box(cosine_similarity(black_box(&v1), black_box(&v2))));
    });

    group.bench_function("cosine_dissimilar_1536d", |b| {
        b.iter(|| black_box(cosine_similarity(black_box(&v1), black_box(&v3))));
    });

    for dim in [64usize, 128, 256, 512, 1024, 1536] {
        let a = simhash_embedding("similar text about rust programming", dim);
        let b = simhash_embedding("related text about rust language features", dim);

        group.bench_with_input(
            BenchmarkId::new("cosine_similar", format!("{dim}d")),
            &(a, b),
            |bencher, (a, b): &(Vec<f32>, Vec<f32>)| {
                bencher.iter(|| black_box(cosine_similarity(black_box(a), black_box(b))));
            },
        );
    }

    group.finish();
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn deterministic_f32(seed: u32, extra: u32) -> f32 {
    let hash = blake3::hash(format!("{seed}:{extra}").as_bytes());
    let h = u64::from_le_bytes(
        hash.as_bytes()[..8]
            .try_into()
            .expect("blake3 输出至少 8 字节"),
    );
    ((h % 10000) as f32 / 10000.0).mul_add(2.0, -1.0)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn bench_vector_operations(c: &mut Criterion) {
    let v1: Vec<f32> = (0..EMBEDDING_DIM).map(|i| i as f32 * 0.001).collect();
    let v2: Vec<f32> = (0..EMBEDDING_DIM)
        .map(|i| (i as f32).mul_add(0.001, 0.5).sin())
        .collect();
    let large_vecs: Vec<Vec<f32>> = (0..1000u32)
        .map(|i| {
            (0..EMBEDDING_DIM)
                .map(|j| deterministic_f32(i, j as u32))
                .collect()
        })
        .collect();

    let mut group = c.benchmark_group("embedding_vector_ops");

    group.bench_function("dot_product_1536d", |b| {
        b.iter(|| {
            let sum: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
            black_box(sum);
        });
    });

    group.bench_function("l2_norm_1536d", |b| {
        b.iter(|| {
            let norm: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
            black_box(norm);
        });
    });

    group.bench_function("normalize_1536d", |b| {
        b.iter(|| {
            let norm_sq: f32 = v1.iter().map(|x| x * x).sum();
            let norm = norm_sq.sqrt().max(f32::EPSILON);
            let normalized: Vec<f32> = v1.iter().map(|x| x / norm).collect();
            black_box(normalized);
        });
    });

    for k in [1usize, 5, 10, 50, 100] {
        let query = &large_vecs[0];
        group.bench_with_input(
            BenchmarkId::new("brute_force_knn", format!("k{k}_n1000")),
            &(query, &large_vecs, k),
            |b, (query, corpus, k)| {
                b.iter(|| {
                    let mut scores: Vec<(usize, f64)> = corpus
                        .iter()
                        .enumerate()
                        .map(|(i, v)| {
                            let sim = cosine_similarity(query, v);
                            (i, sim)
                        })
                        .collect();
                    scores.sort_by(|a, b| b.1.total_cmp(&a.1));
                    let top_k: Vec<(usize, f64)> = scores.into_iter().take(*k).collect();
                    black_box(top_k);
                });
            },
        );
    }

    group.finish();
}

fn bench_embedding_service_async(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("embedding_service_async");

    let service =
        EmbeddingService::new(EmbeddingModel::LocalBgeLarge, EMBEDDING_DIM, 16, 256).unwrap();

    let block_id = "bench_1".to_string();
    let content = "This is a test block for async embedding benchmark.".to_string();

    group.bench_function("single_embed_async", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = service
                    .embed_block(black_box(&block_id), black_box(&content))
                    .await
                    .unwrap();
                black_box(result.embedding);
            });
        });
    });

    let block_ids: Vec<String> = (0..50u32).map(|i| format!("bench_{i}")).collect();
    let contents: Vec<String> = (0..50u32)
        .map(|i| format!("Test block {i} content for batch embedding benchmarking."))
        .collect();

    for batch_size in [1usize, 10, 25, 50] {
        group.throughput(Throughput::Elements(batch_size as u64));
        let batch_ids: Vec<String> = block_ids[..batch_size].to_vec();
        let batch_contents: Vec<String> = contents[..batch_size].to_vec();
        group.bench_with_input(
            BenchmarkId::new("batch_embed_async", format!("n{batch_size}")),
            &(batch_ids, batch_contents),
            |b, (ids, contents)| {
                b.iter(|| {
                    rt.block_on(async {
                        let results = service
                            .embed_batch(black_box(ids), black_box(contents))
                            .await
                            .unwrap();
                        black_box(results);
                    });
                });
            },
        );
    }

    group.finish();
}

fn bench_text_tokenization_for_embedding(c: &mut Criterion) {
    let texts = [
        "hello world",
        "The quick brown fox jumps over the lazy dog and runs away quickly",
        "Rust is a multi-paradigm, general-purpose programming language that emphasizes performance, type safety, and concurrency",
        "函数式编程（Functional Programming）是一种编程范式，它将计算视为数学函数的求值过程，并避免改变状态和可变数据",
        "def fibonacci(n: int) -> list[int]: return [fib(i) for i in range(n)] if n > 0 else []",
        "{\"key\": \"value\", \"nested\": {\"array\": [1, 2, 3]}, \"number\": 42.5}",
    ];

    let mut group = c.benchmark_group("embedding_text_preprocessing");

    for text in &texts {
        let name = &text[..text.len().min(20)];
        group.throughput(Throughput::Bytes(text.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("tokenize_split_ws", name),
            text,
            |b, text| {
                b.iter(|| {
                    let tokens: Vec<&str> = black_box(text)
                        .split_whitespace()
                        .chain(black_box(text).split(|c: char| !c.is_alphanumeric()))
                        .filter(|t| !t.is_empty())
                        .collect();
                    black_box(tokens);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_simhash_embedding_single,
    bench_simhash_embedding_dimensions,
    bench_batch_embedding_throughput,
    bench_cosine_similarity,
    bench_vector_operations,
    bench_embedding_service_async,
    bench_text_tokenization_for_embedding
);
criterion_main!(benches);
