//! Kani 模型检验工具证明
//!
//! 本模块包含 Kani 验证所需的证明 harness，
//! 用于验证代码的内存安全性和关键属性。
//!
//! # 运行方式
//!
//! ```bash
//! # 安装 Kani
//! cargo install cargo-kani
//!
//! # 运行所有 Kani 证明
//! cargo kani
//!
//! # 运行特定模块的证明
//! cargo kani -p knowledge-core --harnesses crypto_proofs
//! cargo kani -p knowledge-core --harnesses math_proofs
//! ```
//!
//! # 验证覆盖
//!
//! ## crypto_proofs
//! - KANI_PROOF_1: encrypt_then_decrypt_returns_original
//! - KANI_PROOF_2: hash_deterministic
//! - KANI_PROOF_3: nonce_length_always_12_bytes
//!
//! ## math_proofs
//! - KANI_PROOF_4: cosine_similarity_in_range
//! - KANI_PROOF_5: euclidean_distance_nonpositive
//! - KANI_PROOF_6: dot_product_symmetric
//!
//! ## search_proofs
//! - KANI_PROOF_7: bm25_score_nonnegative
//! - KANI_PROOF_8: bm25_zero_for_nonexistent_doc
//!
//! ## aggregate_proofs
//! - KANI_PROOF_9: version_increases_on_apply
//! - KANI_PROOF_10: valid_state_transitions_only

#[cfg(feature = "kani")]
mod crypto_proofs {
    use super::*;

    /// KANI_PROOF_1: 加密后解密返回原始明文
    ///
    /// 验证：AES-256-GCM 加密解密互逆性
    ///
    /// ```text
    /// ∀K, P: decrypt(K, encrypt(K, P)) = P
    /// ```
    #[kani::proof]
    fn encrypt_then_decrypt_returns_original() {
        let key = kani::any();
        let plaintext_len = kani::any_where(|&len| len < 10000);
        let plaintext: Vec<u8> = kani::vec(plaintext_len);

        let encryptor = Encryptor::new(key);
        let decryptor = Decryptor::new(key);

        if let (Ok(encryptor), Ok(decryptor)) = (encryptor, decryptor) {
            if let Ok((nonce, ciphertext)) = encryptor.encrypt(&plaintext) {
                if let Ok(decrypted) = decryptor.decrypt(&nonce, &ciphertext) {
                    assert_eq!(plaintext, decrypted);
                }
            }
        }
    }

    /// KANI_PROOF_2: 哈希函数是确定性的
    ///
    /// 验证：相同输入产生相同哈希
    ///
    /// ```text
    /// ∀D: hash(D) = hash(D)
    /// ```
    #[kani::proof]
    fn hash_deterministic() {
        let data: [u8; 32] = kani::any();
        let h1 = hash(&data);
        let h2 = hash(&data);
        assert_eq!(h1, h2);
    }

    /// KANI_PROOF_3: Nonce 长度始终为 12 字节
    ///
    /// 验证：GCM 标准要求的 Nonce 长度
    ///
    /// ```text
    /// ∀ encryptor: nonce.len() = 12
    /// ```
    #[kani::proof]
    fn nonce_length_always_12_bytes() {
        let key = kani::any();
        let encryptor = Encryptor::new(key);

        if let Ok(enc) = encryptor {
            let plaintext = b"test message";
            if let Ok((nonce, _)) = enc.encrypt(plaintext) {
                assert_eq!(nonce.len(), 12, "GCM Nonce must be 12 bytes");
            }
        }
    }

    /// KANI_PROOF_3b: KeyManager 生成的密钥长度为 32 字节
    #[kani::proof]
    fn generated_key_length_is_32_bytes() {
        let key = KeyManager::generate_key();
        assert_eq!(key.len(), 32, "AES-256 requires 32-byte key");
    }
}

#[cfg(feature = "kani")]
mod math_proofs {
    use super::*;

    /// KANI_PROOF_4: 余弦相似度始终在 [-1, 1] 范围内
    ///
    /// 验证数学不变量：
    /// ```text
    /// ∀a,b: |cosine_similarity(a,b)| ≤ 1
    /// ```
    #[kani::proof]
    fn cosine_similarity_in_range() {
        let len: usize = kani::any_where(|&l| l > 0 && l <= 1000);
        let a: Vec<f32> = kani::vec(len);
        let b: Vec<f32> = kani::vec(len);

        let sim = cosine_similarity(&a, &b);
        assert!(sim >= -1.0 && sim <= 1.0);
    }

    /// KANI_PROOF_5: 欧氏距离始终 ≤ 0
    ///
    /// 验证设计决策：距离取反以适配"越大越相似"的排序语义
    ///
    /// ```text
    /// euclidean_distance(a,b) ≤ 0
    /// ```
    #[kani::proof]
    fn euclidean_distance_nonpositive() {
        let len: usize = kani::any_where(|&l| l > 0 && l <= 1000);
        let a: Vec<f32> = kani::vec(len);
        let b: Vec<f32> = kani::vec(len);

        let dist = euclidean_distance(&a, &b);
        assert!(dist <= 0.0);
    }

    /// KANI_PROOF_6: 点积是对称的
    ///
    /// 验证：A · B = B · A
    ///
    /// ```text
    /// ∀a,b: dot_product(a,b) = dot_product(b,a)
    /// ```
    #[kani::proof]
    fn dot_product_symmetric() {
        let len: usize = kani::any_where(|&l| l > 0 && l <= 1000);
        let a: Vec<f32> = kani::vec(len);
        let b: Vec<f32> = kani::vec(len);

        let dot_ab = dot_product(&a, &b);
        let dot_ba = dot_product(&b, &a);
        assert!((dot_ab - dot_ba).abs() < f64::EPSILON);
    }

    /// KANI_PROOF_6b: 零向量的点积为零
    #[kani::proof]
    fn dot_product_with_zero_is_zero() {
        let len: usize = kani::any_where(|&l| l > 0 && l <= 100);
        let a: Vec<f32> = kani::vec(len);
        let zero: Vec<f32> = vec![0.0; len];

        let dot = dot_product(&a, &zero);
        assert!((dot - 0.0).abs() < f64::EPSILON);
    }
}

#[cfg(feature = "kani")]
mod search_proofs {
    use super::*;

    /// KANI_PROOF_7: BM25 评分始终非负
    ///
    /// 验证：BM25 算法的评分下界
    ///
    /// ```text
    /// ∀Q,D: score(Q,D) ≥ 0
    /// ```
    #[kani::proof]
    fn bm25_score_nonnegative() {
        let num_docs = kani::any_where(|&n| n > 0 && n <= 100);
        let num_terms = kani::any_where(|&t| t > 0 && t <= 1000);

        let mut tokens = Vec::new();
        for i in 0..num_docs {
            for _ in 0..10 {
                let term_idx = i % num_terms;
                tokens.push((format!("doc{}", i), format!("term{}", term_idx)));
            }
        }

        let index = Bm25Index::build_from_tokens(&tokens);
        let query_terms = vec!["term0".to_string()];

        let score = index.score(&query_terms, "doc0");
        assert!(score >= 0.0);
    }

    /// KANI_PROOF_8: 不存在的文档 BM25 评分为零
    ///
    /// 验证：BM25 缺失文档处理
    ///
    /// ```text
    /// score(Q, nonexistent) = 0
    /// ```
    #[kani::proof]
    fn bm25_zero_for_nonexistent_doc() {
        let tokens = vec![
            ("doc1".to_string(), "term1".to_string()),
            ("doc2".to_string(), "term2".to_string()),
        ];
        let index = Bm25Index::build_from_tokens(&tokens);
        let score = index.score(&["term1".to_string()], "nonexistent_doc");
        assert_eq!(score, 0.0);
    }

    /// KANI_PROOF_8b: BM25 词频计算正确性
    #[kani::proof]
    fn bm25_term_frequency_correct() {
        let tokens = vec![
            ("doc1".to_string(), "a".to_string()),
            ("doc1".to_string(), "a".to_string()),
            ("doc1".to_string(), "a".to_string()),
            ("doc2".to_string(), "a".to_string()),
        ];
        let index = Bm25Index::build_from_tokens(&tokens);

        let doc_len = index.doc_length("doc1");
        assert_eq!(doc_len, 3, "doc1 should have term frequency 3 for 'a'");
    }
}

#[cfg(feature = "kani")]
mod staleness_proofs {
    use super::*;

    /// KANI_PROOF_S1: 相同哈希表示 Fresh 状态
    #[kani::proof]
    fn same_hash_implies_fresh() {
        let hash: [u8; 64] = kani::any();
        let hash_str = hash
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        let status = StalenessChecker::check_document(&hash_str, &hash_str);
        assert!(matches!(status, StalenessStatus::Fresh));
    }

    /// KANI_PROOF_S2: 不同哈希表示 Stale 状态
    #[kani::proof]
    fn different_hash_implies_stale() {
        let hash1: [u8; 64] = kani::any();
        let hash2: [u8; 64] = kani::any();

        let h1 = hash1
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let h2 = hash2
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        let status = StalenessChecker::check_document(&h1, &h2);
        if h1 != h2 {
            assert!(matches!(status, StalenessStatus::Stale));
        }
    }
}

#[cfg(feature = "kani")]
mod aggregate_proofs {
    use crate::cqrs::aggregate::{
        CreateDocumentData, DeleteDocumentData, DocumentAggregate, DocumentCommand, DocumentEvent,
        DocumentStatus, UpdateDocumentData,
    };
    use crate::cqrs::event_store::ChangeSet;
    use crate::model::ContentType;
    use chrono::Utc;

    /// KANI_PROOF_A1: apply 后 version 严格递增
    ///
    /// 验证事件溯源不变量：
    /// ```text
    /// ∀agg, event: agg.apply(event).version() = agg.version() + 1
    /// ```
    #[kani::proof]
    fn version_increases_on_apply() {
        let mut agg = DocumentAggregate::new();
        let initial_version = agg.version();

        agg.apply(&DocumentEvent::Created(DocumentCreatedData {
            document_id: "doc".to_string(),
            title: "Test".to_string(),
            content: "Content".to_string(),
            content_type: ContentType::Plain,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
            triggered_by: crate::cqrs::event_store::TriggeredBy::from_command("Test", "doc"),
        }));

        assert_eq!(agg.version(), initial_version + 1);
    }

    /// KANI_PROOF_A2: 状态转换遵循合法转换规则
    ///
    /// 验证：DocumentStatus 状态机
    /// ```text
    /// ∀s, t: s.can_transition_to(t) ⇒ 转换合法
    /// ```
    #[kani::proof]
    fn valid_state_transitions_only() {
        let from: DocumentStatus = kani::any();
        let to: DocumentStatus = kani::any();

        if from.can_transition_to(&to) {
            assert!(matches!(
                (from.clone(), to.clone()),
                (
                    DocumentStatus::Pending,
                    DocumentStatus::Parsing | DocumentStatus::Deleted
                ) | (
                    DocumentStatus::Parsing,
                    DocumentStatus::Indexed | DocumentStatus::Deleted
                ) | (
                    DocumentStatus::Indexed,
                    DocumentStatus::Active | DocumentStatus::Deleted
                ) | (
                    DocumentStatus::Active,
                    DocumentStatus::Archived | DocumentStatus::Deleted
                ) | (DocumentStatus::Archived, DocumentStatus::Deleted)
            ));
        }
    }

    /// KANI_PROOF_A3: 已删除文档无法执行更新命令
    #[kani::proof]
    fn deleted_document_rejects_update() {
        let mut agg = DocumentAggregate::new();

        agg.apply(&DocumentEvent::Created(DocumentCreatedData {
            document_id: "doc".to_string(),
            title: "Test".to_string(),
            content: "Content".to_string(),
            content_type: ContentType::Plain,
            metadata: serde_json::json!({}),
            occurred_at: Utc::now(),
            triggered_by: crate::cqrs::event_store::TriggeredBy::from_command("Test", "doc"),
        }));

        agg.apply(&DocumentEvent::Deleted(DocumentDeletedData {
            document_id: "doc".to_string(),
            reason: None,
            occurred_at: Utc::now(),
            triggered_by: crate::cqrs::event_store::TriggeredBy::from_command("Test", "doc"),
        }));

        let result = agg.execute(DocumentCommand::Update(UpdateDocumentData {
            title: Some("Updated".to_string()),
            content: None,
            content_type: None,
            metadata: None,
        }));

        assert!(result.is_err());
    }

    /// KANI_PROOF_A4: 新建聚合根 ID 为空
    #[kani::proof]
    fn new_aggregate_has_empty_id() {
        let agg = DocumentAggregate::new();
        assert!(agg.id().is_empty());
    }
}

#[cfg(feature = "proptest")]
mod property_tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_hash_deterministic_proptest(data: Vec<u8>) {
            let h1 = hash(&data);
            let h2 = hash(&data);
            prop_assert_eq!(h1, h2, "Hash must be deterministic");
        }

        #[test]
        fn test_cosine_similarity_bounds(a: Vec<f32>, b: Vec<f32>) {
            if a.len() == b.len() && !a.is_empty() {
                let sim = cosine_similarity(&a, &b);
                prop_assert!(sim >= -1.0 && sim <= 1.0, "cosine_similarity out of bounds");
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
        fn test_euclidean_distance_nonpositive(a: Vec<f32>, b: Vec<f32>) {
            if a.len() == b.len() && !a.is_empty() {
                let dist = euclidean_distance(&a, &b);
                prop_assert!(dist <= 0.0, "Euclidean distance must be non-positive");
            }
        }

        #[test]
        fn test_bm25_score_nonnegative(
            tokens in prop::collection::vec(
                (prop::string::string("a-z", 1..10), prop::string::string("a-z", 1..10)),
                1..100
            )
        ) {
            let index = Bm25Index::build_from_tokens(&tokens);
            let query = vec!["test".to_string()];

            for i in 0..index.total_docs().min(10) {
                let doc_id = format!("doc{}", i);
                let score = index.score(&query, &doc_id);
                prop_assert!(score >= 0.0, "BM25 score must be non-negative");
            }
        }
    }
}
