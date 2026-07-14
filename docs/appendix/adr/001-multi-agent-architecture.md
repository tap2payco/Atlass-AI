# ADR-001: Multi-Agent Architecture

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Architecture, AI

---

## Context

Atlas needs an architecture for its AI capabilities. The fundamental question is whether to use a single monolithic AI model for all tasks or distribute responsibilities across multiple specialized agents.

Current AI coding tools (Copilot, Cursor) use a single model. More advanced systems (Devin, OpenHands) use multi-step approaches but not true multi-agent specialization.

Software engineering involves fundamentally different cognitive tasks: planning requires different skills than coding, which differs from debugging, which differs from security review. A single model must compromise on all.

## Decision

**We will use a Multi-Agent Architecture where specialized AI agents collaborate to complete software engineering tasks.**

Each agent has a defined role, formal contract, and specific model assignment. Agents communicate through a defined protocol and are coordinated by an orchestrator.

## Alternatives Considered

### Alternative 1: Single Model, Multi-Prompt

Use one model with different system prompts for different tasks.

**Pros:** Simpler architecture, lower resource usage, easier to maintain  
**Cons:** No true specialization, model compromises on all tasks, no parallel execution, single point of failure

### Alternative 2: Single Model, Multi-Step

Use one model in a sequential pipeline (plan → code → test → review).

**Pros:** Simpler than multi-agent, sequential logic is easy to debug  
**Cons:** Sequential bottleneck, no specialization, can't run review while coding

### Alternative 3: Multi-Agent, Single Model

Multiple agents but all using the same model with different prompts.

**Pros:** Specialization through prompts, simpler model management  
**Cons:** One model can't be best at everything, wastes resources loading same model multiple times

## Rationale

Multi-agent architecture is chosen because:
1. **Specialization** — Qwen3-Coder is optimized for code generation; DeepSeek R1 is optimized for reasoning. Using each where it excels produces better results than either alone.
2. **Parallelism** — Independent agents can work concurrently (e.g., documentation while testing).
3. **Isolation** — Agent failures don't cascade. A coding agent crashing doesn't affect the planning agent.
4. **Extensibility** — New agents can be added without modifying existing ones.
5. **Real-world analogy** — High-performing engineering teams use specialists, not generalists.

## Consequences

### Positive
- Each agent can use the optimal model for its task
- Parallel execution reduces end-to-end latency
- Agents can be tested, debugged, and improved independently
- New capabilities are added by creating new agents, not modifying existing ones

### Negative
- Coordination complexity (orchestrator, communication protocol, conflict resolution)
- Higher resource usage (multiple agent instances)
- More complex testing (agent interactions)
- Inter-agent communication overhead

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-02: Specialization | ✅ | Core expression of this principle |
| EP-05: Fail Safe | ✅ | Agent isolation prevents cascade |
| EP-07: Extensibility | ✅ | New agents without modifying existing |

## Related Documents

- [Agent Architecture](../../03-architecture/agent-architecture.md)
- [Orchestrator Design](../../03-architecture/orchestrator-design.md)
- [Communication Protocol](../../03-architecture/communication-protocol.md)
- [ADR-002: Why Three Models](002-why-three-models.md)
