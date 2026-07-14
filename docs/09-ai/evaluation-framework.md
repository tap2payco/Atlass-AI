# Evaluation Framework

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team  
> **Upstream:** [Cognitive Architecture](cognitive-architecture.md)

---

## Objective
To quantify the intelligence of Atlas. The Evaluation Framework provides scorecards to measure how well the cognitive engines perform their duties, preventing regressions when model weights (e.g., Qwen to Qwen-Next) or core prompts are updated.

---

## Cognitive Contract

| Section | Specification |
|---|---|
| **Objective** | Quantify cognitive performance and prevent regression. |
| **Inputs** | System telemetry, model outputs, benchmarking data. |
| **Outputs** | Scorecards, alerting thresholds. |
| **Dependencies** | Event Bus (OpenTelemetry). |
| **Failure Modes** | Goodhart's Law (optimizing for the metric, not the goal). |
| **Recovery** | Rotate evaluation datasets periodically. |
| **KPIs** | Evaluation Suite Run Time. |
| **Extension Points** | Custom enterprise evaluation metrics. |

---

## Cognitive Scorecards

Rather than measuring simple "Lines of Code Written", Atlas tracks intelligence vectors.

### 1. Planning Quality Scorecard
- **DAG Viability Rate:** % of plans that do not contain circular dependencies.
- **Dependency Accuracy:** % of plans where the Execution Engine didn't have to dynamically insert a missing prerequisite task.
- **Replanning Frequency:** Average number of times a plan is rewritten during execution (lower is better).

### 2. Reasoning Quality Scorecard
- **Decision Accuracy:** % of times the Decision Engine chose the optimal path compared to human evaluation.
- **Reflection Effectiveness:** % of times a task succeeds on the retry *after* the Self-Correction Engine executes a Deep Loop reflection.
- **Hallucination Rate:** Frequency of agents attempting to read/write files that do not exist.

### 3. Context & Retrieval Scorecard
- **Context Precision:** Ratio of useful tokens to total tokens in the LLM context window (measured via LLM self-reporting).
- **Retrieval Hit Rate:** % of times a required fact was retrieved on the first RAG++ query.
- **Context Latency:** P95 time to assemble a 32k token context payload.

### 4. Agent Collaboration Scorecard
- **Delegation Success:** % of subtasks successfully completed by a secondary agent called by a primary agent.
- **Communication Overhead:** Ratio of tokens spent talking between agents vs. tokens spent generating code.

---

## Continuous Evaluation (LLM-as-a-Judge)

Because human grading is slow, Atlas uses **LLM-as-a-Judge** (specifically DeepSeek-R1 running offline) to evaluate Qwen3-Coder's outputs continuously. 

When a task completes, an asynchronous job samples the interaction and asks the Judge:
> *"Rate the conciseness of Qwen3's code from 1-5. Did it introduce unnecessary dependencies?"*

---

## Traceability
- Provides the metrics for every other Cognitive Engine.
- Integrates with the [Benchmark Suite](benchmark-suite.md) for automated CI/CD gating.
