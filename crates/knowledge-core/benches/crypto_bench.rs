use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use knowledge_core::crypto::{Decryptor, Encryptor, KeyManager};
use std::hint::black_box;

const DATA_SIZES: &[usize] = &[1_024, 10_240, 102_400, 1_048_576];

fn generate_data(size: usize) -> Vec<u8> {
    vec![0xABu8; size]
}

fn bench_blake3_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_blake3_hash");
    for size in DATA_SIZES {
        let data = generate_data(*size);
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("blake3_hash", format_size(*size)),
            &data,
            |b, data| {
                b.iter(|| black_box(knowledge_core::crypto::hash(black_box(data))));
            },
        );
    }
    group.finish();
}

fn bench_blake3_hash_str(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_blake3_hash_str");
    for size in DATA_SIZES {
        let data = String::from_utf8(generate_data(*size)).unwrap();
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("blake3_hash_str", format_size(*size)),
            &data,
            |b, data| {
                b.iter(|| black_box(knowledge_core::crypto::hash_str(black_box(data.as_str()))));
            },
        );
    }
    group.finish();
}

fn bench_aes256_gcm_encrypt(c: &mut Criterion) {
    let key = KeyManager::generate_key();
    let encryptor = Encryptor::new(key).expect("Encryptor 初始化失败");

    let mut group = c.benchmark_group("crypto_aes256_encrypt");
    for size in DATA_SIZES {
        let plaintext = generate_data(*size);
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("aes256_gcm_encrypt", format_size(*size)),
            &plaintext,
            |b, pt| {
                b.iter(|| black_box(encryptor.encrypt(black_box(pt)).unwrap()));
            },
        );
    }
    group.finish();
}

fn bench_aes256_gcm_decrypt(c: &mut Criterion) {
    let key = KeyManager::generate_key();
    let encryptor = Encryptor::new(key).expect("Encryptor 初始化失败");
    let decryptor = Decryptor::new(key).expect("Decryptor 初始化失败");

    let mut group = c.benchmark_group("crypto_aes256_decrypt");
    for size in DATA_SIZES {
        let plaintext = generate_data(*size);
        let (nonce, ciphertext) = encryptor.encrypt(&plaintext).unwrap();
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("aes256_gcm_decrypt", format_size(*size)),
            &(nonce, ciphertext),
            |b, (nonce, ct)| {
                b.iter(|| black_box(decryptor.decrypt(black_box(nonce), black_box(ct)).unwrap()));
            },
        );
    }
    group.finish();
}

fn bench_aes256_roundtrip(c: &mut Criterion) {
    let key = KeyManager::generate_key();
    let encryptor = Encryptor::new(key).expect("Encryptor 初始化失败");
    let decryptor = Decryptor::new(key).expect("Decryptor 初始化失败");

    let mut group = c.benchmark_group("crypto_aes256_roundtrip");
    for size in DATA_SIZES {
        let plaintext = generate_data(*size);
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("aes256_roundtrip", format_size(*size)),
            &plaintext,
            |b, pt| {
                b.iter(|| {
                    let (nonce, ct) = encryptor.encrypt(black_box(pt)).unwrap();
                    black_box(decryptor.decrypt(&nonce, &ct).unwrap());
                });
            },
        );
    }
    group.finish();
}

fn bench_key_generation(c: &mut Criterion) {
    c.bench_function("crypto_key_generate", |b| {
        b.iter(|| black_box(KeyManager::generate_key()));
    });
}

fn bench_encryptor_creation(c: &mut Criterion) {
    let key = KeyManager::generate_key();
    c.bench_function("crypto_encryptor_new", |b| {
        b.iter(|| black_box(Encryptor::new(black_box(key)).unwrap()));
    });
}

fn format_size(b: usize) -> String {
    match b {
        b if b < 1_024 => format!("{b}B"),
        b if b < 1_048_576 => format!("{}KB", b / 1_024),
        b if b < 1_073_741_824 => format!("{}MB", b / 1_048_576),
        b => format!("{}GB", b / 1_073_741_824),
    }
}

criterion_group!(
    benches,
    bench_blake3_hash,
    bench_blake3_hash_str,
    bench_aes256_gcm_encrypt,
    bench_aes256_gcm_decrypt,
    bench_aes256_roundtrip,
    bench_key_generation,
    bench_encryptor_creation
);
criterion_main!(benches);
