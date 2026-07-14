# Decision Engine

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team  
> **Upstream:** [Reasoning Engine](reasoning-engine.md)

---

## Objective
To mathematically and logically determine the best course of action when multiple valid options exist. The Decision Engine acts as the "executive function" of Atlas, weighing risk, cost, and confidence before allowing the Execution Engine to mutate the workspace.

---

## Cognitive Contract

| Section | Specification |
|---|---|
| **Objective** | Select optimal paths and block dangerous/costly actions. |
| **Inputs** | Proposed Actions, Confidence Scores, System Constraints. |
| **Outputs** | Approved Action, Rejected Action, or Escalation Request. |
| **Dependencies** | Reasoning Engine, Approval Manager (Orchestrator). |
| **Failure Modes** | Analysis paralysis, miscalculation of risk. |
| **Recovery** | Default to safest action (usually: block and ask user). |
| **KPIs** | Escalation Rate (%), Rollback Frequency (due to bad decisions). |
| **Extension Points** | Pluggable risk policies per project environment. |

---

## The Decision Matrix

When the Reasoning Engine generates an intent, it passes through the Decision Engine. The engine calculates three metrics for every proposed action:

1. **Confidence Score (0.0 - 1.0):** How certain is the Reasoning Engine that this action solves the problem?
2. **Risk Score (0.0 - 1.0):** What is the blast radius? (e.g., `write_file` to a test = 0.1. `DROP TABLE` = 1.0).
3. **Cost Score (0.0 - 1.0):** How long will this take, or how much compute will it use?

### Evaluation Logic

```mermaid
flowchart TD
    A[Proposed Action] --> B{Risk > 0.8?}
    B -->|Yes| C{Confidence > 0.95?}
    
    C -->|No| D[Escalate to Human]
    C -->|Yes| E[Require Architecture Approval]
    
    B -->|No| F{Confidence > Risk?}
    
    F -->|Yes| G[Approve Execution]
    F -->|No| H{Cost < 0.3?}
    
    H -->|Yes| I[Approve (Low Penalty for Failure)]
    H -->|No| D
```

---

## Tool and Agent Selection

The Decision Engine doesn't just evaluate risk; it evaluates strategy. 

If the Planning Engine needs a new REST endpoint written, the Decision Engine determines:
- **Agent Selection:** Send to `Backend Engineer` (Qwen3-Coder) because the task is standard generation.
- **Tool Selection:** Deny access to `git_commit`. The agent must output diffs for the Orchestrator to apply atomically.

---

## Escalation to Human Review

Atlas is designed to be autonomous, but it is not reckless. The Decision Engine triggers an `ApprovalRequired` event under specific conditions:
1. **Destructive Ambiguity:** Confidence is low, but the action modifies core infrastructure (e.g., CI/CD pipelines, DB schemas).
2. **Policy Violation:** The action violates a rule in the Project Architecture Memory (e.g., "Use fetch, not axios," but the agent insists on axios to solve a dependency conflict).
3. **Resource Exhaustion:** The chosen path will require >30 minutes of continuous GPU inference.

When escalated, the UI presents the user with:
- The Problem.
- The Options Generated.
- The Risk Analysis.
- The Reason for Escalation.

---

## Traceability
- Feeds decisions into the [Orchestrator Design](../03-architecture/orchestrator-design.md) (specifically the Approval Manager).
- Operates on outputs from the [Reasoning Engine](reasoning-engine.md).
