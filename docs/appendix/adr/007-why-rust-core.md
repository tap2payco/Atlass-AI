# ADR-007: Why Rust for Core Orchestration

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Technology, Architecture

---

## Context

Atlas needs a primary programming language for its core orchestration, execution engine, memory management, and system interactions. The core must coordinate multiple parallel agents, handle complex state machines, manage local resources (filesystem, processes), and present a unified API to the desktop shell.

## Decision

**We will use Rust for the core orchestration layer and system infrastructure.**

## Alternatives Considered

### Alternative 1: Go

**Pros:** Excellent concurrency model (goroutines), fast compilation, single binary deployment.
**Cons:** Garbage collection pauses can affect predictable latency; lacks the deep native integration with desktop frameworks like Tauri (which is Rust-native).

### Alternative 2: TypeScript (Node.js)

**Pros:** Massive ecosystem, same language as the frontend, fast iteration speed.
**Cons:** Single-threaded event loop is poorly suited for CPU-intensive orchestration and parallel data processing; requires a runtime (Node or Deno), increasing distribution size.

### Alternative 3: Python

**Pros:** The lingua franca of AI, massive ecosystem for LLM interaction.
**Cons:** Poor concurrency (GIL), slow performance, difficult to distribute as a single standalone executable without bundling a massive runtime environment.

## Rationale

Rust is selected because:
1. **Performance and Reliability:** Zero-cost abstractions and the ownership model guarantee memory safety and thread safety without a garbage collector. This eliminates an entire class of concurrency bugs critical in a multi-agent system.
2. **Local-First Distribution:** Rust compiles to a small, self-contained native binary for Windows, macOS, and Linux, making installation trivial.
3. **Tauri Integration:** As established in [ADR-009](009-why-tauri.md), we are using Tauri for the desktop shell. Tauri's backend is written in Rust, allowing native, seamless integration between the UI and the orchestrator.
4. **Predictable Resource Usage:** In a local environment where AI models consume massive amounts of RAM and VRAM, the orchestrator must have a minimal and predictable memory footprint. Rust excels here.

## Consequences

### Positive
- Extremely stable core system free of data races.
- Low overhead, leaving system resources available for AI inference.
- Seamless integration with the desktop shell.

### Negative
- **Steeper Learning Curve:** Rust has a notoriously steep learning curve, which may slow down onboarding for new contributors.
- **Slower Compilation:** Compile times are longer than Go or TypeScript, impacting the inner development loop.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-05: Fail Safe | ✅ | Rust's type system and error handling (Result/Option) enforce explicit failure handling. |

## Related Documents

- [Technology Selection Framework](../08-governance/technology-selection-framework.md)
- [System Architecture](../../03-architecture/system-architecture.md)
- [ADR-008: Why Python for AI Runtime](008-why-python-ai-runtime.md)
- [ADR-009: Why Tauri](009-why-tauri.md)
