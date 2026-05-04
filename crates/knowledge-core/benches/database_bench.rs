use criterion::{
    black_box, criterion_group, criterion_main, Criterion,
};
use knowledge_core::model::{Document, Block, Token, Reference, SourceType, BlockType, TokenType, RefType, Direction, RecordIdType};

fn make_test_doc(id_num: usize) -> Document {
    Document {
        id: None,
        path: format!("/bench/doc_{id_num}.md"),
        title: format!("Benchmark Document {id_num}"),
        source_type: SourceType::Markdown,
        hash: "a".repeat(64),
    }
}

const fn make_test_block(doc_id: RecordIdType, line_start: u32) -> Block {
    Block::new(doc_id, BlockType::Paragraph, line_start, line_start + 10)
}

fn make_test_token(block_id: RecordIdType, idx: u32) -> Token {
    Token::new(
        block_id,
        format!("benchmark_token_{idx}"),
        TokenType::Word,
        idx * 5,
        (idx * 100).into(),
    )
}

const fn make_test_reference(from: RecordIdType, to: RecordIdType) -> Reference {
    Reference::new(RefType::Usage, Direction::OneWay, from, to)
}

fn bench_document_create(c: &mut Criterion) {
    c.bench_function("document_create_placeholder", |b| {
        b.iter(|| black_box(make_test_doc(0)));
    });
}

fn bench_document_find_by_id(c: &mut Criterion) {
    c.bench_function("document_find_by_id_placeholder", |b| {
        b.iter(|| black_box(make_test_doc(1)));
    });
}

fn bench_document_find_by_hash(c: &mut Criterion) {
    c.bench_function("document_find_by_hash_placeholder", |b| {
        b.iter(|| black_box(make_test_doc(2)));
    });
}

fn bench_block_query(c: &mut Criterion) {
    let doc_id: RecordIdType = "document:bench".parse().expect("解析失败");
    c.bench_function("block_query_placeholder", |b| {
        b.iter(|| black_box(make_test_block(doc_id.clone(), 0)));
    });
}

fn bench_batch_vs_single_insert_token(c: &mut Criterion) {
    let block_id: RecordIdType = "block:bench".parse().expect("解析失败");
    c.bench_function("batch_insert_token_placeholder", |b| {
        b.iter(|| black_box(make_test_token(block_id.clone(), 0)));
    });
}

fn bench_serialization_overhead(c: &mut Criterion) {
    let doc = make_test_doc(42);
    let block = make_test_block(
        "document:ser".parse().expect("解析失败"),
        10,
    );
    let token = make_test_token("block:ser".parse().expect("解析失败"), 1);
    let reference = make_test_reference(
        "token:from_ser".parse().expect("解析失败"),
        "token:to_ser".parse().expect("解析失败"),
    );

    let mut group = c.benchmark_group("db_serialization");
    group.bench_function("serialize_document", |b| {
        b.iter(|| black_box(serde_json::to_value(black_box(&doc)).unwrap()));
    });
    group.bench_function("serialize_block", |b| {
        b.iter(|| black_box(serde_json::to_value(black_box(&block)).unwrap()));
    });
    group.bench_function("serialize_token", |b| {
        b.iter(|| black_box(serde_json::to_value(black_box(&token)).unwrap()));
    });
    group.bench_function("serialize_reference", |b| {
        b.iter(|| black_box(serde_json::to_value(black_box(&reference)).unwrap()));
    });

    let doc_json = serde_json::to_value(&doc).unwrap();
    group.bench_function("deserialize_document", |b| {
        b.iter(|| {
            black_box(
                serde_json::from_value::<Document>(black_box(doc_json.clone())).unwrap(),
            );
        });
    });
    group.finish();
}

fn bench_knowledge_graph_save(c: &mut Criterion) {
    c.bench_function("knowledge_graph_save_placeholder", |b| {
        b.iter(|| black_box(make_test_doc(99)));
    });
}

fn bench_concurrent_read_contention(c: &mut Criterion) {
    c.bench_function("concurrent_read_contention_placeholder", |b| {
        b.iter(|| black_box(make_test_doc(100)));
    });
}

criterion_group!(
    benches,
    bench_document_create,
    bench_document_find_by_id,
    bench_document_find_by_hash,
    bench_block_query,
    bench_batch_vs_single_insert_token,
    bench_serialization_overhead,
    bench_knowledge_graph_save,
    bench_concurrent_read_contention
);
criterion_main!(benches);
