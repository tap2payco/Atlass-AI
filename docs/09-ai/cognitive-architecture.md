# Cognitive Architecture

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team  
> **Upstream:** [Vision](../01-vision/vision.md), [System Architecture](../03-architecture/system-architecture.md)

---

## Objective

To define the core intelligence loops that elevate Atlas from an autocomplete tool into an **Agentic Software Engineering Platform**. This document outlines how Atlas thinks, plans, reflects, decides, and learns, mapping these cognitive processes to the underlying models.

---

## Inputs
- Natural language requests from the user.
- System events (e.g., test failures, build errors).
- Contextual payloads (ASTs, file contents).

## Outputs
- Actionable task graphs (DAGs).
- Verified code diffs and file modifications.
- Promoted memories and structural graph updates.

## Dependencies
- DeepSeek-R1 (Reasoning/Planning).
- Qwen3-Coder (Generation/Transformation).
- BGE-M3 (Retrieval/Indexing).
- Event Bus (NATS).

---

## Model Responsibilities (Strict Delineation)

To prevent role drift and optimize hardware utilization, cognitive workloads are strictly segregated:

### 1. DeepSeek-R1 (The Brain)
- **Primary Domain:** System 2 Thinking (Slow, deliberate, logical).
- **Responsibilities:** Goal decomposition, long-term planning, code review, debugging root-cause analysis, alternative generation, risk assessment, and reflection.
- **Restrictions:** Never used for bulk code generation.

### 2. Qwen3-Coder (The Hands)
- **Primary Domain:** System 1 Thinking (Fast, pattern-matching, execution).
- **Responsibilities:** Translating plans into code, refactoring, generating tests, generating documentation, and writing shell commands.
- **Restrictions:** Never used for architectural decisions or task decomposition.

### 3. BGE-M3 (The Hippocampus)
- **Primary Domain:** Memory and recall.
- **Responsibilities:** Embedding generation, semantic indexing, and cross-referencing past decisions.
- **Restrictions:** Does not make decisions; strictly a retrieval mechanism.

---

## The Core Cognitive Loops

Intelligence in Atlas is not a linear pipeline; it is a system of intersecting feedback loops.

```mermaid
flowchart TD
    subgraph Execution Loop
        A1(Code Gen) --> A2(Tool Invocation)
        A2 --> A3(Result Parsing)
        A3 --> A1
    end
    
    subgraph Thinking Loop
        B1(Context Eval) --> B2(Hypothesis Gen)
        B2 --> B3(Verification)
    end
    
    subgraph Learning Loop
        C1(Reflection) --> C2(Pattern Extraction)
        C2 --> C3(Memory Update)
    end
    
    UserRequest --> Thinking Loop
    Thinking Loop --> Planning Loop
    Planning Loop --> Execution Loop
    Execution Loop -.->|Failures| Thinking Loop
    Execution Loop --> Learning Loop
```

### 1. The Thinking Loop (DeepSeek-R1)
Before acting, the system pauses to build a mental model of the problem space.
- **Context Evaluation:** What do I know? What am I missing?
- **Hypothesis Generation:** "If the API is returning 404, either the route is missing, or the NGINX config is blocking it."
- **Verification Plan:** "I will use `grep` to check the routes, then read the config."

### 2. The Planning Loop (DeepSeek-R1)
Converts hypotheses into executable DAGs.
- **Goal Decomposition:** Breaks "Add Stripe integration" into 15 subtasks.
- **Resource Estimation:** Determines which agents are needed.
- **Risk Assessment:** Identifies breaking changes.

### 3. The Execution Loop (Qwen3-Coder)
The fastest, tightest loop.
- **Generation:** Writes the code.
- **Tooling:** Runs the linter/compiler.
- **Micro-Correction:** Fixes syntax errors immediately without consulting the Planning loop.

### 4. The Decision Loop (Orchestrator + DeepSeek-R1)
When faced with ambiguity, Atlas evaluates cost, risk, and confidence (See [Decision Engine](decision-engine.md)).

### 5. The Reflection Loop (DeepSeek-R1)
Runs after a task completes or fails catastrophically.
- **Self-Interrogation:** "Why did my first plan fail? What assumption was wrong?"

### 6. The Learning Loop (BGE-M3 + DeepSeek-R1)
The mechanism for permanent improvement.
- **Pattern Extraction:** Converts a localized fix into a generalized rule.
- **Memory Update:** Promotes the rule to Architecture Memory (See [Learning Engine](learning-engine.md)).

---

## Failure Modes & Recovery

| Failure Mode | Detection | Recovery Strategy |
|---|---|---|
| **Looping (Execution)** | Qwen3 makes the same syntax mistake 3 times. | Break Execution Loop. Escalate to Thinking Loop (DeepSeek) to find root cause. |
| **Hallucinated Plan** | Execution engine finds missing dependencies in DAG. | Halt execution. DeepSeek-R1 replans with the failure context injected. |
| **Role Drift** | Qwen3 attempts to redesign architecture. | Orchestrator/Event Bus rejects the architectural tool call based on Agent Contract constraints. |

---

## KPIs

- **Cognitive Efficiency:** The ratio of Execution cycles to Planning cycles (a smart plan requires fewer execution retries).
- **Autonomy Horizon:** The average amount of time the system can operate independently before requiring a human decision.

---

## Extension Points
- New foundation models can be hot-swapped via the Hexagonal `InferencePort` as long as they map to the Reasoning (System 2) or Generation (System 1) paradigms.
