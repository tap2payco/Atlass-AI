# ADR-010: Why Event-Driven Architecture

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Architecture

---

## Context

Atlas consists of multiple bounded contexts: Agent Execution, Memory, RAG/Indexing, Tooling, and UI. These components need to communicate. For example, when an agent completes a task, the Memory system needs to update, the UI needs to reflect the new state, and the Orchestrator needs to schedule the next step.

## Decision

**We will use an Event-Driven Architecture (EDA) as the primary communication pattern between bounded contexts.**

Components will publish domain events (e.g., `TaskCompleted`, `FileModified`, `AgentThought`) to an event bus, and interested components will subscribe to these events.

## Alternatives Considered

### Alternative 1: Direct Synchronous Calls (RPC / Method Calls)

**Pros:** Simple to trace, easy to understand, immediate consistency.
**Cons:** High coupling. If the Orchestrator calls the Memory system directly, it must know about the Memory system's API and handle its failures. Slows down execution if one system is blocked.

### Alternative 2: Shared Database / State

**Pros:** Easy to implement initially.
**Cons:** Violates AP-04 (Agent Isolation). Creates a monolith where every component depends on the database schema, making it impossible to scale or replace components independently.

## Rationale

Event-Driven Architecture is selected because:
1. **Decoupling:** The Orchestrator doesn't need to know *who* cares that a task is completed; it just publishes the fact. This allows us to add new features (like an Observability dashboard or a Plugin system) by simply subscribing to existing events without changing core code.
2. **Asynchronous Non-Blocking Execution:** Agents can emit thoughts and progress updates without waiting for the UI to render or the database to write.
3. **Auditability:** An append-only log of domain events provides a perfect audit trail of everything the AI did, fulfilling strict security and transparency requirements.
4. **Evolution:** EDA paves the way for a future distributed architecture. Today the events pass in-memory; tomorrow they can pass across a network to distributed agents.

## Consequences

### Positive
- Highly extensible system (plugins just listen to events).
- Excellent observability and auditability.
- Resilient to individual component failures.

### Negative
- **Eventual Consistency:** The UI might briefly lag behind the actual state while events propagate.
- **Debugging Complexity:** Following a flow across multiple event handlers is harder than reading a single synchronous function. Requires strong distributed tracing (OpenTelemetry).
- **Schema Management:** Event payloads must be versioned and strictly typed to prevent breaking subscribers.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| AP-03: Event-Driven Comm | ✅ | This ADR establishes the principle. |
| EP-07: Extensibility | ✅ | Events are the primary extension point for plugins. |

## Related Documents

- [Architecture Principles](../08-governance/architecture-principles.md)
- [Communication Protocol](../../03-architecture/communication-protocol.md)
- [ADR-011: Why NATS](011-why-nats.md)
