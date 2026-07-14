# ADR-011: Why NATS

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Technology, Infrastructure

---

## Context

Given the decision to use an Event-Driven Architecture ([ADR-010](010-why-event-driven.md)), we need a messaging system to route events between bounded contexts (Orchestrator, Agents, UI, Memory).

The system must run locally with zero configuration for single users, but the architecture must support future distributed deployments (e.g., enterprise clusters with remote agents).

## Decision

**We will use NATS as our messaging and event bus abstraction.** 

For local-first deployments, we will use an in-process, NATS-compatible abstraction (or embedded NATS). For distributed deployments, the system will connect to a standard NATS server with zero code changes to the domain logic.

## Alternatives Considered

### Alternative 1: RabbitMQ

**Pros:** Industry standard, extremely feature-rich.
**Cons:** Requires an Erlang runtime, heavy resource footprint, complex configuration. Completely unsuitable for a zero-config, local-first application.

### Alternative 2: Redis Pub/Sub

**Pros:** Fast, widely understood.
**Cons:** Requires running a Redis server. Does not guarantee message delivery (fire and forget) out of the box without complex Redis Streams setup.

### Alternative 3: Tokio Channels (In-Process Only)

**Pros:** Zero external dependencies, extremely fast, native to Rust.
**Cons:** Cannot scale beyond a single process. If a user wants to run an agent on a separate machine with a bigger GPU, the architecture would have to be rewritten.

## Rationale

NATS is selected because:
1. **Lightweight and Embeddable:** The NATS server is a single, small Go binary. More importantly, we can abstract the NATS API so that for local use, it runs entirely in-process, incurring zero overhead.
2. **Scalability Path:** Code written against the NATS abstraction works identically whether the messages are passing in memory or across the world to a distributed cluster. This future-proofs Atlas for enterprise deployment.
3. **JetStream:** NATS JetStream provides persistent, at-least-once delivery, which is critical for the task execution queue and audit logs.
4. **Pattern Support:** NATS natively supports Pub/Sub (for events), Request-Reply (for synchronous queries), and Queue Groups (for load balancing work across multiple agents of the same type).

## Consequences

### Positive
- Zero-friction transition from local-only to distributed execution.
- High performance and low latency.
- Rich messaging patterns out of the box.

### Negative
- Adds a layer of abstraction (the NATS interface) compared to just using direct Rust channels initially.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | Using the embedded/in-process abstraction ensures it runs offline. |
| AP-02: Hexagonal Arch | ✅ | NATS is hidden behind an `EventBusPort`. |

## Related Documents

- [Technology Selection Framework](../08-governance/technology-selection-framework.md)
- [ADR-010: Why Event-Driven Architecture](010-why-event-driven.md)
