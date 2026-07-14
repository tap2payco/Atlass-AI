# ADR-004: Why DeepSeek R1

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** AI, Technology

---

## Context

Atlas requires a model dedicated to complex reasoning, task decomposition, architectural planning, code review, and root cause analysis. This model acts as the "brain" of the system, guiding the generation models and validating their output.

## Decision

**We will use DeepSeek R1 (quantized variants based on user hardware) as the primary reasoning and planning model.**

## Alternatives Considered

### Alternative 1: Llama 3 (70B)

**Pros:** Strong general reasoning capabilities.
**Cons:** Standard models without specific reasoning-focused training (like R1's RLHF approach) require extensive chain-of-thought prompting to match specialized reasoning models, increasing latency and context usage.

### Alternative 2: Qwen3-Coder (Single Model Approach)

**Pros:** Simplifies architecture to one model.
**Cons:** While good at coding, it lacks the deep, multi-step logical deduction capabilities of a dedicated reasoning model. Planning and debugging suffer.

## Rationale

DeepSeek R1 is selected because:
1. **State-of-the-Art Reasoning:** It currently represents the frontier of open-weight reasoning models, utilizing advanced reinforcement learning techniques specifically to improve logical deduction, mathematics, and complex problem-solving.
2. **Distilled Variants:** The availability of smaller, distilled versions of R1 allows users with less VRAM to still benefit from its reasoning patterns.
3. **Agentic Suitability:** DeepSeek R1 excels at breaking down ambiguous problems into structured plans, a critical requirement for the Planner and Architect agents.
4. **Review Capabilities:** Its strong logical capabilities make it exceptional at identifying edge cases, security flaws, and architectural inconsistencies during code review.

## Consequences

### Positive
- Extremely high-quality planning and debugging.
- Reduces the likelihood of the system getting stuck in loops during autonomous execution.
- Provides rigorous code review, improving overall system output quality.

### Negative
- Reasoning models are inherently slower than standard generation models because they "think" (generate internal chain-of-thought tokens) before producing the final answer. This increases latency for planning steps.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | Runs locally; distilled versions fit on consumer GPUs. |
| EP-02: Specialization | ✅ | Specifically chosen for the reasoning domain. |
| EP-04: Quality Over Speed | ✅ | We accept the slower generation speed of R1 for the superior quality of its plans. |

## Related Documents

- [ADR-002: Why Three Models](002-why-three-models.md)
- [Agent Architecture](../../03-architecture/agent-architecture.md)
- [Reasoning Framework](../../09-ai/reasoning-framework.md)
