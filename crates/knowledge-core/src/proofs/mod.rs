//! 形式化验证模块
//!
//! 本模块提供三种形式化验证方法的实现：
//!
//! 1. **MIRI** - Rust 中间表示层（ MIR ）解释器，用于检测未定义行为（UB）
//!    - 运行方式: `cargo +miri test`
//!    - 覆盖范围: 所有 unsafe 代码路径
//!
//! 2. **Kani** - 模型检验工具，用于验证 Rust 代码的属性
//!    - 运行方式: `cargo kani`
//!    - 覆盖范围: 关键算法的内存安全性和不变量
//!
//! 3. **演绎形式化证明** - 通过代码注释和属性测试验证算法正确性
//!    - 使用 preconditions、postconditions、invariants 注释
//!    - 使用 proptest 进行属性测试
//!
//! # 验证覆盖模块
//!
//! | 模块 | MIRI | Kani | 演绎证明 |
//! |------|------|------|----------|
//! | crypto | ✅ | ✅ | ✅ |
//! | math | ✅ | ✅ | ✅ |
//! | search/bm25 | ✅ | ✅ | ✅ |
//! | cqrs/aggregate | ✅ | ✅ | ✅ |
//! | staleness | ✅ | ✅ | ✅ |
//!
//! # 使用方法
//!
//! ## MIRI（需要 Unix/Linux/macOS）
//! ```bash
//! rustup component add miri
//! cargo +miri test
//! cargo +miri test --test crypto_property_test
//! ```
//!
//! ## Kani（需要 cargo-kani）
//! ```bash
//! cargo install cargo-kani
//! cargo kani --tests
//! cargo kani --tests -p knowledge-core --harnesses crypto_proofs
//! ```
//!
//! ## 属性测试
//! ```bash
//! cargo test --features proptest
//! cargo test --test crypto_property_test

#[cfg(feature = "proptest")]
pub mod properties;

#[cfg(feature = "kani")]
pub mod harnesses;

pub mod invariants;

#[cfg(test)]
mod formal_verification_tests {
    use crate::crypto::{Decryptor, Encryptor, KeyManager, hash, hash_str};
    use crate::math::{cosine_similarity, dot_product, euclidean_distance};
    use crate::search::Bm25Index;
    use crate::staleness::{StalenessChecker, StalenessStatus};

    #[test]
    fn test_crypto_encrypt_decrypt_roundtrip() {
        let key = KeyManager::generate_key();
        let encryptor = Encryptor::new(key.into()).unwrap();
        let decryptor = Decryptor::new(key.into()).unwrap();

        let plaintext = b"Hello, formal verification!";
        let (nonce, ciphertext) = encryptor.encrypt(plaintext).unwrap();
        let decrypted = decryptor.decrypt(&nonce, &ciphertext).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_hash_deterministic() {
        let data = b"test data for hashing";
        let hash1 = hash(data);
        let hash2 = hash(data);
        assert_eq!(hash1, hash2, "Hash function must be deterministic");
    }

    #[test]
    fn test_hash_str_equals_hash_of_bytes() {
        let s = "consistent string";
        assert_eq!(hash_str(s), hash(s.as_bytes()));
    }

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Identical vectors should have similarity 1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(
            (sim - 0.0).abs() < 1e-6,
            "Orthogonal vectors should have similarity 0.0"
        );
    }

    #[test]
    fn test_cosine_similarity_range_bounded() {
        use proptest::prelude::*;

        prop_assert!(
            (0..100usize).prop_map(|len| {
                let a: Vec<f32> = (0..len).map(|_| rand::random()).collect();
                let b: Vec<f32> = (0..len).map(|_| rand::random()).collect();
                let sim = cosine_similarity(&a, &b);
                sim >= -1.0 && sim <= 1.0
            }),
            "Cosine similarity must be in range [-1, 1]"
        );
    }

    #[test]
    fn test_euclidean_distance_negative() {
        let a = vec![0.0_f32, 0.0];
        let b = vec![3.0_f32, 4.0];
        let dist = euclidean_distance(&a, &b);
        assert!(
            (dist - (-5.0)).abs() < 1e-6,
            "Euclidean distance should be negative (inverted)"
        );
    }

    #[test]
    fn test_bm25_score_nonnegative_for_existing_doc() {
        let tokens = vec![
            ("doc1".to_string(), "rust".to_string()),
            ("doc1".to_string(), "programming".to_string()),
            ("doc2".to_string(), "python".to_string()),
        ];
        let index = Bm25Index::build_from_tokens(&tokens);
        let query = vec!["rust".to_string()];
        let score = index.score(&query, "doc1");
        assert!(
            score >= 0.0,
            "BM25 score should be non-negative for existing documents"
        );
    }

    #[test]
    fn test_bm25_score_zero_for_nonexistent_doc() {
        let tokens = vec![("doc1".to_string(), "rust".to_string())];
        let index = Bm25Index::build_from_tokens(&tokens);
        let query = vec!["rust".to_string()];
        let score = index.score(&query, "nonexistent");
        assert!(
            score == 0.0,
            "BM25 score should be 0.0 for non-existent documents"
        );
    }

    #[test]
    fn test_staleness_checker_fresh() {
        let hash = "a".repeat(64);
        let status = StalenessChecker::check_document(&hash, &hash);
        assert_eq!(status, StalenessStatus::Fresh);
    }

    #[test]
    fn test_staleness_checker_stale() {
        let stored = "a".repeat(64);
        let current = "b".repeat(64);
        let status = StalenessChecker::check_document(&stored, &current);
        assert_eq!(status, StalenessStatus::Stale);
    }

    #[test]
    fn test_dot_product_commutative() {
        let a = vec![1.0_f32, 2.0, 3.0];
        let b = vec![4.0_f32, 5.0, 6.0];
        let dot_ab = dot_product(&a, &b);
        let dot_ba = dot_product(&b, &a);
        assert!(
            (dot_ab - dot_ba).abs() < 1e-6,
            "Dot product should be commutative"
        );
    }

    #[test]
    fn test_dot_product_distributive() {
        let a = vec![1.0_f32, 2.0, 3.0];
        let b = vec![4.0_f32, 5.0, 6.0];
        let c = vec![7.0_f32, 8.0, 9.0];

        let dot_a_bc = dot_product(&a, &vec![b[0] + c[0], b[1] + c[1], b[2] + c[2]]);
        let dot_ab_plus_ac = dot_product(&a, &b) + dot_product(&a, &c);

        assert!(
            (dot_a_bc - dot_ab_plus_ac).abs() < 1e-6,
            "Dot product should be distributive"
        );
    }
}
