# Cognitive Traceability Model

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team & Architecture Team  
> **Upstream:** [Cognitive Architecture](cognitive-architecture.md), [Communication Protocol](../03-architecture/communication-protocol.md)

---

## Objective

To provide an unbroken, end-to-end map of a single user request flowing through the entire cognitive and architectural stack. This document traces exactly which component owns the step, which model performs the cognitive labor, which memory layers are accessed, and what events are emitted.

This is the definitive blueprint for debugging, observability, and tracing in Atlas.

---

## Traceability Matrix

The following matrix traces a complex request: *"Refactor the authentication module to use JWTs instead of session cookies."*

| Step | Action | Component / Owner | Model | Memory Layer (R/W) | Emitted Event |
|:---|:---|:---|:---|:---|:---|
| **1. Request** | User hits Enter in the UI. | Desktop Shell / UI | None | None | `atlas.req.received` |
| **2. Context Selection** | Initial token budget calculated; active file context grabbed. | Context Engine | None | Session Memory (R) | `atlas.ctx.assembled` |
| **3. Understanding** | Parse intent, identify ambiguity. | AI Runtime / Task Coordinator | DeepSeek-R1 | Session Memory (R/W) | `atlas.cog.understanding` |
| **4. Retrieval** | Search repo for "session cookies", "authentication". | RAG++ Pipeline | BGE-M3 | Repo Memory (R) | `atlas.rag.queried` |
| **5. Reasoning** | Analyze security implications of JWT switch. | Decision Engine | DeepSeek-R1 | Arch Memory (R) | `atlas.cog.reasoning` |
| **6. Planning** | Decompose into: 1. Auth service, 2. Middleware, 3. Tests. | Planning Engine | DeepSeek-R1 | Task Memory (W) | `atlas.task.planned` |
| **7. Decision** | Decide which agent gets which subtask. | Orchestrator (Dispatcher) | None | None | `atlas.task.dispatched` |
| **8. Execution 1** | Implement Auth service code. | Backend Eng Agent | Qwen3-Coder | Working Memory (W) | `atlas.agent.tool_req` |
| **9. Validation** | Run `npm test` on the auth service. | Tool Runtime (Sandbox) | None | None | `atlas.agent.tool_res` |
| **10. Review** | Statically analyze JWT implementation for known CVEs. | Code Review Agent | DeepSeek-R1 | Knowledge Memory (R) | `atlas.review.approved` |
| **11. Reflection** | Analyze why 3 tests failed initially during Execution 1. | Self-Correction Engine | DeepSeek-R1 | Task Memory (R) | `atlas.cog.reflection` |
| **12. Learning** | Extract rule: "JWTs require `Bearer` prefix in this framework." | Learning Engine | DeepSeek-R1 | None | `atlas.cog.learning` |
| **13. Memory Update** | Commit rule to Project Architecture. | Memory Manager | BGE-M3 | Arch Memory (W) | `atlas.memory.promoted` |
| **14. Done** | Notify user of completion. | API Gateway | None | Session Memory (W) | `atlas.req.completed` |

---

## Observability Implementation

To make this traceability model a reality in production code, Atlas relies on the [Communication Protocol](../03-architecture/communication-protocol.md) tracing headers:

### Correlation ID (`corr_id`)
A single `corr_id` is generated at Step 1 and attached to every single event through Step 14. This allows the UI to render a perfect chronological timeline of the AI's "thought process."

### Trace Hierarchy (`parent_id`)
Step 8 (Execution) emits a `tool_req`. Step 9 (Validation) emits a `tool_res`. The `tool_res` will have the `parent_id` of the `tool_req`, proving exactly which agent triggered which shell command.

---

## Failure Modes & Recovery

| Failure Mode | Impact | Recovery |
|---|---|---|
| **Broken Trace Chain** | An event drops its `corr_id`. The UI loses track of the task. | Strict JSON schema validation at the Event Bus rejects any payload missing tracing headers. |
| **Model Hallucination** | Step 6 (Planning) generates tasks for files that don't exist. | Step 8 (Execution) fails on `read_file`. Self-Correction Engine catches the error, traces back to Step 6, and forces a re-plan. |

---

## KPIs
- **Trace Completeness:** 100% of events on the NATS bus must contain valid correlation headers.
- **Timeline Resolution:** The ability to visualize the end-to-end matrix in the UI in under 100ms.

## Extension Points
- **OpenTelemetry Export:** Because every cognitive step maps to an event with a standard envelope, the entire flow can be exported to Jaeger, Datadog, or Grafana seamlessly.
