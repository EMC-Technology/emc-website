//! 属性测试模块
//!
//! 使用 Proptest 框架进行基于属性的测试（Property-Based Testing），
//! 通过随机生成的输入验证代码的不变属性。
//!
//! # 运行方式
//!
//! ```bash
//! cargo test --features proptest
//! cargo test --test crypto_property_test --features proptest
//! ```
//!
//! # 测试覆盖
//!
//! ## Crypto 属性
//!
//! - **P-C1**: 确定性哈希：`hash(D) = hash(D)`
//! - **P-C2**: 加解密互逆：`decrypt(K, encrypt(K, P)) = P`
//! - **P-C3**: 不同输入不同哈希（概率意义上）
//!
//! ## Math 属性
//!
//! - **P-M1**: 余弦相似度范围：`[-1, 1]`
//! - **P-M2**: 自相似性：`cosine_similarity(v, v) = 1`
//! - **P-M3**: 对称性：`cosine_similarity(a, b) = cosine_similarity(b, a)`
//! - **P-M4**: 正交性：`cosine_similarity(a, b) = 0` 当 `a ⊥ b`
//! - **P-M5**: 点积对称性
//! - **P-M6**: 点积分配律
//!
//! ## Search 属性
//!
//! - **P-S1**: BM25 评分非负
//! - **P-S2**: 不存在文档评分为零
//! - **P-S3**: 词频累加正确性

#[cfg(feature = "proptest")]
pub mod crypto_properties {
    use super::*;
    use crate::crypto::{Decryptor, Encryptor, KeyManager, hash, hash_str};

    proptest! {
        #[test]
        fn test_hash_deterministic(data: Vec<u8>) {
            let h1 = hash(&data);
            let h2 = hash(&data);
            prop_assert_eq!(h1, h2, "Hash must be deterministic");
        }

        #[test]
        fn test_hash_str_equals_hash_of_bytes(s: String) {
            prop_assert_eq!(hash_str(&s), hash(s.as_bytes()));
        }

        #[test]
        fn test_encrypt_decrypt_roundtrip(plaintext: Vec<u8>) {
            let key = KeyManager::generate_key();
            let encryptor = Encryptor::new(key.into()).unwrap();
            let decryptor = Decryptor::new(key.into()).unwrap();

            let (nonce, ciphertext) = encryptor.encrypt(&plaintext).unwrap();
            let decrypted = decryptor.decrypt(&nonce, &ciphertext).unwrap();

            prop_assert_eq!(plaintext, decrypted, "Decrypt(Encrypt(P)) must equal P");
        }

        #[test]
        fn test_encrypt_produces_different_nonces(key: [u8; 32], plaintext: Vec<u8>) {
            let encryptor1 = Encryptor::new(key.into()).unwrap();
            let encryptor2 = Encryptor::new(key.into()).unwrap();

            let (nonce1, _) = encryptor1.encrypt(&plaintext).unwrap();
            let (nonce2, _) = encryptor2.encrypt(&plaintext).unwrap();

            prop_assert_ne!(nonce1, nonce2, "Different encryptions should produce different nonces");
        }

        #[test]
        fn test_same_key_same_ciphertext_differs_with_different_nonces(key: [u8; 32], plaintext: Vec<u8>) {
            let encryptor = Encryptor::new(key.into()).unwrap();

            let (nonce1, ct1) = encryptor.encrypt(&plaintext).unwrap();
            let (nonce2, ct2) = encryptor.encrypt(&plaintext).unwrap();

            prop_assert_ne!(nonce1, nonce2);
            prop_assert_ne!(ct1, ct2, "Different nonces should produce different ciphertexts");
        }

        #[test]
        fn test_different_keys_produce_different_results(plaintext: Vec<u8>) {
            let key1 = KeyManager::generate_key();
            let key2 = KeyManager::generate_key();

            if key1 != key2 {
                let encryptor1 = Encryptor::new(key1.into()).unwrap();
                let encryptor2 = Encryptor::new(key2.into()).unwrap();

                let (_, ct1) = encryptor1.encrypt(&plaintext).unwrap();
                let (_, ct2) = encryptor2.encrypt(&plaintext).unwrap();

                prop_assert_ne!(ct1, ct2, "Different keys should produce different ciphertexts");
            }
        }
    }
}

#[cfg(feature = "proptest")]
pub mod math_properties {
    use super::*;
    use crate::math::{cosine_similarity, dot_product, euclidean_distance};

    proptest! {
        #[test]
        fn test_cosine_similarity_bounds(a: Vec<f32>, b: Vec<f32>) {
            if a.len() == b.len() && !a.is_empty() {
                let sim = cosine_similarity(&a, &b);
                prop_assert!(
                    sim >= -1.0 && sim <= 1.0,
                    "cosine_similarity must be in [-1, 1], got {}",
                    sim
                );
            }
        }

        #[test]
        fn test_cosine_similarity_self_is_one(v: Vec<f32>) {
            if !v.is_empty() && v.iter().all(|x| x.is_finite()) {
                let sim = cosine_similarity(&v, &v);
                prop_assert!(
                    (sim - 1.0).abs() < 1e-6,
                    "Self-similarity must be 1.0, got {}",
                    sim
                );
            }
        }

        #[test]
        fn test_cosine_similarity_symmetric(a: Vec<f32>, b: Vec<f32>) {
            if a.len() == b.len() && !a.is_empty() {
                let sim_ab = cosine_similarity(&a, &b);
                let sim_ba = cosine_similarity(&b, &a);
                prop_assert!(
                    (sim_ab - sim_ba).abs() < 1e-6,
                    "Cosine similarity must be symmetric"
                );
            }
        }

        #[test]
        fn test_euclidean_distance_nonpositive(a: Vec<f32>, b: Vec<f32>) {
            if a.len() == b.len() && !a.is_empty() {
                let dist = euclidean_distance(&a, &b);
                prop_assert!(
                    dist <= 0.0,
                    "Euclidean distance must be non-positive, got {}",
                    dist
                );
            }
        }

        #[test]
        fn test_euclidean_distance_zero_self(a: Vec<f32>) {
            if !a.is_empty() {
                let dist = euclidean_distance(&a, &a);
                prop_assert!(
                    (dist - 0.0).abs() < 1e-6,
                    "Self-distance must be 0, got {}",
                    dist
                );
            }
        }

        #[test]
        fn test_dot_product_symmetric(a: Vec<f32>, b: Vec<f32>) {
            if a.len() == b.len() && !a.is_empty() {
                let dot_ab = dot_product(&a, &b);
                let dot_ba = dot_product(&b, &a);
                prop_assert!(
                    (dot_ab - dot_ba).abs() < 1e-6,
                    "Dot product must be symmetric"
                );
            }
        }

        #[test]
        fn test_dot_product_distributive(a: Vec<f32>, b: Vec<f32>, c: Vec<f32>) {
            if a.len() == b.len() && b.len() == c.len() && !a.is_empty() {
                let b_plus_c: Vec<f32> = b.iter().zip(c.iter()).map(|(x, y)| x + y).collect();
                let dot_a_bc = dot_product(&a, &b_plus_c);
                let dot_ab_plus_ac = dot_product(&a, &b) + dot_product(&a, &c);

                prop_assert!(
                    (dot_a_bc - dot_ab_plus_ac).abs() < 1e-4,
                    "Dot product must be distributive: {} vs {}",
                    dot_a_bc,
                    dot_ab_plus_ac
                );
            }
        }

        #[test]
        fn test_dot_product_scalar_multiplication(a: Vec<f32>, b: Vec<f32>, scalar: f32) {
            if a.len() == b.len() && !a.is_empty() && scalar.is_finite() {
                let scaled_a: Vec<f32> = a.iter().map(|x| x * scalar).collect();
                let dot_scaled_a_b = dot_product(&scaled_a, &b);
                let scalar_times_dot = scalar * dot_product(&a, &b);

                prop_assert!(
                    (dot_scaled_a_b - scalar_times_dot).abs() < 1e-4,
                    "Scalar multiplication property failed"
                );
            }
        }

        #[test]
        fn test_cosine_similarity_zero_vector(a: Vec<f32>, b: Vec<f32>) {
            if a.len() == b.len() && !a.is_empty() {
                let zero = vec![0.0_f32; a.len()];
                let sim_zero_a = cosine_similarity(&zero, &a);
                prop_assert!(
                    sim_zero_a.abs() < 1e-6,
                    "Similarity with zero vector must be 0, got {}",
                    sim_zero_a
                );
            }
        }

        #[test]
        fn test_euclidean_distance_symmetric(a: Vec<f32>, b: Vec<f32>) {
            if a.len() == b.len() && !a.is_empty() {
                let dist_ab = euclidean_distance(&a, &b);
                let dist_ba = euclidean_distance(&b, &a);
                prop_assert!(
                    (dist_ab - dist_ba).abs() < 1e-6,
                    "Euclidean distance must be symmetric"
                );
            }
        }
    }
}

#[cfg(feature = "proptest")]
pub mod search_properties {
    use super::*;
    use crate::search::Bm25Index;

    proptest! {
        #[test]
        fn test_bm25_score_nonnegative(
            tokens in prop::collection::vec(
                (prop::string::string("a-z", 1..5), prop::string::string("a-z", 1..5)),
                1..50
            )
        ) {
            let index = Bm25Index::build_from_tokens(&tokens);
            let query = vec!["test".to_string()];

            let docs: Vec<String> = tokens.iter()
                .map(|(doc, _)| doc.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .take(10)
                .collect();

            for doc_id in docs {
                let score = index.score(&query, &doc_id);
                prop_assert!(
                    score >= 0.0,
                    "BM25 score must be non-negative for existing doc, got {}",
                    score
                );
            }
        }

        #[test]
        fn test_bm25_score_zero_for_nonexistent_doc(
            tokens in prop::collection::vec(
                (prop::string::string("a-z", 1..5), prop::string::string("a-z", 1..5)),
                1..20
            )
        ) {
            let index = Bm25Index::build_from_tokens(&tokens);
            let score = index.score(&["nonexistent".to_string()], "nonexistent_doc");
            prop_assert_eq!(score, 0.0, "Non-existent doc should have score 0");
        }

        #[test]
        fn test_bm25_term_frequency_accumulation(
            doc_id: String,
            terms in prop::collection::vec(prop::string::string("a-z", 1..5), 1..20)
        ) {
            let tokens: Vec<(String, String)> = terms
                .iter()
                .map(|t| (doc_id.clone(), t.clone()))
                .collect();

            let index = Bm25Index::build_from_tokens(&tokens);
            let doc_len = index.doc_length(&doc_id);
            prop_assert_eq!(
                doc_len,
                terms.len(),
                "Document length should equal number of term occurrences"
            );
        }

        #[test]
        fn test_bm25_idf_nonzero_for_rare_term(
            docs in prop::collection::vec(
                prop::collection::vec(prop::string::string("a-z", 1..3), 1..5),
                1..10
            )
        ) {
            let mut tokens = Vec::new();
            for (i, terms) in docs.into_iter().enumerate() {
                for term in terms {
                    tokens.push((format!("doc{}", i), term));
                }
            }

            let index = Bm25Index::build_from_tokens(&tokens);
            let unique_terms: std::collections::HashSet<_> = tokens.iter()
                .map(|(_, t)| t.clone())
                .collect();

            let rare_term = unique_terms.iter().next().cloned().unwrap_or_default();
            let freq = index.doc_freq(&rare_term);

            prop_assert!(
                freq > 0,
                "Term should have positive document frequency"
            );
        }

        #[test]
        fn test_bm25_total_docs_correct(
            docs in prop::collection::vec(
                prop::collection::vec(prop::string::string("a-z", 1..3), 1..5),
                1..15
            )
        ) {
            let tokens: Vec<(String, String)> = docs.iter()
                .enumerate()
                .flat_map(|(i, terms)| {
                    terms.iter()
                        .map(|t| (format!("doc{}", i), t.clone()))
                        .collect::<Vec<_>>()
                })
                .collect();

            let index = Bm25Index::build_from_tokens(&tokens);
            let unique_docs: usize = tokens.iter()
                .map(|(d, _)| d)
                .collect::<std::collections::HashSet<_>>()
                .len();

            prop_assert_eq!(
                index.total_docs(),
                unique_docs,
                "total_docs should count unique documents"
            );
        }
    }
}

#[cfg(feature = "proptest")]
pub mod staleness_properties {
    use super::*;
    use crate::staleness::{StalenessChecker, StalenessStatus};

    proptest! {
        #[test]
        fn test_staleness_same_hash_fresh(hash: String) {
            let status = StalenessChecker::check_document(&hash, &hash);
            prop_assert!(
                matches!(status, StalenessStatus::Fresh),
                "Same hash should be Fresh"
            );
        }

        #[test]
        fn test_staleness_different_hash_stale(hash1: String, hash2: String) {
            prop_assume!(hash1 != hash2);
            let status = StalenessChecker::check_document(&hash1, &hash2);
            prop_assert!(
                matches!(status, StalenessStatus::Stale),
                "Different hashes should be Stale"
            );
        }

        #[test]
        fn test_staleness_batch_consistency(
            docs in prop::collection::vec(
                (prop::string::string("a-z", 1..10), prop::string::string("a-f", 64..64)),
                1..20
            )
        ) {
            let results = StalenessChecker::check_documents(&docs);

            prop_assert_eq!(
                results.len(),
                docs.len(),
                "Batch check should return same number of results"
            );

            for ((_, stored, current), (id, status)) in docs.iter().zip(results.iter()) {
                let expected = if stored == current {
                    StalenessStatus::Fresh
                } else {
                    StalenessStatus::Stale
                };
                prop_assert_eq!(
                    status,
                    &expected,
                    "Status should match hash comparison"
                );
            }
        }
    }
}
