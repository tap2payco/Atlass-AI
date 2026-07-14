# ADR-003: Why Qwen3-Coder

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** AI, Technology

---

## Context

Atlas needs a primary model for code generation, implementation, and refactoring. As established in [ADR-002](002-why-three-models.md), we are using a specialized model for this domain. The model must run locally and provide state-of-the-art performance for its size.

## Decision

**We will use Qwen3-Coder (specifically the 7B and 32B variants) as the primary code generation model.**

## Alternatives Considered

### Alternative 1: DeepSeek Coder V2

**Pros:** Excellent performance, wide context window
**Cons:** High resource requirements for local inference; slightly edge out by Qwen in some latest benchmarks for pure generation speed vs quality ratio.

### Alternative 2: Llama 3 (8B/70B)

**Pros:** Ubiquitous, strong general performance, massive community support
**Cons:** Not explicitly optimized for code generation to the same degree as Qwen-Coder or DeepSeek Coder. The 70B model is too large for most local setups, and the 8B model falls short on complex coding tasks compared to specialized models.

### Alternative 3: CodeLlama

**Pros:** Specifically for code, built on Llama architecture
**Cons:** Older architecture, significantly outperformed by newer models like Qwen3-Coder in both quality and context length handling.

## Rationale

Qwen3-Coder is selected because:
1. **Performance-to-Size Ratio:** The 7B model offers exceptional performance that can run on consumer hardware (even laptops), while the 32B model provides near-frontier capabilities for users with dedicated GPUs.
2. **Context Window:** Handles large context windows efficiently, which is critical for multi-file generation and understanding repository context.
3. **Language Support:** Unmatched multilingual programming language support, ensuring Atlas is not limited to just Python or JavaScript.
4. **Instruction Following:** High adherence to formatting constraints (e.g., producing valid JSON or specific diff formats), which is essential for agentic workflows where the orchestrator needs to reliably parse the model's output.

## Consequences

### Positive
- High-quality code generation even on modest hardware.
- Reliable structured outputs for tool calling and orchestrator parsing.
- Support for a wide array of programming languages.

### Negative
- Qwen models sometimes exhibit different prompting nuances compared to Llama-based models, requiring specific prompt engineering.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | Runs locally via Ollama/llama.cpp |
| EP-02: Specialization | ✅ | Specifically chosen for the coding domain |

## Related Documents

- [ADR-002: Why Three Models](002-why-three-models.md)
- [Agent Architecture](../../03-architecture/agent-architecture.md)
