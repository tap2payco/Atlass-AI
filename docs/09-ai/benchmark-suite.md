# Benchmark Suite

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** Platform Team  
> **Upstream:** [Evaluation Framework](evaluation-framework.md)

---

## Objective
To provide a reproducible, versioned set of complex, multi-agent software engineering tests that prove Atlas's capabilities. If the Evaluation Framework provides the *metrics*, the Benchmark Suite provides the *obstacle course*.

---

## Cognitive Contract

| Section | Specification |
|---|---|
| **Objective** | Prove system capability through reproducible tests. |
| **Inputs** | Mock repositories, predefined user intents. |
| **Outputs** | Pass/Fail binary results, execution traces. |
| **Dependencies** | Full Atlas Platform. |
| **Failure Modes** | Tests become flaky due to LLM non-determinism. |
| **Recovery** | Run benchmarks N times; require a 90% pass rate. |
| **KPIs** | Benchmark execution speed. |
| **Extension Points** | Community-contributed benchmark scenarios. |

---

## The Benchmark Scenarios

The suite is divided into capability tiers. Every major release of Atlas must pass these tests.

### Tier 1: Core Generation
- **REST API Scaffold:** Generate a Node.js/Express API with 3 endpoints (CRUD), connected to a mocked PostgreSQL DB. *Success: Code runs, tests pass.*
- **React Component:** Build a complex data table component with sorting, filtering, and pagination. *Success: Compiles, passes visual regression tests.*

### Tier 2: Comprehension & Refactoring
- **Framework Migration:** Given a React codebase using class components, rewrite 10 files to use functional components and Hooks. *Success: 0 build errors, exact same DOM output.*
- **Database Migration:** Given a schema and a new requirement (e.g., adding multi-tenant ID), generate the SQL migration and update all ORM calls in the codebase. *Success: Migration applies cleanly, integration tests pass.*

### Tier 3: Autonomous Debugging
- **Memory Leak Search:** Given a codebase with an intentional cyclic reference causing an OOM error, find and fix the leak. *Success: Profiler shows flat memory curve post-fix.*
- **Race Condition Resolution:** Given a Go application with a data race, identify the missing Mutex and apply it. *Success: `go test -race` passes.*

### Tier 4: Cognitive Resilience
- **The Catastrophic Failure Test:** Instruct the agent to build a feature, but intentionally sabotage the sandbox halfway through (e.g., simulate `npm` going down). *Success: Self-Correction Engine detects the network failure, pauses execution, and safely resumes when the network is restored.*

---

## Execution Methodology

To account for LLM temperature and non-determinism, a single benchmark run is not enough.

1. Each scenario is run **5 times**.
2. To achieve a "Pass", the scenario must succeed **4 out of 5 times**.
3. All network calls are stubbed to prevent external APIs from affecting the benchmark results.
4. Inference seeds are fixed where possible, but exact token-for-token matches are not required—success is measured by the Definition of Done (e.g., "Do the tests pass?").

---

## Traceability
- Provides the testbed for the [Evaluation Framework](evaluation-framework.md).
- Validates the [Self-Correction Engine](self-correction-engine.md).
