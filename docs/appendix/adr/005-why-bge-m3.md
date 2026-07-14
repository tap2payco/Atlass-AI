# ADR-005: Why BAAI BGE-M3

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** AI, Technology, Data Management

---

## Context

Atlas requires persistent memory and the ability to understand large codebases. This necessitates a semantic search system (RAG). To power this, we need an embedding model to convert code, documentation, and memory entries into vector representations.

## Decision

**We will use the BAAI BGE-M3 model for all text and code embeddings.**

## Alternatives Considered

### Alternative 1: nomic-embed-text

**Pros:** Highly efficient, excellent performance for English text.
**Cons:** Less optimized for multilingual scenarios and specifically for code compared to newer models. Shorter maximum context window.

### Alternative 2: OpenAI text-embedding-3-small

**Pros:** Industry standard, excellent performance.
**Cons:** Violates EP-01 (Local-First). Requires network calls and API keys.

### Alternative 3: CodeBERT / GraphCodeBERT

**Pros:** Specifically trained on code.
**Cons:** Older architecture, shorter context windows, and less effective at embedding natural language (which is needed for memory entries and PRDs).

## Rationale

BAAI BGE-M3 is selected because:
1. **M3 Capabilities:** It excels in Multi-linguality (great for various programming languages and human languages), Multi-granularity (handles short queries and long documents equally well), and Multi-Functionality (supports dense retrieval, sparse retrieval, and multi-vector retrieval).
2. **Context Length:** Supports up to 8192 tokens, allowing entire files or large chunks of documentation to be embedded as a single vector without excessive splitting.
3. **Performance:** Consistently ranks at the top of the MTEB (Massive Text Embedding Benchmark) for open-source models of its size.
4. **Efficiency:** Small enough (~2.2GB) to run entirely on the CPU or consume minimal VRAM, leaving resources free for Qwen and DeepSeek.

## Consequences

### Positive
- High-quality semantic retrieval for code and documentation.
- Capable of running efficiently in the background during indexing.
- Long context support simplifies chunking strategies.

### Negative
- Generates 1024-dimensional vectors, which require more storage space in the vector database compared to smaller embedding models (e.g., 384 or 768 dims).

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | Runs completely locally. |
| EP-06: Memory Is Architecture | ✅ | Foundational component of the memory and RAG systems. |

## Related Documents

- [ADR-002: Why Three Models](002-why-three-models.md)
- [RAG Architecture](../../03-architecture/rag-architecture.md)
- [Memory System](../../03-architecture/memory-system.md)
