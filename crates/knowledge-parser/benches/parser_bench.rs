use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use knowledge_parser::pipeline::Pipeline;
use knowledge_parser::text_splitter::TextSplitterBlocker;

const SMALL_MD: &str = r#"# Benchmark Document

This is a **small** markdown document for performance testing.

## Section 1

- Item 1
- Item 2
- Item 3

## Code Example

```rust
fn hello() -> String {
    "Hello, World!".to_string()
}
```

> A blockquote for testing purposes.

| Column 1 | Column 2 |
|----------|----------|
| Data 1   | Data 2   |
"#;

#[allow(clippy::literal_string_with_formatting_args)]
fn generate_large_markdown(size_kb: usize) -> String {
    let base =
        "# Large Document\n\n## Introduction\n\nThis is a generated document for benchmarking.\n\n";
    let paragraph = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.\n\n";
    let code_block = "```rust\nfn process_data(input: &[u8]) -> Result<Vec<u8>> {\n    input.iter().map(|&b| b.wrapping_add(1)).collect()\n}\n```\n\n";
    let table = "| Key | Value |\n|-----|-------|\n| A   | 1     |\n| B   | 2     |\n\n";
    let list = "- First item with **bold** and *italic* text\n- Second item with `code` inline\n- Third item with [link](https://example.com)\n\n";

    let mut output = String::with_capacity(size_kb * 1024);
    output.push_str(base);
    let target_size = size_kb * 1024 - base.len();
    let chunk = format!("### Section {{i}}\n\n{paragraph}{code_block}{table}{list}");
    let mut i = 1;
    while output.len() < target_size {
        let section = chunk.replace("{i}", &i.to_string());
        if output.len() + section.len() > target_size {
            let remaining = target_size - output.len();
            output.push_str(&section[..remaining.min(section.len())]);
            break;
        }
        output.push_str(&section);
        i += 1;
    }
    output
}

#[allow(dead_code)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::literal_string_with_formatting_args)]
fn generate_code_content(lang: &str, size_kb: usize) -> String {
    match lang {
        "rust" => {
            let fn_template = r#"/// Documentation comment for function_{i}
pub fn function_{i}(input: &str) -> Result<String, Box<dyn std::error::Error>> {{
    let processed = input
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(processed)
}}

#[cfg(test)]
mod test_{i} {{
    use super::*;

    #[test]
    fn test_function_{i}() {{
        let result = function_{i}("hello world").unwrap();
        assert!(!result.is_empty());
    }}
}}
"#;
            let mut output = String::with_capacity(size_kb * 1024);
            output.push_str("//! Auto-generated Rust file for benchmarking\n\n");
            let struct_def = r"#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchStruct {
    pub id: u64,
    pub name: String,
    pub data: Vec<u8>,
    pub metadata: Option<serde_json::Value>,
}
";
            output.push_str(struct_def);
            for i in 0.. {
                let fn_code = fn_template.replace("{i}", &i.to_string());
                if output.len() + fn_code.len() > size_kb * 1024 {
                    break;
                }
                output.push_str(&fn_code);
            }
            output
        }
        "python" => {
            let fn_template = r##"""Documentation string for function_{i}."""
def function_{i}(input: str) -> list[str]:
    """Process input and return filtered lines."""
    return [
        line.strip().rstrip()
        for line in input.splitlines()
        if not line.lstrip().startswith("#")
    ]


class TestClass{i}:
    """Test class {i} for benchmarking."""

    def __init__(self, value: int = 0):
        self.value = value
        self._cache: dict[str, Any] = {{}}

    def process(self, data: list[int]) -> int:
        return sum(x * self.value for x in data if x > 0)


if __name__ == "__main__":
    result = function_{i}("hello from python")
    print(f"Result: {{result}}")
"##;
            let mut output = String::with_capacity(size_kb * 1024);
            output.push_str("\"\"\"Auto-generated Python file for benchmarking\n\n");
            for i in 0.. {
                let fn_code = fn_template.replace("{i}", &i.to_string());
                if output.len() + fn_code.len() > size_kb * 1024 {
                    break;
                }
                output.push_str(&fn_code);
            }
            output
        }
        "javascript" => {
            let fn_template = r"
/**
 * Documentation for function{i}
 * @param {{string}} input - The input string to process
 * @returns {{string[]}} Filtered lines
 */
function function{i}(input) {{
    return input
        .split('\n')
        .filter(line => !line.trimStart().startsWith('//'))
        .map(line => line.trim());
}}

class TestClass{i}}{{
    /**
     * Test class {i}
     * @param {{number}} value - Initial value
     */
    constructor(value = 0) {{
        this.value = value;
        this._cache = new Map();
    }}

    /** Process an array of numbers */
    process(data) {{
        return data.filter(x => x > 0).reduce((acc, x) => acc + x * this.value, 0);
    }}
}}

// Export for module usage
module.exports = {{ function{i}, TestClass{i} }};
";
            let mut output = String::with_capacity(size_kb * 1024);
            output.push_str("// Auto-generated JavaScript file for benchmarking\n\n");
            for i in 0.. {
                let fn_code = fn_template.replace("{i}", &i.to_string());
                if output.len() + fn_code.len() > size_kb * 1024 {
                    break;
                }
                output.push_str(&fn_code);
            }
            output
        }
        _ => panic!("不支持的代码语言: {lang}"),
    }
}

fn bench_markdown_parse_comrak(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_markdown_comrak");

    group.bench_function("parse_small_document", |b| {
        b.iter(|| {
            let html =
                comrak::markdown_to_html(black_box(SMALL_MD), &comrak::ComrakOptions::default());
            black_box(html)
        });
    });

    for size_kb in [10usize, 100, 500, 10_000] {
        let md = generate_large_markdown(size_kb);
        group.throughput(Throughput::Bytes(md.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("comrak_parse", format!("{size_kb}KB")),
            &md,
            |b, content| {
                b.iter(|| {
                    let html = comrak::markdown_to_html(
                        black_box(content),
                        &comrak::ComrakOptions::default(),
                    );
                    black_box(html)
                });
            },
        );
    }
    group.finish();
}

fn bench_markdown_render(c: &mut Criterion) {
    use comrak::{Arena, ComrakOptions, parse_document};

    let mut group = c.benchmark_group("parser_markdown_render");

    group.bench_function("render_small_html", |b| {
        b.iter(|| {
            let arena = Arena::new();
            let options = ComrakOptions::default();
            let root = parse_document(&arena, SMALL_MD, &options);
            let mut output = Vec::new();
            let _ = comrak::html::format_document(root, &options, &mut output);
            black_box(output);
        });
    });

    for size_kb in [10usize, 100, 500] {
        let md = generate_large_markdown(size_kb);
        group.throughput(Throughput::Bytes(md.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("render_html", format!("{size_kb}KB")),
            &md,
            |b, content| {
                b.iter(|| {
                    let arena = Arena::new();
                    let options = ComrakOptions::default();
                    let root = parse_document(&arena, content, &options);
                    let mut output = Vec::new();
                    let _ = comrak::html::format_document(root, &options, &mut output);
                    black_box(output);
                });
            },
        );
    }
    group.finish();
}

fn bench_text_chunking(c: &mut Criterion) {
    let splitter = TextSplitterBlocker::new();

    let mut group = c.benchmark_group("parser_text_chunking");

    for chunk_size in [256usize, 512, 1000, 2000] {
        let config_splitter = TextSplitterBlocker::with_config(chunk_size, chunk_size / 5).expect("基准测试配置应合法");

        for text_size_kb in [1usize, 10, 100, 1000] {
            let text = "The quick brown fox jumps over the lazy dog. ".repeat(text_size_kb * 50);

            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(
                    "split_to_blocks",
                    format!("chunk{chunk_size}_{text_size_kb}KB"),
                ),
                &text,
                |b, content| {
                    b.iter(|| {
                        black_box(config_splitter.split_to_blocks(black_box(content)).unwrap());
                    });
                },
            );
        }
    }

    let large_text = generate_large_markdown(10_000);
    group.throughput(Throughput::Bytes(large_text.len() as u64));
    group.bench_with_input("split_10mb_markdown", &large_text, |b, content| {
        b.iter(|| { black_box(splitter.split_to_blocks(black_box(content)).unwrap()); });
    });

    group.finish();
}

fn bench_pipeline_dag_execution(c: &mut Criterion) {
    use knowledge_core::model::{Document, SourceType};

    let pipeline = Pipeline::new();

    let mut group = c.benchmark_group("parser_pipeline_dag");

    group.bench_function("run_empty_pipeline", |b| {
        b.iter(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let input: Vec<Document> = vec![];
            rt.block_on(async { black_box(pipeline.run(black_box(input)).await.unwrap()) });
        });
    });

    for doc_count in [1usize, 10, 100, 1000] {
        let docs: Vec<Document> = (0..doc_count)
            .map(|i| {
                Document::new(
                    format!("/bench/doc_{i}.md"),
                    format!("Document {i}"),
                    SourceType::Markdown,
                    "a".repeat(64),
                )
                .unwrap()
            })
            .collect();

        group.throughput(Throughput::Elements(doc_count as u64));
        group.bench_with_input(
            BenchmarkId::new("run_pipeline", format!("{doc_count}docs")), 
            &docs,
            |b, docs| {
                b.iter(|| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let input: Vec<Document> = docs.clone();
                    rt.block_on(async { black_box(pipeline.run(black_box(input)).await.unwrap()) });
                });
            },
        );
    }
    group.finish();
}

fn bench_blake3_file_hashing(c: &mut Criterion) {
    use blake3::Hasher;

    let mut group = c.benchmark_group("parser_blake3_incremental_hash");

    for size_kb in [1usize, 10, 100, 1000, 10_000] {
        let data = vec![0xCDu8; size_kb * 1024];
        group.throughput(Throughput::Bytes((size_kb * 1024) as u64));
        group.bench_with_input(
            BenchmarkId::new("incremental_hash", format!("{size_kb}KB")), 
            &data,
            |b, data| {
                b.iter(|| {
                    let mut hasher = Hasher::new();
                    hasher.update(black_box(data));
                    black_box(hasher.finalize());
                });
            },
        );
    }
    group.finish();
}

fn bench_source_type_detection(c: &mut Criterion) {
    use knowledge_core::model::SourceType;
    use knowledge_parser::SourceTypeDetector;
    use std::path::Path;

    let paths = [
        ("/path/to/file.md", SourceType::Markdown),
        ("/path/to/document.markdown", SourceType::Markdown),
        ("/src/main.rs", SourceType::Code),
        ("/app/handler.py", SourceType::Code),
        ("/ui/index.js", SourceType::Code),
        ("/data/config.toml", SourceType::Plain),
        ("/readme.txt", SourceType::Plain),
    ];

    let mut group = c.benchmark_group("parser_source_type_detection");

    for (path_str, _) in &paths {
        let name = path_str
            .split('/')
            .next_back()
            .unwrap_or("unknown")
            .split('.')
            .next_back()
            .unwrap_or("none");
        group.bench_function(format!("detect_{name}"), |b| {
            b.iter(|| { black_box(SourceTypeDetector::detect(Path::new(path_str)).unwrap()); });
        });
    }

    group.bench_function("detect_batch_1000_calls", |b| {
        b.iter(|| {
            for (path_str, _) in &paths {
                black_box(SourceTypeDetector::detect(Path::new(path_str)).unwrap());
            }
        });
    });

    group.finish();
}

fn bench_string_operations_for_parsing(c: &mut Criterion) {
    let text = "Hello 世界! This is a mixed 中文+English text for benchmarking tokenization performance. 测试中文分词性能。";

    let mut group = c.benchmark_group("parser_string_ops");

    group.bench_function("char_iteration", |b| {
        b.iter(|| {
            let count = black_box(text).chars().count();
            black_box(count);
        });
    });

    group.bench_function("byte_iteration", |b| {
        b.iter(|| {
            let bytes = black_box(text).len();
            black_box(bytes);
        });
    });

    group.bench_function("whitespace_split", |b| {
        b.iter(|| {
            black_box(black_box(text).split_whitespace().count());
        });
    });

    group.bench_function("line_count", |b| {
        b.iter(|| {
            let lines = black_box(text).lines().count();
            black_box(lines);
        });
    });

    let large_text = generate_large_markdown(100);
    group.throughput(Throughput::Bytes(large_text.len() as u64));

    group.bench_with_input("char_iter_100kb", &large_text, |b, text| {
        b.iter(|| { black_box(black_box(text).chars().count()); });
    });

    group.bench_with_input("whitespace_split_100kb", &large_text, |b, text| {
        b.iter(|| {
            black_box(black_box(text).split_whitespace().count());
        });
    });

    group.bench_with_input("line_count_100kb", &large_text, |b, text| {
        b.iter(|| { black_box(black_box(text).lines().count()); });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_markdown_parse_comrak,
    bench_markdown_render,
    bench_text_chunking,
    bench_pipeline_dag_execution,
    bench_blake3_file_hashing,
    bench_source_type_detection,
    bench_string_operations_for_parsing
);
criterion_main!(benches);
