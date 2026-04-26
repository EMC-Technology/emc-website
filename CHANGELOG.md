# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-26

### Added

- **error-core**: Unified error handling core library with four-dimensional classification fingerprint (ErrorSource × Severity × ImpactScope × Recoverability), global error code registry, type-state Builder, four-layer capture architecture, cause chain + context frame dual-chain propagation, recovery state machine + circuit breaker + deterministic backoff (FNV-1a)
- **knowledge-core**: Core domain logic — BM25 search, hybrid search (RRF fusion), Leiden community detection, knowledge graph construction, CQRS + Event Sourcing, ABAC engine (Cedar-style), PII scanner (11 patterns), AES-256-GCM crypto, multi-level cache (Moka L1 + Redis L2), SurrealDB repository, audit logging
- **knowledge-parser**: Deterministic parsing pipeline — DAG orchestration engine (Kahn topological sort), Markdown pipeline (Comrak + ICU segmenter), Code pipeline (tree-sitter), execution flow tracer, community detection stage
- **knowledge-api**: HTTP/WebSocket/MCP API — GEMMA4-E4B pure Rust inference (42-layer Transformer, Candle framework, 2560-dim), RAG engine (BM25+Vector+Graph three-way recall), ReAct Agent framework, MCP Server (7 Tool + 2 Prompt + Resources), ABAC policy engine, JWT authentication
- **knowledge-frontend**: Dioxus WASM frontend — CRUD operations, graph visualization basics, WebSocket communication
- **spec**: Complete documentation suite — Software Engineering Detailed Design V4.0, Gap Analysis V5.0, Open Source Purpose Statement, 10 ADRs, architecture overview, data model, getting started guide, unsafe audit report
- **Infrastructure**: MIT OR Apache-2.0 dual license (code) + CC-BY-SA 4.0 (knowledge content), DCO contributor agreement, cargo-deny supply chain audit, .gitignore, CI/CD workflows

### Design Philosophy

- **0 randomness, 0 black-box inference**: Deterministic backoff (FNV-1a), deterministic float sorting (f64::total_cmp), Token-level global offset addressing
- **Error-Driven Development**: error-core as first-class infrastructure — error codes before implementation, recovery strategies before failures
- **Process-Driven Development**: UPCM (Universal Process Control Model)先行实践 — DAG orchestration + YAML workflow definition + unified state machine

### Known Limitations

- Vector Store: In-memory adapter uses brute-force search (HNSW params unused); Qdrant adapter is production-ready
- Cross-Encoder Reranker: Pipeline architecture complete, but cross-encoder is Mock (Jaccard similarity), no real neural network
- Frontend: Basic CRUD and graph visualization, not production-ready
- RAG Evaluation: No evaluation framework (RAGAS/LLM-as-Judge) implemented
- error-core: No no_std support yet (roadmap: HashMap→BTreeMap, OnceLock→critical_section, regex→const validation)
- error-core logging: ErrorLoggingLayer, rotate_logs(), configure_logging() are placeholder implementations

[0.1.0]: https://github.com/emc-technology/knowledge-system/releases/tag/v0.1.0
