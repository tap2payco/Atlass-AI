# Knowledge Graph Architecture

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team  
> **Upstream:** [Layered Memory System](../03-architecture/memory-system.md), [Repository Intelligence](../04-subsystems/repository-intelligence.md)

---

## Objective
To map the relational structure of both the codebase (structural intelligence) and the agent's history (causal intelligence), providing deep relational context that standard vector embeddings cannot capture.

---

## Cognitive Contract

| Section | Specification |
|---|---|
| **Objective** | Provide deterministic relational context. |
| **Inputs** | AST Parses, Execution Logs, Decisions, Tasks. |
| **Outputs** | Graph query results (e.g., shortest path, dependents). |
| **Dependencies** | Tree-sitter, Orchestrator Event Bus. |
| **Failure Modes** | Stale edges after bulk file changes. |
| **Recovery** | Background graph reconciliation job. |
| **KPIs** | Graph update latency (< 500ms per file). |
| **Extension Points** | Pluggable graph storage backends. |

---

## Dual Graph Structure

The Knowledge Graph consists of two intersecting domains:

### 1. The Structural Graph (The "What")
Tracks the physical reality of the code.
```text
Workspace
  └─ Project
        └─ Repository
              └─ Package
                    └─ Module
                          └─ Class
                                └─ Method
                                      └─ Symbol (Variable)
```
**Edges:** `Contains`, `Calls`, `Imports`, `Implements`, `Extends`.

### 2. The Causal Graph (The "Why")
Tracks the history of AI execution.
```text
User Request
  └─ Session
        └─ Task DAG
              └─ Subtask
                    └─ Decision (via Decision Engine)
                          └─ Execution (Tool Run)
                                └─ Outcome (Success/Fail)
                                      └─ Learning (Promoted Memory)
```
**Edges:** `Resolves`, `TriggeredBy`, `ResultedIn`, `GeneratedArtifact`.

---

## Graph Intersection

The true power of the Knowledge Graph is the intersection of these two domains. 

Because an `Execution` node generates an `Artifact` (a file diff), which mutates a `Module` node, the graph can answer questions like:
- *"Which Agent modified this Method last week, and what Decision led to it?"*
- *"Show me all Failed Tasks that touched the Authentication Package."*

This intersection enables the [Learning Engine](learning-engine.md) to find systemic patterns in failures.

---

## Storage & Implementation

- **Database:** While a dedicated graph DB (like Neo4j) is standard in enterprise, to maintain the Local-First requirement (EP-01), the Knowledge Graph is implemented relationally via recursive CTEs in **SQLite/PostgreSQL**.
- **Vector Overlay:** Every node in the graph contains a foreign key to a Qdrant vector ID, allowing Semantic Search (RAG) to serve as the entry point into a Graph Traversal.

---

## Traceability
- Fuses [Repository Intelligence](../04-subsystems/repository-intelligence.md) with the [Layered Memory System](../03-architecture/memory-system.md).
