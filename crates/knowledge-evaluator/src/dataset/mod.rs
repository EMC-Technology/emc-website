//! 数据集加载器模块
//!
//! 详见文档: §5 | 用例: UC-042

pub mod sample;

pub use sample::{GoldenSample, SampleDifficulty};

use std::path::Path;

use crate::error::{self, Result};

/// Golden Dataset
///
/// 详见文档: §5.1 | 用例: UC-042
#[derive(Debug, Clone)]
pub struct GoldenDataset {
    /// 数据集 ID
    pub dataset_id: String,
    /// 数据集描述
    pub description: String,
    /// 数据集版本
    pub version: String,
    /// 样本列表
    pub samples: Vec<GoldenSample>,
    /// 创建时间
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl GoldenDataset {
    /// 从 JSON 文件加载数据集
    ///
    /// 详见文档: §5.1 | 用例: UC-042 | 方法: M-058
    ///
    /// # Errors
    ///
    /// 当文件读取或 JSON 解析失败时返回错误
    pub fn from_json(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| error::dataset_load_error(&e))?;
        Self::from_json_str(&content)
    }

    /// 从 JSON 字符串加载数据集
    ///
    /// # Errors
    ///
    /// 当 JSON 解析失败时返回错误
    pub fn from_json_str(content: &str) -> Result<Self> {
        #[derive(serde::Deserialize)]
        struct DatasetFile {
            #[serde(default)]
            dataset_id: Option<String>,
            #[serde(default, alias = "id")]
            id: Option<String>,
            #[serde(default)]
            description: String,
            version: String,
            samples: Vec<GoldenSample>,
            #[serde(default)]
            created_at: Option<chrono::DateTime<chrono::Utc>>,
        }

        let data: DatasetFile = serde_json::from_str(content)?;
        let dataset_id = data.dataset_id.or(data.id).ok_or_else(|| {
            error_core::helpers::validation_error("数据集 ID 不能为空", "load_dataset")
        })?;
        Ok(Self {
            dataset_id,
            description: data.description,
            version: data.version,
            samples: data.samples,
            created_at: data.created_at.unwrap_or_else(chrono::Utc::now),
        })
    }

    /// 从 YAML 文件加载数据集
    ///
    /// 详见文档: §5.1 | 用例: UC-042 | 方法: M-059
    ///
    /// # Errors
    ///
    /// 当文件读取或 YAML 解析失败时返回错误
    pub fn from_yaml(path: &Path) -> Result<Self> {
        #[derive(serde::Deserialize)]
        struct DatasetFile {
            #[serde(default)]
            dataset_id: Option<String>,
            #[serde(default, alias = "id")]
            id: Option<String>,
            #[serde(default)]
            description: String,
            version: String,
            samples: Vec<GoldenSample>,
            #[serde(default)]
            created_at: Option<chrono::DateTime<chrono::Utc>>,
        }

        let content = std::fs::read_to_string(path).map_err(|e| error::dataset_load_error(&e))?;
        let data: DatasetFile = serde_yaml::from_str(&content).map_err(|e| {
            error::dataset_load_error(&std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;
        let dataset_id = data.dataset_id.or(data.id).ok_or_else(|| {
            error_core::helpers::validation_error("数据集 ID 不能为空", "load_dataset")
        })?;
        Ok(Self {
            dataset_id,
            description: data.description,
            version: data.version,
            samples: data.samples,
            created_at: data.created_at.unwrap_or_else(chrono::Utc::now),
        })
    }

    /// 按难度过滤样本
    ///
    /// 详见文档: §5.1 | 用例: UC-042 | 方法: M-060
    #[must_use]
    pub fn filter_by_difficulty(&self, difficulty: SampleDifficulty) -> Vec<&GoldenSample> {
        self.samples
            .iter()
            .filter(|s| s.difficulty == difficulty)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_json_str() {
        let json = r#"{
            "id": "test-ds",
            "version": "1.0",
            "samples": [
                {
                    "id": "s1",
                    "query": "What is Rust?",
                    "expected_answer": "A systems language",
                    "context_ids": ["c1"],
                    "difficulty": "easy",
                    "category": "programming",
                    "metadata": {}
                }
            ]
        }"#;

        let ds = GoldenDataset::from_json_str(json).unwrap();
        assert_eq!(ds.dataset_id, "test-ds");
        assert_eq!(ds.samples.len(), 1);
        assert_eq!(ds.samples[0].query, "What is Rust?");
    }

    #[test]
    fn test_from_json_str_with_dataset_id() {
        let json = r#"{
            "dataset_id": "test-ds-2",
            "description": "Test dataset",
            "version": "2.0",
            "samples": []
        }"#;

        let ds = GoldenDataset::from_json_str(json).unwrap();
        assert_eq!(ds.dataset_id, "test-ds-2");
        assert_eq!(ds.description, "Test dataset");
    }

    #[test]
    fn test_from_json_str_invalid() {
        let json = "{invalid}";
        let result = GoldenDataset::from_json_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_by_difficulty() {
        let json = r#"{
            "id": "test-ds",
            "version": "1.0",
            "samples": [
                {
                    "id": "s1",
                    "query": "Q1",
                    "expected_answer": "A1",
                    "context_ids": [],
                    "difficulty": "easy",
                    "category": "cat1",
                    "metadata": {}
                },
                {
                    "id": "s2",
                    "query": "Q2",
                    "expected_answer": "A2",
                    "context_ids": [],
                    "difficulty": "hard",
                    "category": "cat2",
                    "metadata": {}
                },
                {
                    "id": "s3",
                    "query": "Q3",
                    "expected_answer": "A3",
                    "context_ids": [],
                    "difficulty": "easy",
                    "category": "cat1",
                    "metadata": {}
                }
            ]
        }"#;

        let ds = GoldenDataset::from_json_str(json).unwrap();
        let easy = ds.filter_by_difficulty(SampleDifficulty::Easy);
        assert_eq!(easy.len(), 2);
        assert_eq!(easy[0].id, "s1");
        assert_eq!(easy[1].id, "s3");

        let hard = ds.filter_by_difficulty(SampleDifficulty::Hard);
        assert_eq!(hard.len(), 1);
        assert_eq!(hard[0].id, "s2");

        let expert = ds.filter_by_difficulty(SampleDifficulty::Expert);
        assert!(expert.is_empty());
    }

    #[test]
    fn test_from_json_file_success() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("dataset.json");
        let json = r#"{
            "id": "file-ds",
            "description": "File loaded dataset",
            "version": "1.0",
            "samples": [
                {
                    "id": "s1",
                    "query": "Q1",
                    "expected_answer": "A1",
                    "context_ids": [],
                    "difficulty": "easy",
                    "category": "cat1",
                    "metadata": {}
                }
            ]
        }"#;
        std::fs::write(&file_path, json).unwrap();

        let ds = GoldenDataset::from_json(&file_path).unwrap();
        assert_eq!(ds.dataset_id, "file-ds");
        assert_eq!(ds.description, "File loaded dataset");
        assert_eq!(ds.samples.len(), 1);
    }

    #[test]
    fn test_from_json_file_not_found() {
        let result = GoldenDataset::from_json(Path::new("/nonexistent/path/dataset.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_from_json_str_missing_id() {
        let json = r#"{
            "version": "1.0",
            "samples": []
        }"#;
        let result = GoldenDataset::from_json_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_yaml_file_success() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("dataset.yaml");
        let yaml = r#"
id: yaml-ds
description: YAML loaded dataset
version: "1.0"
samples:
  - id: s1
    query: "What is Rust?"
    expected_answer: "A systems language"
    context_ids:
      - c1
    difficulty: easy
    category: programming
    metadata: {}
"#;
        std::fs::write(&file_path, yaml).unwrap();

        let ds = GoldenDataset::from_yaml(&file_path).unwrap();
        assert_eq!(ds.dataset_id, "yaml-ds");
        assert_eq!(ds.description, "YAML loaded dataset");
        assert_eq!(ds.samples.len(), 1);
        assert_eq!(ds.samples[0].query, "What is Rust?");
    }

    #[test]
    fn test_from_yaml_file_not_found() {
        let result = GoldenDataset::from_yaml(Path::new("/nonexistent/path/dataset.yaml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_from_yaml_invalid_content() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("bad.yaml");
        std::fs::write(&file_path, "{{invalid yaml:::}").unwrap();

        let result = GoldenDataset::from_yaml(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_yaml_missing_id() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("no_id.yaml");
        let yaml = r#"
version: "1.0"
samples: []
"#;
        std::fs::write(&file_path, yaml).unwrap();

        let result = GoldenDataset::from_yaml(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_yaml_with_dataset_id_field() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("with_dataset_id.yaml");
        let yaml = r#"
dataset_id: ds-via-dataset-id
version: "2.0"
samples: []
"#;
        std::fs::write(&file_path, yaml).unwrap();

        let ds = GoldenDataset::from_yaml(&file_path).unwrap();
        assert_eq!(ds.dataset_id, "ds-via-dataset-id");
    }

    #[test]
    fn test_from_yaml_with_created_at() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("with_created_at.yaml");
        let yaml = r#"
id: ds-with-ts
version: "1.0"
samples: []
created_at: "2025-01-01T00:00:00Z"
"#;
        std::fs::write(&file_path, yaml).unwrap();

        let ds = GoldenDataset::from_yaml(&file_path).unwrap();
        assert_eq!(ds.dataset_id, "ds-with-ts");
        assert!(ds.created_at.timestamp() > 0);
    }

    #[test]
    fn test_from_json_str_with_created_at() {
        let json = r#"{
            "id": "ds-ts",
            "version": "1.0",
            "samples": [],
            "created_at": "2025-06-15T12:00:00Z"
        }"#;
        let ds = GoldenDataset::from_json_str(json).unwrap();
        assert_eq!(ds.dataset_id, "ds-ts");
        assert!(ds.created_at.timestamp() > 0);
    }
}
