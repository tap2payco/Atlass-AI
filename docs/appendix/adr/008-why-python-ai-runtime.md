# ADR-008: Why Python for AI Runtime

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Technology, AI

---

## Context

While Rust handles the core orchestration ([ADR-007](007-why-rust-core.md)), the system needs to interface deeply with AI models, perform advanced prompt engineering, handle complex text parsing, and execute agent logic. 

## Decision

**We will use Python for the AI runtime layer, managing agent prompts, AI tool definitions, and reasoning pipelines.** 

This Python layer will run as a managed subprocess communicating with the Rust core via defined IPC/RPC boundaries.

## Alternatives Considered

### Alternative 1: Pure Rust (No Python)

**Pros:** Single language codebase, easier deployment (one binary), maximum performance.
**Cons:** The AI ecosystem in Rust is immature. Re-implementing prompt templates, LangChain-like pipelines, chunking algorithms, and evaluation frameworks in Rust would require massive effort and isolate Atlas from community innovations.

### Alternative 2: TypeScript

**Pros:** Stronger AI ecosystem than Rust (e.g., LangChain.js, LlamaIndex.ts).
**Cons:** Still lags significantly behind Python; introduces a V8 runtime dependency which is heavier than an embedded Python runtime.

## Rationale

Python is selected for the AI runtime because:
1. **Ecosystem Dominance:** Python is the undisputed standard for AI/ML. All new libraries, evaluation frameworks, chunking strategies, and model adapters are released in Python first.
2. **Iteration Speed:** Prompt engineering and agent pipeline tuning require rapid iteration. Python's dynamic nature allows for faster experimentation than Rust's strict compilation model.
3. **Talent Pool:** AI researchers and engineers are predominantly Python developers. Using Python lowers the barrier to entry for contributing to the agent logic.
4. **I/O Bound Nature:** Agent logic is primarily waiting for model inference. The performance penalty of Python is negligible when 99% of the time is spent waiting for the GPU.

## Consequences

### Positive
- Access to the richest AI ecosystem available.
- Faster iteration on prompt engineering and agent logic.
- Easier for AI practitioners to contribute.

### Negative
- **Distribution Complexity:** Shipping Python applications locally requires bundling the runtime (e.g., PyOxidizer, PyInstaller, or embedded Python via PyO3), which is harder than shipping a single Rust binary.
- **Inter-process Communication:** Requires a robust bridge (gRPC or PyO3) between the Rust core and Python runtime, adding architectural complexity.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-02: Specialization | ✅ | Using the right tool for the job: Rust for systems, Python for AI. |
| AP-01: Clean Architecture | ✅ | Enforces a hard boundary between core infrastructure and AI logic. |

## Related Documents

- [Technology Selection Framework](../08-governance/technology-selection-framework.md)
- [System Architecture](../../03-architecture/system-architecture.md)
- [ADR-007: Why Rust for Core](007-why-rust-core.md)
