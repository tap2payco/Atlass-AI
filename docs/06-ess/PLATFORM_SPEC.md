# Atlas Platform Specification (PLATFORM_SPEC)

> **Status:** Active  
> **Version:** 1.0.0  
> **Purpose:** The architectural constitution governing all runtimes in the Atlas Platform.  

## 1. Runtime Lifecycle
Every runtime must implement a standardized lifecycle managed by the `Runtime Kernel`:
1. **Initialize:** Configuration parsing, dependency injection.
2. **Register:** Declare capabilities to the Platform Registry.
3. **Start:** Open communication channels and subscribe to events.
4. **Health Check:** Expose liveness and readiness probes.
5. **Shutdown:** Graceful termination and resource cleanup.

## 2. Dependency Rules (Downward Only)
Atlas enforces a strict downward-only dependency graph. Runtimes may not reference runtimes higher in the stack.
**Stack Order (Top to Bottom):**
1. Applications (Desktop, CLI, Server)
2. Agent Runtime
3. Planning Runtime
4. Reasoning Runtime
5. Memory Runtime (Depends on Context Runtime, NOT Repository Runtime)
6. Context Runtime
7. Repository Runtime
8. Execution Core
9. Runtime Kernel
10. Communication
11. Shared (SDK, Observability)

## 3. Communication Rules
- The **Communication** crate owns all NATS, serialization, deserialization, envelopes, and retries.
- The **Runtime Kernel** does NOT own messaging.
- Runtimes communicate exclusively across defined SDK boundaries. No shared memory passing across runtime boundaries.

## 4. Capability Model
Every runtime must declare a machine-readable manifest (`runtime.yaml` or equivalent) containing:
- `name` and `version`
- `capabilities`
- `dependencies`
- `provides` / `consumes`
- `configuration`
- `health_checks`
- `metrics`
- `events`

## 5. Certification Levels
Runtimes are governed by maturity levels:
- **L0:** Specification complete
- **L1:** Skeleton / Scaffolding complete
- **L2:** Core logic implemented
- **L3:** Feature complete (Test coverage > 80%, Benchmarks pass)
- **L4:** Production hardened
- **L5:** Optimized

A runtime must achieve **L3 Certification** before its abstractions can be extracted into shared infrastructure (e.g., `Execution Core`).

## 6. Platform Invariants
- **No Circular Dependencies:** Automated via Conformance tests.
- **Explainable Context:** Memory only stores what Context validates as meaningful.
- **Execution Agnostic:** `Execution Core` knows nothing about AI context, planning, or providers. It only orchestrates generic `PipelineStage<I, O>`.
