use axum::{
    Json, Router,
    body::Body,
    extract::Query,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use knowledge_core::model::{Block, BlockType, Document, SourceType};
use serde::Deserialize;
use tower::ServiceExt;

async fn bench_health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "version": "0.1.0-bench",
        "timestamp": "bench-timestamp"
    }))
}

async fn bench_echo_handler(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    Json(body)
}

async fn bench_query_handler(Query(params): Query<BenchQueryParams>) -> impl IntoResponse {
    let _ = params.q;
    let _ = params.limit;
    Json(serde_json::json!({
        "results": [],
        "total": 0
    }))
}

#[derive(Debug, Deserialize)]
struct BenchQueryParams {
    q: String,
    limit: Option<usize>,
}

fn create_bench_app() -> Router {
    Router::new()
        .route("/healthz", get(bench_health_handler))
        .route("/api/echo", post(bench_echo_handler))
        .route("/api/search", get(bench_query_handler))
}

fn bench_health_endpoint(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let app = create_bench_app().into_service();

    let mut group = c.benchmark_group("api_http_latency");
    group.bench_function("health_check_get", |b| {
        b.iter(|| {
            let req = Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap();
            let app_clone = app.clone();
            rt.block_on(async {
                let resp = app_clone.oneshot(req).await.unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                black_box(resp);
            });
        });
    });
    group.finish();
}

fn bench_json_payload_handling(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("api_json_payload");
    for payload_size in [100usize, 1_000, 10_000] {
        let body: serde_json::Value = match payload_size {
            100 => serde_json::json!({"path":"/test.md","title":"Test","content":"x".repeat(50)}),
            1000 => {
                serde_json::json!({"path":"/bench/large.md","title":"Large Benchmark Document","content":"A ".repeat(950)})
            }
            10000 => {
                serde_json::json!({"path":"/bench/xlarge.md","title":"XLarge Benchmark Document For Performance Testing","content":"Performance testing data. ".repeat(200)})
            }
            _ => unreachable!(),
        };

        let app = create_bench_app().into_service();
        let body_bytes = serde_json::to_vec(&body).unwrap();

        group.throughput(Throughput::Bytes(body_bytes.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("echo_post_payload", format!("{payload_size}B")),
            &body_bytes,
            |b, payload| {
                b.iter(|| {
                    let req = Request::builder()
                        .method("POST")
                        .uri("/api/echo")
                        .header("content-type", "application/json")
                        .body(Body::from(payload.clone()))
                        .unwrap();
                    let app_clone = app.clone();
                    rt.block_on(async {
                        let resp = app_clone.oneshot(req).await.unwrap();
                        assert_eq!(resp.status(), StatusCode::OK);
                        black_box(resp);
                    });
                });
            },
        );
    }
    group.finish();
}

fn bench_query_parameter_parsing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let app = create_bench_app().into_service();

    let mut group = c.benchmark_group("api_query_parsing");

    group.bench_function("search_simple_query", |b| {
        b.iter(|| {
            let req = Request::builder()
                .uri("/api/search?q=rust+programming&limit=20")
                .body(Body::empty())
                .unwrap();
            let app_clone = app.clone();
            rt.block_on(async {
                let resp = app_clone.oneshot(req).await.unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                black_box(resp);
            });
        });
    });

    group.bench_function("search_complex_query", |b| {
        b.iter(|| {
            let query = urlencoding::encode("Rust 异步编程 + tokio runtime 性能优化");
            let uri = format!("/api/search?q={query}&limit=50&offset=10");
            let req = Request::builder().uri(&uri).body(Body::empty()).unwrap();
            let app_clone = app.clone();
            rt.block_on(async {
                let resp = app_clone.oneshot(req).await.unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                black_box(resp);
            });
        });
    });
    group.finish();
}

fn bench_response_serialization(c: &mut Criterion) {
    let documents: Vec<Document> = (0..100u32)
        .map(|i| Document {
            id: None,
            path: format!("/doc_{i}.md"),
            title: format!("Document {i}"),
            source_type: SourceType::Markdown,
            hash: "a".repeat(64),
        })
        .collect();

    let api_response = serde_json::json!({
        "success": true,
        "data": documents,
        "pagination": { "total": 100, "offset": 0, "limit": 100 }
    });

    let mut group = c.benchmark_group("api_response_serialization");
    group.bench_function("serialize_100_documents_response", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(black_box(&api_response)).unwrap());
        });
    });

    let blocks: Vec<Block> = (0..500u32)
        .map(|i| Block {
            id: None,
            doc_id: "document:ser_bench".parse().expect("解析失败"),
            block_type: BlockType::Paragraph,
            start_line: i * 5,
            end_line: i * 5 + 5,
            embedding: None,
            idempotency_key: Some(format!("para_{}_{}", i * 5, i * 5 + 5)),
        })
        .collect();

    let block_response = serde_json::json!({
        "success": true,
        "data": blocks
    });

    group.bench_function("serialize_500_blocks_response", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(black_box(&block_response)).unwrap());
        });
    });
    group.finish();
}

fn bench_blake3_hash_in_api_path(c: &mut Criterion) {
    use blake3::Hasher;

    let content_sizes: &[usize] = &[1_024, 10_240, 102_400];
    let mut group = c.benchmark_group("api_blake3_content_hashing");

    for size in content_sizes {
        let content = "The quick brown fox jumps over the lazy dog. ".repeat(size / 45);
        group.throughput(Throughput::Bytes(content.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("blake3_hash_content", format!("{}B", content.len())),
            &content,
            |b, content| {
                b.iter(|| {
                    let mut hasher = Hasher::new();
                    hasher.update(black_box(content.as_bytes()));
                    black_box(hasher.finalize().to_hex().to_string());
                });
            },
        );
    }
    group.finish();
}

fn bench_concurrent_request_simulation(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let app = create_bench_app().into_service();

    let mut group = c.benchmark_group("api_concurrent_requests");

    for concurrency in [1usize, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("concurrent_health_checks", concurrency),
            &concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    let handles: Vec<_> = (0..concurrency)
                        .map(|_| {
                            let app_clone = app.clone();
                            rt.spawn(async move {
                                let req = Request::builder()
                                    .uri("/healthz")
                                    .body(Body::empty())
                                    .unwrap();
                                let resp = app_clone.oneshot(req).await.unwrap();
                                assert_eq!(resp.status(), StatusCode::OK);
                                resp
                            })
                        })
                        .collect();
                    for h in handles {
                        black_box(rt.block_on(h).unwrap());
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_router_matching(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let complex_app = Router::new()
        .route("/healthz", get(bench_health_handler))
        .route(
            "/api/v1/documents",
            get(bench_health_handler).post(bench_echo_handler),
        )
        .route(
            "/api/v1/documents/:id",
            get(bench_health_handler).delete(bench_health_handler),
        )
        .route("/api/v1/documents/:id/blocks", get(bench_health_handler))
        .route("/api/v1/blocks/:id", get(bench_health_handler))
        .route("/api/v1/blocks/:id/tokens", get(bench_health_handler))
        .route("/api/v1/tokens/:id", get(bench_health_handler))
        .route("/api/v1/tokens/:id/references", get(bench_health_handler))
        .route("/api/v1/search/vector", post(bench_echo_handler))
        .route("/api/v1/search/fulltext", get(bench_query_handler));

    let app = complex_app.into_service();

    let routes = [
        "/healthz",
        "/api/v1/documents",
        "/api/v1/documents/doc123",
        "/api/v1/documents/doc123/blocks",
        "/api/v1/blocks/block456",
        "/api/v1/blocks/block456/tokens",
        "/api/v1/tokens/token789",
        "/api/v1/tokens/token789/references",
        "/api/v1/search/fulltext?q=test&limit=20",
    ];

    let mut group = c.benchmark_group("api_router_matching");
    for route in &routes {
        let method = if route.starts_with("/api/v1/search/vector") {
            "POST"
        } else {
            "GET"
        };
        let name = route.replace('/', "_").trim_matches('_').to_string();
        group.bench_function(name, |b| {
            b.iter(|| {
                let req = Request::builder()
                    .method(method)
                    .uri(*route)
                    .body(Body::empty())
                    .unwrap();
                let app_clone = app.clone();
                rt.block_on(async {
                    let resp = app_clone.oneshot(req).await.unwrap();
                    black_box(resp.status());
                });
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_health_endpoint,
    bench_json_payload_handling,
    bench_query_parameter_parsing,
    bench_response_serialization,
    bench_blake3_hash_in_api_path,
    bench_concurrent_request_simulation,
    bench_router_matching
);
criterion_main!(benches);
