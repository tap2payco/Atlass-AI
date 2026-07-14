# ADR-015: Why Ollama

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Technology, Infrastructure

---

## Context

Atlas requires a local inference server to run Qwen3-Coder and DeepSeek R1. The server must manage hardware acceleration (CUDA, Metal, ROCm), handle model quantization, manage model swapping in memory, and expose an API for the AI runtime to query.

## Decision

**We will use Ollama as the primary development and default production inference server, integrated via an abstraction layer (Adapter Pattern).**

## Alternatives Considered

### Alternative 1: llama.cpp (Direct Integration)

**Pros:** Maximum performance, lowest level control, supports almost any hardware.
**Cons:** Requires manual compilation for different hardware backends; managing model downloads and formats (GGUF) is a manual burden for the end user; lacks a built-in REST API (though `llama-server` exists, it's barebones).

### Alternative 2: vLLM

**Pros:** Unmatched throughput, PagedAttention, continuous batching.
**Cons:** Primarily optimized for Linux and datacenter GPUs; complex to set up on Windows/macOS laptops; overkill for a single-user local tool where batch size is typically 1.

### Alternative 3: LocalAI

**Pros:** OpenAI-compatible API out of the box.
**Cons:** Architecture has many moving parts; slower community growth compared to Ollama.

## Rationale

Ollama is selected as the default because:
1. **Developer Experience:** It provides the Apple-like "it just works" experience for local AI. Running `ollama run qwen-coder` handles download, hardware detection, and serving automatically.
2. **Cross-Platform:** Pre-compiled binaries for Windows, macOS (Apple Silicon), and Linux that automatically detect and utilize the best available GPU backend.
3. **Community & Ecosystem:** The largest and fastest-growing community in local AI. It has a vast model library and excellent integration tooling.
4. **API:** Exposes a clean, reliable REST API that our Python AI runtime can easily consume.

## Why the Adapter Pattern?

While Ollama is the default, inference technology moves rapidly. By strictly enforcing **AP-02 (Hexagonal Architecture)**, Ollama is just one adapter implementing the `InferencePort`. If vLLM becomes trivial to install locally, or if a user demands raw `llama.cpp` for ultimate performance, they can swap the adapter in configuration without changing any agent logic.

## Consequences

### Positive
- Trivially easy onboarding for new Atlas users.
- Cross-platform GPU acceleration without users needing to configure CUDA/Metal manually.
- Clean model management.

### Negative
- Ollama is essentially a wrapper around `llama.cpp`. We incur a slight performance and memory overhead compared to running `llama.cpp` directly.
- We are dependent on Ollama's release cycle for support of new model architectures.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | Standard tool for local inference. |
| AP-02: Hexagonal Arch | ✅ | Hidden behind an InferenceAdapter. |

## Related Documents

- [Technology Selection Framework](../08-governance/technology-selection-framework.md)
- [System Architecture](../../03-architecture/system-architecture.md)
