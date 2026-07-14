# Agent Architecture & Formal Contracts

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team  
> **Upstream:** [System Architecture](system-architecture.md), [Agent Lifecycle](agent-lifecycle.md)  
> **ADR References:** [ADR-001](../appendix/adr/001-multi-agent-architecture.md), [ADR-002](../appendix/adr/002-why-three-models.md)

---

## Purpose

To ensure components are swappable and verifiable, agents are not defined by prose descriptions or arbitrary prompts. Every agent in Atlas is defined by a **Formal Contract**. 

The Execution Engine enforces these contracts. If an agent attempts to invoke a tool not listed in its `Allowed Tools`, the Execution Engine rejects the request. If it fails to produce the required `Outputs`, the subtask is marked as Failed.

---

## The Base Contract Schema

Every agent implements this 20-field schema.

| Field | Description |
|---|---|
| **Purpose** | Single-sentence reason for existence. |
| **Responsibilities** | Bulleted list of what the agent actually does. |
| **Inputs** | Required data payload to start the agent. |
| **Outputs** | Required data payload produced upon success. |
| **Required Context** | Specific memory layers that must be injected into the prompt. |
| **Memory Access** | Read/Write permissions for the 10 memory layers. |
| **Allowed Tools** | Strict allowlist of sandbox tools. |
| **Forbidden Tools** | Strict blocklist of tools. |
| **Permissions** | OS-level permissions (Network, FS-Write, Exec). |
| **Dependencies** | Other agents that must run before this one. |
| **Produced Events** | NATS topics this agent publishes to. |
| **Consumed Events** | NATS topics this agent subscribes to. |
| **Failure Modes** | Expected ways this agent might fail. |
| **Retry Strategy** | How to handle transient failures. |
| **Timeout Strategy** | SLA limits and what happens when breached. |
| **Recovery Strategy** | How to unwind state if failure is permanent. |
| **Health Checks** | How the Orchestrator verifies the agent is alive. |
| **KPIs** | Metrics used to evaluate model performance for this agent. |
| **Performance SLA** | Maximum acceptable latency for standard tasks. |
| **Versioning** | Strategy for upgrading the agent's prompts/schema. |

---

## Core Agent Contracts (Examples)

### 1. Planner Agent

The brain of task decomposition. Uses DeepSeek R1.

| Field | Contract Specification |
|---|---|
| **Purpose** | Convert natural language user requests into Directed Acyclic Graphs (DAGs) of executable subtasks. |
| **Responsibilities** | Task decomposition, dependency mapping, initial estimation. |
| **Inputs** | `UserRequest { text: string, session_id: string }` |
| **Outputs** | `TaskDAG { nodes: Subtask[], edges: Dependency[] }` |
| **Required Context** | `Repository Memory (AST)`, `Project Memory` |
| **Memory Access** | Read: All. Write: `Task Memory` |
| **Allowed Tools** | `read_file`, `list_dir`, `semantic_search`, `get_dependencies` |
| **Forbidden Tools** | `write_file`, `run_cmd`, `git_*` (No mutation allowed) |
| **Permissions** | Read-Only FS, No Network, No Exec |
| **Dependencies** | None (Entrypoint) |
| **Produced Events** | `atlas.task.planned` |
| **Consumed Events** | `atlas.task.requested` |
| **Failure Modes** | Circular dependencies in DAG; Hallucinated files. |
| **Retry Strategy** | Re-prompt with error string (Max 2). |
| **Timeout Strategy** | SLA: 45s. Emit `PlanningTimeout` event on breach. |
| **Recovery Strategy** | Inform user planning failed, request clearer instructions. |
| **Health Checks** | Responds to ping via LLM context window check. |
| **KPIs** | % of DAGs that execute without missing dependency errors. |
| **Performance SLA** | < 30 seconds for standard decomposition. |
| **Versioning** | Tied to DeepSeek R1 prompt template versions. |

---

### 2. Architect Agent

Ensures structural integrity. Uses DeepSeek R1.

| Field | Contract Specification |
|---|---|
| **Purpose** | Validate Planner DAGs against established Project Architecture Memory. |
| **Responsibilities** | Architecture review, technical design generation, API contract drafting. |
| **Inputs** | `TaskDAG { ... }` |
| **Outputs** | `ValidatedTaskDAG { ... }` or `DesignDocument` |
| **Required Context** | `Architecture Memory`, `Decision Memory (ADRs)` |
| **Memory Access** | Read: All. Write: `Architecture Memory`, `Decision Memory` |
| **Allowed Tools** | `read_file`, `semantic_search`, `get_callers_of`, `get_type_graph` |
| **Forbidden Tools** | `write_file`, `run_cmd` |
| **Permissions** | Read-Only FS, No Network, No Exec |
| **Dependencies** | `Planner Agent` |
| **Produced Events** | `atlas.architecture.approved`, `atlas.architecture.rejected` |
| **Consumed Events** | `atlas.task.planned` |
| **Failure Modes** | Misses subtle architectural violations; Over-engineers solutions. |
| **Retry Strategy** | Fast-fail. If validation fails, route back to Planner. |
| **Timeout Strategy** | SLA: 30s. |
| **Recovery Strategy** | User override required. |
| **Health Checks** | Schema validation of output. |
| **KPIs** | Defect rate of architectural regressions caught post-commit. |
| **Performance SLA** | < 20 seconds. |
| **Versioning** | Semantic versioning of output schema. |

---

### 3. Backend Engineer Agent

The primary code generator. Uses Qwen3-Coder.

| Field | Contract Specification |
|---|---|
| **Purpose** | Implement backend logic, APIs, and database migrations. |
| **Responsibilities** | Writing implementation code, writing unit tests, fixing linter errors. |
| **Inputs** | `Subtask { id, description, dependencies_completed }` |
| **Outputs** | `FileDiff[]` |
| **Required Context** | `Working Memory`, `Task Memory`, `Repository Intelligence (Target Files)` |
| **Memory Access** | Read: Project, Task. Write: `Working Memory` |
| **Allowed Tools** | `read_file`, `write_file`, `run_cmd`, `run_tests`, `run_linter` |
| **Forbidden Tools** | `git_commit`, `git_push` |
| **Permissions** | Read/Write FS (Workspace only), Network (Localhost only), Exec (Sandboxed) |
| **Dependencies** | `Architect Agent` |
| **Produced Events** | `atlas.agent.tool_request`, `atlas.task.review_requested` |
| **Consumed Events** | `atlas.task.ready_for_exec` |
| **Failure Modes** | Syntax errors, failing tests, logic errors. |
| **Retry Strategy** | If tests fail, re-prompt with test output (Max 3 revisions). |
| **Timeout Strategy** | SLA: 120s. On breach, terminate subprocess and Rollback. |
| **Recovery Strategy** | Rollback workspace to pre-task snapshot. |
| **Health Checks** | Subprocess heartbeat. |
| **KPIs** | First-pass compilation success rate; First-pass test success rate. |
| **Performance SLA** | < 60 seconds per file generated. |
| **Versioning** | Tied to Qwen3-Coder prompt template versions. |

---

### 4. Code Review Agent

The quality gate. Uses DeepSeek R1.

| Field | Contract Specification |
|---|---|
| **Purpose** | Ensure all generated code meets quality, security, and correctness standards before user presentation. |
| **Responsibilities** | Static analysis, logic review, security check, adherence to DoD. |
| **Inputs** | `FileDiff[]`, `Subtask` |
| **Outputs** | `ReviewDecision { approved: boolean, comments: string[], fix_suggestions: Diff[] }` |
| **Required Context** | `Project Memory (Coding Standards)`, `Decision Memory` |
| **Memory Access** | Read: All. Write: None |
| **Allowed Tools** | `read_file`, `get_symbol_definition` |
| **Forbidden Tools** | All mutating tools. |
| **Permissions** | Read-Only FS |
| **Dependencies** | `Backend Engineer`, `Frontend Engineer`, etc. |
| **Produced Events** | `atlas.review.approved`, `atlas.review.rejected` |
| **Consumed Events** | `atlas.task.review_requested` |
| **Failure Modes** | Misses bugs (False Negative); Rejects good code (False Positive). |
| **Retry Strategy** | No retries. Decision is final per cycle. |
| **Timeout Strategy** | SLA: 45s. |
| **Recovery Strategy** | If reviewer crashes, fail open (present to User for manual review) with warning. |
| **Health Checks** | Schema validation of ReviewDecision. |
| **KPIs** | Catch rate of bugs introduced by Coder agents. |
| **Performance SLA** | < 30 seconds per diff. |
| **Versioning** | Independent of generation agents. |

---

## Agent Roster

Beyond the core 4 defined above, Atlas supports a full roster of agents adhering to the same contract structure:

1. **Frontend Engineer:** (Qwen3-Coder) Focuses on React/TS, CSS, accessibility.
2. **Database Engineer:** (Qwen3-Coder) Focuses on SQL migrations, schema design, performance indexing.
3. **QA Engineer:** (DeepSeek R1) Focuses on E2E test generation, edge case identification.
4. **DevOps Engineer:** (Qwen3-Coder) Focuses on Dockerfiles, CI/CD pipelines, NATS config.
5. **Security Engineer:** (DeepSeek R1) Focuses on SAST, secret detection, dependency audits.
6. **Documentation Agent:** (Qwen3-Coder) Generates inline docs, READMEs, API specs.
7. **Memory Agent:** (BGE-M3) Background agent that consolidates, scores, and prunes layered memory.
8. **Repository Agent:** (Tree-sitter) Background agent that builds the 9 semantic/structural graphs.
9. **Task Coordinator:** (DeepSeek R1) Resolves conflicts when two executing agents need the same file.

---

## Design Rationale

**Why formal contracts instead of just different system prompts?**
System prompts are brittle. If an engineer modifies the DevOps agent's prompt to accidentally encourage running `docker rm -f $(docker ps -aq)`, the system needs a hard boundary to prevent it. The Contract's `Allowed Tools` and `Permissions` fields are enforced by the Rust Execution Engine, completely independent of the LLM. This provides defense-in-depth.

**Why Versioning?**
Agent prompts are highly coupled to specific model versions. What works for Qwen 2.5 might degrade in Qwen 3. By versioning the contracts, we can run A/B tests on new models and gracefully roll back if an agent's KPIs drop.

---

## Traceability

- Implements [FR-300 (AI Agent System)](../02-prd/prd.md) requirements.
- Core mechanism for enforcing [EP-02 (Specialization)](../08-governance/engineering-principles.md).
- Permissions fields enforce [EP-05 (Fail Safe)](../08-governance/engineering-principles.md) and [SEC-07](../02-prd/prd.md).
