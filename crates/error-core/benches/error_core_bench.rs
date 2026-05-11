use criterion::{Criterion, criterion_group, criterion_main};

const fn benchmark_placeholder(_c: &mut Criterion) {}

criterion_group!(benches, benchmark_placeholder);
criterion_main!(benches);
