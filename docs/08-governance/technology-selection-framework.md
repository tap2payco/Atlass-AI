# Technology Selection Framework

> **Version:** 1.0.0  
> **Status:** ✅ Complete  
> **Last Updated:** 2026-07-14  
> **Owner:** Architecture Team  
> **Upstream:** [Decision Framework](decision-framework.md)  
> **ADR References:** [ADR-007](../appendix/adr/007-why-rust-core.md) through [ADR-015](../appendix/adr/015-why-ollama.md)

---

## Purpose

This document defines **how technologies are evaluated** for Atlas and documents **why each current technology was selected** over its alternatives. Every technology comparison is preserved here so future engineers understand the reasoning and can revisit decisions with data.

---

## Evaluation Criteria

Every technology is evaluated against these weighted criteria:

| Criterion | Weight | Description |
|---|---|---|
| **Local-First Compatibility** | 25% | Can it run entirely offline without external services? |
| **Performance** | 20% | Latency, throughput, resource efficiency |
| **Cross-Platform** | 15% | Windows, macOS, Linux support without workarounds |
| **Ecosystem & Community** | 15% | Library availability, community size, documentation quality |
| **Maintainability** | 10% | Code quality support (type safety, error handling, testing) |
| **Maturity & Stability** | 10% | Production track record, release cadence, backward compatibility |
| **Integration Complexity** | 5% | Effort to integrate with existing Atlas stack |

### Scoring

Each criterion is scored 1-5:

| Score | Meaning |
|---|---|
| 5 | Excellent — best-in-class for this criterion |
| 4 | Good — meets all requirements with minor gaps |
| 3 | Adequate — meets requirements but with trade-offs |
| 2 | Weak — significant gaps that require workarounds |
| 1 | Poor — fails to meet requirements |

---

## Technology Decisions

### 1. Core Orchestration Language

**Decision: Rust** → [ADR-007](../appendix/adr/007-why-rust-core.md)

| Criterion (Weight) | Rust | Go | TypeScript (Node) | Python | C++ |
|---|---|---|---|---|---|
| Local-First (25%) | 5 — native binary | 5 — native binary | 3 — requires Node runtime | 2 — requires Python runtime | 5 — native binary |
| Performance (20%) | 5 — zero-cost abstractions | 4 — GC pauses | 2 — single-threaded event loop | 1 — GIL limitations | 5 — maximum control |
| Cross-Platform (15%) | 5 — excellent cross-compilation | 5 — excellent | 4 — needs Node per platform | 3 — dependency hell | 3 — complex build systems |
| Ecosystem (15%) | 4 — growing rapidly, Tauri ecosystem | 4 — strong for services | 5 — massive npm ecosystem | 5 — massive ecosystem | 3 — fragmented |
| Maintainability (10%) | 5 — ownership system prevents bugs | 3 — simple but less safe | 4 — TypeScript types help | 3 — runtime errors | 2 — manual memory management |
| Maturity (10%) | 4 — production-proven (Mozilla, AWS, Discord) | 5 — Google, Docker, K8s | 4 — widely deployed | 5 — decades of use | 5 — decades of use |
| Integration (5%) | 4 — PyO3 for Python, Tauri for frontend | 3 — CGo for extensions | 3 — native addons complex | 4 — easy FFI | 2 — complex FFI |
| **Weighted Score** | **4.65** | **4.20** | **3.25** | **2.90** | **3.85** |

**Why Rust won:**
- Highest weighted score driven by performance, safety, and cross-platform strength
- Natural alignment with Tauri (also Rust)
- Ownership system prevents entire categories of bugs (memory, concurrency)
- PyO3 provides excellent Python interop for AI runtime
- Compiles to single native binary — ideal for local-first distribution

**Why alternatives lost:**
- **Go:** Strong contender but lacks Rust's safety guarantees and Tauri alignment. GC pauses are a concern for real-time orchestration.
- **TypeScript/Node:** Requires a runtime, single-threaded, not ideal for CPU-intensive orchestration
- **Python:** Too slow for core orchestration; GIL prevents true parallelism. Excellent for AI code but wrong for the core.
- **C++:** Unsafe memory management is unacceptable for a system that manages user code and files

---

### 2. AI Runtime Language

**Decision: Python** → [ADR-008](../appendix/adr/008-why-python-ai-runtime.md)

| Criterion (Weight) | Python | Rust | TypeScript | Go |
|---|---|---|---|---|
| Local-First (25%) | 4 — needs runtime but installable | 5 | 3 | 5 |
| Performance (20%) | 2 — slow, GIL | 5 | 3 | 4 |
| Cross-Platform (15%) | 3 — dependency management varies | 5 | 4 | 5 |
| Ecosystem (15%) | 5 — dominant AI/ML ecosystem | 2 — limited AI libs | 3 — growing AI libs | 2 — limited AI libs |
| Maintainability (10%) | 3 — dynamic typing | 5 | 4 | 3 |
| Maturity (10%) | 5 — PyTorch, HuggingFace, LangChain | 2 — nascent AI ecosystem | 3 — emerging | 2 — nascent |
| Integration (5%) | 5 — native for all AI libraries | 3 — PyO3 bridge | 3 — needs bindings | 3 — CGo bridge |
| **Weighted Score** | **3.65** | **4.20** | **3.30** | **3.65** |

**Why Python won despite lower score than Rust:**
- The AI/ML ecosystem is overwhelmingly Python. LangChain, LlamaIndex, HuggingFace Transformers, sentence-transformers — all Python-first.
- Prompt engineering, agent logic, and reasoning pipelines change frequently and benefit from Python's rapid iteration
- Model integration libraries (ollama-python, openai, etc.) are Python-first
- The performance penalty is acceptable because AI runtime is I/O-bound (waiting for model inference), not CPU-bound
- Rust handles the performance-critical core; Python handles the AI-specific logic where ecosystem matters most

**Architecture note:** Python runs as a subprocess managed by the Rust core, communicating via gRPC or IPC. This keeps the Rust core clean while leveraging Python's ecosystem.

---

### 3. Desktop Framework

**Decision: Tauri** → [ADR-009](../appendix/adr/009-why-tauri.md)

| Criterion (Weight) | Tauri | Electron | Flutter | Qt | Native per OS |
|---|---|---|---|---|---|
| Local-First (25%) | 5 | 5 | 5 | 5 | 5 |
| Performance (20%) | 5 — uses system webview, ~10MB | 2 — Chromium, ~150MB | 4 — native rendering | 4 — native widgets | 5 — fully native |
| Cross-Platform (15%) | 5 — Win/Mac/Linux | 5 | 5 | 5 | 1 — three codebases |
| Ecosystem (15%) | 3 — growing, smaller than Electron | 5 — massive ecosystem | 4 — growing Dart ecosystem | 3 — C++ ecosystem | varies |
| Maintainability (10%) | 4 — Rust + React (TypeScript) | 3 — JS + runtime complexity | 4 — Dart type safety | 3 — C++ complexity | 2 — triple maintenance |
| Maturity (10%) | 3 — v2 is stable, smaller track record | 5 — VS Code, Slack, Discord | 4 — production-proven (mobile) | 5 — decades | 5 — each native SDK |
| Integration (5%) | 5 — Rust core integrates natively | 3 — Node bridge to Rust | 3 — FFI bridge | 3 — C++ bridge | 2 — per-platform bridges |
| **Weighted Score** | **4.55** | **3.90** | **4.35** | **4.10** | **3.40** |

**Why Tauri won:**
- System webview means ~10MB binary vs Electron's ~150MB (local-first = ship small)
- Rust backend integrates natively with Atlas's Rust core — no bridge needed
- React frontend uses the same TypeScript/React skills as the web interface
- Memory usage is 5-10x lower than Electron

**Why Electron lost:**
- Bundles an entire Chromium instance — unacceptable for a local-first tool
- Memory usage (300MB+ baseline) conflicts with leaving room for AI models
- The ecosystem advantage doesn't outweigh the resource penalty

---

### 4. Primary Database

**Decision: PostgreSQL (production) + SQLite (local lightweight)** → [ADR-013](../appendix/adr/013-why-postgresql-sqlite.md)

| Criterion (Weight) | PostgreSQL + SQLite | PostgreSQL only | SQLite only | MongoDB | DuckDB |
|---|---|---|---|---|---|
| Local-First (25%) | 5 — SQLite embedded; PG local | 4 — PG needs server process | 5 — fully embedded | 3 — needs server | 5 — embedded |
| Performance (20%) | 5 — PG for complex; SQLite for simple | 5 | 3 — limited concurrency | 3 — document overhead | 4 — analytics-focused |
| Cross-Platform (15%) | 5 | 5 | 5 | 4 | 4 |
| Ecosystem (15%) | 5 — best of both worlds | 5 | 4 — limited tooling | 4 | 2 — niche |
| Maintainability (10%) | 4 — two adapters but well-isolated | 5 — single adapter | 5 | 3 — schema-less challenges | 3 |
| Maturity (10%) | 5 | 5 | 5 | 4 | 3 — newer |
| Integration (5%) | 4 | 4 | 5 — single file | 3 | 3 |
| **Weighted Score** | **4.85** | **4.60** | **4.35** | **3.35** | **3.55** |

**Why dual strategy won:**
- SQLite provides zero-configuration local storage for simple deployments (solo developer)
- PostgreSQL provides enterprise-grade features for teams (concurrent access, advanced queries, JSONB)
- The port/adapter pattern means swapping between them is a configuration change, not a code change
- Most users start with SQLite; teams or advanced users upgrade to PostgreSQL

---

### 5. Vector Database

**Decision: Qdrant** → [ADR-012](../appendix/adr/012-why-qdrant.md)

| Criterion (Weight) | Qdrant | Milvus | Chroma | Weaviate | pgvector |
|---|---|---|---|---|---|
| Local-First (25%) | 5 — embedded mode available | 3 — heavy dependencies | 5 — embedded Python | 3 — needs server | 4 — PG extension |
| Performance (20%) | 5 — Rust-native, HNSW + quantization | 5 — highly optimized | 3 — Python overhead | 4 — Go-native | 3 — PG overhead |
| Cross-Platform (15%) | 5 — Rust binary | 3 — complex deployment | 4 — Python everywhere | 4 — Docker | 4 — wherever PG runs |
| Ecosystem (15%) | 4 — good clients, growing | 4 — enterprise ecosystem | 4 — popular in AI | 4 — GraphQL API | 3 — limited vector tooling |
| Maintainability (10%) | 4 — well-documented API | 3 — complex architecture | 4 — simple API | 4 — well-documented | 3 — SQL-based vector ops |
| Maturity (10%) | 4 — production-proven, growing fast | 4 — CNCF graduated | 3 — newer | 4 — production-proven | 3 — extension model |
| Integration (5%) | 5 — Rust native client | 2 — Python/gRPC | 4 — Python native | 3 — REST/GraphQL | 5 — already using PG |
| **Weighted Score** | **4.70** | **3.65** | **3.95** | **3.65** | **3.50** |

**Why Qdrant won:**
- Written in Rust — natural integration with Atlas's Rust core
- Supports embedded mode (runs in-process, no separate server needed for local-first)
- HNSW + scalar/product quantization provides excellent search quality with low memory
- First-class Rust client eliminates bridge overhead

---

### 6. Repository Parsing

**Decision: Tree-sitter** → [ADR-014](../appendix/adr/014-why-tree-sitter.md)

| Criterion (Weight) | Tree-sitter | LSP | Regex | Custom Parsers | ANTLR |
|---|---|---|---|---|---|
| Local-First (25%) | 5 | 4 — needs language servers | 5 | 5 | 5 |
| Performance (20%) | 5 — incremental, C-native | 3 — process overhead per language | 4 — fast but limited | 4 | 4 |
| Cross-Platform (15%) | 5 | 3 — each LSP has requirements | 5 | 5 | 4 |
| Ecosystem (15%) | 5 — 200+ language grammars | 5 — wide LSP support | 1 — no structure | 1 — per language | 4 — many grammars |
| Maintainability (10%) | 4 — declarative grammars | 3 — multiple processes | 2 — fragile | 1 — massive effort | 3 |
| Maturity (10%) | 5 — GitHub, Neovim, Helix, Zed | 5 — VS Code standard | 5 | varies | 5 |
| Integration (5%) | 5 — C library, Rust bindings | 2 — JSON-RPC per language | 5 | 5 | 3 |
| **Weighted Score** | **4.90** | **3.75** | **3.80** | **3.45** | **4.15** |

**Why Tree-sitter won:**
- Incremental parsing means re-parsing after edits is nearly instant
- 200+ language grammars maintained by the community
- Produces concrete syntax trees that map directly to source code
- Used by GitHub (syntax highlighting, code navigation), Neovim, Helix, Zed — battle-tested
- C library with excellent Rust bindings (tree-sitter crate)

---

### 7. Messaging / Event Bus

**Decision: NATS** → [ADR-011](../appendix/adr/011-why-nats.md)

| Criterion (Weight) | NATS | RabbitMQ | Redis Pub/Sub | ZeroMQ | In-Process (tokio channels) |
|---|---|---|---|---|---|
| Local-First (25%) | 5 — embeddable, single binary | 3 — Erlang runtime | 4 — needs Redis server | 5 — library | 5 — no external dep |
| Performance (20%) | 5 — extremely low latency | 4 — good throughput | 4 — fast | 5 — zero-copy | 5 — in-process |
| Cross-Platform (15%) | 5 — Go binary, everywhere | 4 — Erlang issues on Windows | 4 | 4 | 5 |
| Ecosystem (15%) | 4 — JetStream, KV, Object Store | 5 — extensive plugins | 4 | 3 — low-level | 2 — Rust only |
| Maintainability (10%) | 4 — simple protocol | 3 — complex configuration | 4 | 2 — manual framing | 4 |
| Maturity (10%) | 4 — CNCF, Synadia | 5 — decades | 5 — part of Redis | 4 | 5 — Tokio stable |
| Integration (5%) | 4 — good Rust client (async-nats) | 3 — AMQP complexity | 4 | 3 | 5 — native |
| **Weighted Score** | **4.65** | **3.85** | **4.05** | **4.10** | **4.30** |

**Why NATS won over in-process channels:**
- NATS starts embedded (in-process) for simple deployments but scales to distributed for enterprise
- JetStream provides persistent messaging, which in-process channels cannot
- NATS supports request-reply, pub-sub, and queue groups natively
- When Atlas evolves to distributed agents, NATS requires zero architecture changes
- The in-process start means no performance penalty for local-first usage

**Architecture note:** Atlas starts with an in-process NATS-compatible abstraction layer. When distributed deployment is needed, it swaps to a full NATS server — same API, same semantics, no code changes.

---

### 8. Inference Server

**Decision: Ollama (development) with adapters** → [ADR-015](../appendix/adr/015-why-ollama.md)

| Criterion (Weight) | Ollama | llama.cpp (direct) | vLLM | LocalAI | Hugging Face TGI |
|---|---|---|---|---|---|
| Local-First (25%) | 5 — fully local | 5 — fully local | 4 — local but complex | 5 — fully local | 4 — Docker-based |
| Performance (20%) | 4 — good, not optimal | 5 — maximum control | 5 — PagedAttention, continuous batching | 3 — abstraction overhead | 4 — optimized serving |
| Cross-Platform (15%) | 5 — Win/Mac/Linux, GPU auto-detect | 4 — manual compilation | 3 — Linux-first | 4 — Docker | 3 — Linux/Docker |
| Ecosystem (15%) | 5 — model library, REST API, huge community | 3 — C++ API, limited tooling | 4 — Python API, OpenAI compat | 4 — OpenAI compat | 4 — HF ecosystem |
| Maintainability (10%) | 5 — simple model management | 2 — manual model management | 3 — configuration complexity | 3 — many moving parts | 3 — Docker management |
| Maturity (10%) | 4 — fast-growing, widely adopted | 4 — reference implementation | 4 — production-proven | 3 — smaller community | 4 — HF backing |
| Integration (5%) | 5 — REST API, Python/Rust clients | 3 — C API, language bindings | 4 — OpenAI-compatible API | 4 — REST API | 4 — REST API |
| **Weighted Score** | **4.70** | **4.05** | **4.00** | **3.80** | **3.70** |

**Why Ollama won:**
- Simplest developer experience: `ollama pull model` → ready
- Cross-platform GPU auto-detection eliminates setup friction
- REST API makes integration trivial
- Model library provides easy access to Qwen3-Coder, DeepSeek R1, and embedding models
- The largest local-inference community ensures rapid improvement

**Why the adapter pattern:**
- Ollama is the primary server but Atlas abstracts inference behind an `InferencePort`
- llama.cpp adapter for users who want maximum performance with manual setup
- vLLM adapter for multi-user or high-throughput scenarios
- The adapter pattern ensures Atlas is never locked into a single inference solution

---

## Future Evaluation Process

When evaluating a new technology for Atlas:

1. **Fill in the evaluation matrix** using the criteria and weights above
2. **Document alternatives considered** with their scores
3. **Write an ADR** with the decision and rationale
4. **Update this document** with the new comparison table
5. **Update the EKB** wherever the technology impacts architecture

---

*This framework ensures technology decisions are data-driven and traceable. For the process of making decisions, see [Decision Framework](decision-framework.md). For individual technology decisions, see the [ADR index](../00-index.md#appendix--architecture-decision-records).*
