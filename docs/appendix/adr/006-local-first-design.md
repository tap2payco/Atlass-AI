# ADR-006: Local-First Design

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Architecture, Security

---

## Context

AI engineering tools heavily rely on cloud APIs (OpenAI, Anthropic) because frontier models demand massive compute. However, this architecture forces developers to transmit proprietary source code to third-party servers. 

For regulated industries, defense, healthcare, and security-conscious enterprises, cloud-based AI coding tools are prohibited. Even for individual developers, reliance on cloud APIs introduces latency, subscription costs, and vendor lock-in.

## Decision

**Atlas will be architected as a strictly Local-First system.** 

All core functionalities—including orchestration, model inference, repository indexing, memory storage, and tool execution—must run entirely on the user's local hardware without requiring an internet connection.

## Alternatives Considered

### Alternative 1: Cloud-Only (SaaS)

**Pros:** Access to the largest frontier models (GPT-4, Claude 3.5 Sonnet); no user hardware requirements; easier to update and monitor.
**Cons:** Excludes security-conscious users; privacy risks; ongoing infrastructure costs; requires continuous internet connection.

### Alternative 2: Hybrid (Local Indexing, Cloud Inference)

**Pros:** Keeps repository index local, only sends necessary snippets to the cloud; balances privacy with model capability.
**Cons:** Code still leaves the machine; requires managing API keys and costs; vulnerable to network latency and API outages.

## Rationale

Local-First is chosen as the foundational constraint because:
1. **Market Differentiation:** The cloud-based AI coding market is saturated (Copilot, Cursor, etc.). The local-first, autonomous agent market is underserved.
2. **Absolute Privacy:** It is the only way to guarantee zero IP leakage, making the tool instantly viable for enterprise and regulated environments.
3. **Zero Marginal Cost:** Users are not constrained by API limits or subscription tiers. They utilize the hardware they already own.
4. **Latency:** Local inference (especially for smaller tasks) and local database queries eliminate network round-trips.

## Consequences

### Positive
- Guaranteed privacy and security of source code.
- Offline capability (usable on airplanes, air-gapped networks).
- Predictable performance not subject to API rate limits or outages.

### Negative
- **Hardware Constraints:** Limits the user base to those with adequate hardware (typically a discrete GPU with >= 8GB VRAM, or a high-end CPU).
- **Capability Ceiling:** Local open-weight models (currently) lag slightly behind the absolute frontier of massive proprietary cloud models.
- **Distribution Size:** The application footprint is large due to bundling dependencies and downloading multi-gigabyte model files.

## Mitigation of Negative Consequences

- Implement model swapping and quantization to lower hardware barriers.
- Rely on multi-agent architectures (ADR-001) to extract maximum performance from smaller models, bridging the capability gap with cloud models.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | This ADR is the instantiation of the principle. |

## Related Documents

- [Engineering Principles](../08-governance/engineering-principles.md)
- [Deployment Architecture](../../05-engineering/deployment-architecture.md)
- [Security Architecture](../../04-subsystems/security-architecture.md)
