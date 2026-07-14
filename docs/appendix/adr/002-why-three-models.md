# ADR-002: Why Three Models

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** AI, Architecture

---

## Context

Atlas's multi-agent architecture requires AI model(s). The question is how many distinct models to use and how to allocate them across agent responsibilities.

## Decision

**We will use exactly three models, each optimized for a specific cognitive domain:**

| Model | Domain | Agents Using It |
|---|---|---|
| **Qwen3-Coder** | Code generation, implementation | Backend Engineer, Frontend Engineer, Mobile Engineer, DevOps Engineer, Database Engineer |
| **DeepSeek R1** | Reasoning, planning, analysis | Planner, Architect, QA Engineer, Security Engineer, Code Review |
| **BAAI BGE-M3** | Embeddings, retrieval | Repository Intelligence, Memory Agent, RAG Pipeline |

## Rationale

Three models is the optimal number because:

1. **Cognitive specialization** — Code generation, logical reasoning, and semantic understanding are fundamentally different tasks that benefit from specialized models.
2. **Resource efficiency** — Three models can be managed on consumer hardware with model swapping. More models would exceed typical hardware capabilities.
3. **Clear allocation** — Every agent maps cleanly to one of three domains, minimizing ambiguity.
4. **Practical constraint** — This is the maximum number of models that can be loaded/swapped on 16GB VRAM hardware.

## Why not two models?

Combining reasoning and generation into one model (e.g., using DeepSeek R1 for everything) sacrifices code generation quality. Dedicated coding models consistently outperform reasoning models at code generation benchmarks.

## Why not four or more?

Each additional model adds VRAM pressure, management complexity, and model-swapping latency. The improvement from a fourth model (e.g., a separate security analysis model) does not justify the resource cost. Specialized prompting within DeepSeek R1 achieves adequate security analysis.

## Consequences

### Positive
- Optimal quality for each cognitive domain
- Manageable on consumer hardware (with swapping)
- Clear model-to-agent mapping reduces confusion

### Negative
- Model swapping adds latency when switching between domains
- Three models consume more disk space than one (~20-40 GB total for quantized versions)
- Model version synchronization (ensuring compatible versions of all three)

## Related Documents

- [ADR-001: Multi-Agent Architecture](001-multi-agent-architecture.md)
- [ADR-003: Why Qwen3-Coder](003-why-qwen3-coder.md)
- [ADR-004: Why DeepSeek R1](004-why-deepseek-r1.md)
- [ADR-005: Why BGE-M3](005-why-bge-m3.md)
