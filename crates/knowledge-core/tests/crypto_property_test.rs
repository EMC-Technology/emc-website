use proptest::prelude::*;
use knowledge_core::{hash, hash_str, Encryptor, Decryptor, KeyManager};

proptest! {
    #[test]
    fn proptest_hash_determinism(data: Vec<u8>) {
        let h1 = hash(&data);
        let h2 = hash(&data);
        prop_assert_eq!(h1, h2, "相同输入的哈希值必须一致");
    }
}

proptest! {
    #[test]
    fn proptest_hash_str_determinism(s: String) {
        let h1 = hash_str(&s);
        let h2 = hash_str(&s);
        prop_assert_eq!(h1, h2, "相同字符串的哈希值必须一致");
    }
}

proptest! {
    #[test]
    fn proptest_hash_different_inputs_differ(a: Vec<u8>, b: Vec<u8>) {
        prop_assume!(a != b);
        let h1 = hash(&a);
        let h2 = hash(&b);
        prop_assert_ne!(h1, h2, "不同输入的哈希值应不同（概率性，碰撞极低）");
    }
}

proptest! {
    #[test]
    fn proptest_hash_str_consistent_with_hash(s: String) {
        let h1 = hash_str(&s);
        let h2 = hash(s.as_bytes());
        prop_assert_eq!(h1, h2, "hash_str 应与 hash(bytes) 结果一致");
    }
}

proptest! {
    #[test]
    fn proptest_encrypt_decrypt_roundtrip(plaintext: Vec<u8>) {
        let key = KeyManager::generate_key();
        let encryptor = Encryptor::new(key).expect("Encryptor 创建不应失败");
        let decryptor = Decryptor::new(key).expect("Decryptor 创建不应失败");

        let (nonce, ciphertext) = encryptor.encrypt(&plaintext).expect("加密不应失败");
        let decrypted = decryptor.decrypt(&nonce, &ciphertext).expect("解密不应失败");

        prop_assert_eq!(decrypted, plaintext, "解密结果应与原始明文一致");
    }
}

proptest! {
    #[test]
    fn proptest_encrypt_produces_different_ciphertexts(plaintext: Vec<u8>) {
        prop_assume!(!plaintext.is_empty());
        let key = KeyManager::generate_key();
        let encryptor = Encryptor::new(key).expect("Encryptor 创建不应失败");

        let (nonce1, ct1) = encryptor.encrypt(&plaintext).expect("加密不应失败");
        let (nonce2, ct2) = encryptor.encrypt(&plaintext).expect("加密不应失败");

        prop_assert_ne!(nonce1, nonce2, "每次加密应生成不同的 nonce");
        prop_assert_ne!(ct1, ct2, "相同明文的不同加密应产生不同密文");
    }
}

proptest! {
    #[test]
    fn proptest_decrypt_with_wrong_key_fails(plaintext: Vec<u8>) {
        prop_assume!(!plaintext.is_empty());
        let key1 = KeyManager::generate_key();
        let key2 = KeyManager::generate_key();
        prop_assume!(key1 != key2);

        let encryptor = Encryptor::new(key1).expect("Encryptor 创建不应失败");
        let decryptor = Decryptor::new(key2).expect("Decryptor 创建不应失败");

        let (nonce, ciphertext) = encryptor.encrypt(&plaintext).expect("加密不应失败");
        let result = decryptor.decrypt(&nonce, &ciphertext);

        prop_assert!(result.is_err(), "使用错误密钥解密应失败");
    }
}

proptest! {
    #[test]
    fn proptest_decrypt_with_corrupted_ciphertext_fails(plaintext: Vec<u8>) {
        prop_assume!(!plaintext.is_empty());
        let key = KeyManager::generate_key();
        let encryptor = Encryptor::new(key).expect("Encryptor 创建不应失败");
        let decryptor = Decryptor::new(key).expect("Decryptor 创建不应失败");

        let (nonce, mut ciphertext) = encryptor.encrypt(&plaintext).expect("加密不应失败");

        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }

        let result = decryptor.decrypt(&nonce, &ciphertext);
        prop_assert!(result.is_err(), "使用损坏密文解密应失败");
    }
}
