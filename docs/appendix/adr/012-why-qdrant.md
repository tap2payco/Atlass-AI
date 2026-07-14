# ADR-012: Why Qdrant

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Technology, Database, AI

---

## Context

Atlas relies heavily on Retrieval-Augmented Generation (RAG) to provide context to the AI agents. The system needs to store and quickly search millions of vector embeddings representing source code, documentation, and agent memories.

## Decision

**We will use Qdrant as the primary vector database.**

## Alternatives Considered

### Alternative 1: ChromaDB

**Pros:** Very popular in the Python AI ecosystem, easy to embed.
**Cons:** Written in Python/C++, which adds friction when integrating with Atlas's Rust core. Performance under heavy load is historically less predictable than native Rust solutions.

### Alternative 2: pgvector (PostgreSQL Extension)

**Pros:** Consolidates all data into one database system (assuming PostgreSQL is used for relational data).
**Cons:** Vector search performance lags behind dedicated vector databases; managing HNSW indexes in Postgres can be operationally complex; limits our ability to use SQLite easily for the local lightweight option.

### Alternative 3: Milvus

**Pros:** Enterprise-grade scalability.
**Cons:** Requires a complex microservices architecture (Docker, Etcd, MinIO) which completely violates our zero-config, local-first requirement.

## Rationale

Qdrant is selected because:
1. **Rust Native:** Qdrant is written in Rust. Because Atlas's core is Rust, we can compile Qdrant natively into our application or communicate with it via highly optimized, native Rust clients without FFI overhead.
2. **Local/Embedded Mode:** Qdrant can run in a fully embedded mode (writing directly to local disk) without requiring a separate server process. This is critical for the local-first, zero-configuration installation requirement.
3. **Performance:** Utilizes HNSW (Hierarchical Navigable Small World) indexing combined with scalar and product quantization, allowing it to search massive vector spaces with very low memory footprints—essential when RAM must be reserved for LLMs.
4. **Filtering:** Excellent support for complex payload filtering alongside vector similarity, which is required for scoping searches to specific projects, files, or memory types.

## Consequences

### Positive
- Blazing fast vector retrieval integrated seamlessly with the Rust core.
- Low resource overhead due to quantization.
- No external server dependencies for end-users.

### Negative
- Introduces a second persistence technology (alongside relational storage), meaning data must be kept in sync between the relational DB (metadata) and Qdrant (vectors).

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | Runs embedded in the local process. |
| EP-06: Memory is Architecture| ✅ | Provides the foundational capability for semantic memory. |

## Related Documents

- [Technology Selection Framework](../08-governance/technology-selection-framework.md)
- [RAG Architecture](../../03-architecture/rag-architecture.md)
