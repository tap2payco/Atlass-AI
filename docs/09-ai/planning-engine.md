# Planning Engine

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team  
> **Upstream:** [Cognitive Architecture](cognitive-architecture.md)

---

## Objective
To convert abstract, ambiguous user goals into highly specific, executable Directed Acyclic Graphs (DAGs) of tasks, assigning agents and estimating resources before any code is written.

---

## Cognitive Contract

| Section | Specification |
|---|---|
| **Objective** | Generate executable Task DAGs from natural language. |
| **Inputs** | User Goal, Repository Context, Architecture Memory. |
| **Outputs** | Validated Task DAG (JSON Schema). |
| **Dependencies** | DeepSeek-R1, Context Engine. |
| **Failure Modes** | Circular dependencies, missing prerequisite tasks. |
| **Recovery** | DAG validation failure triggers immediate re-planning loop. |
| **KPIs** | Plan Success Rate (%), Replanning Frequency per Task. |
| **Extension Points** | Custom DAG validators for specific project types. |

---

## The Planning Pipeline

The Planning Engine operates in 4 distinct phases:

```text
Goal Decomposition
       ↓
Dependency Mapping
       ↓
Agent Assignment
       ↓
Validation
```

### 1. Goal Decomposition (DeepSeek-R1)
The engine breaks a goal like "Migrate to PostgreSQL" into granular nodes. 
**Constraint:** A single subtask node should take no longer than 5 minutes for a generation agent to complete. If a node is estimated to take longer, it must be decomposed further.

### 2. Dependency Mapping
Nodes are assembled into a DAG. The Planning Engine must explicitly declare edges: Node B (`Migrate Users Table`) depends on Node A (`Install PostgreSQL Driver`).

### 3. Agent Assignment
The engine routes subtasks to specific Agent Profiles (e.g., `Database Engineer` for the schema, `Backend Engineer` for the queries).

### 4. Validation (Deterministic)
Before the Orchestrator accepts the DAG, a deterministic Rust function validates it:
- Checks for cycles (A -> B -> A).
- Checks that assigned agents exist.
- Checks that output artifacts from Node A match the required inputs for Node B.

---

## Dynamic Replanning

No plan survives contact with reality. If an execution agent fails a task and the [Self-Correction Engine](self-correction-engine.md) determines the root cause is architectural, it triggers a **Replan**.

During a Replan, the Planning Engine receives:
1. The original DAG.
2. The nodes completed successfully.
3. The failed node + the error trace.

The Engine generates a *Diff DAG* (e.g., deleting Node C, adding Node C1 and C2) and passes it back to the Orchestrator to resume execution.

---

## Traceability
- Generates the payload that drives the [Execution Engine](../03-architecture/execution-engine.md).
- Utilizes [DeepSeek-R1](reasoning-engine.md) for decomposition logic.
