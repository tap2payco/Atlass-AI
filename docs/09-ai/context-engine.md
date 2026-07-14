# Context Engine

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team  
> **Upstream:** [Cognitive Architecture](cognitive-architecture.md), [RAG++ Architecture](../03-architecture/rag-plus-plus.md)

---

## Objective
To act as the bouncer for the LLM's context window. LLMs drown in irrelevant information. The Context Engine dynamically compresses, ranks, and fits the outputs of the RAG++ pipeline and the Workspace Model into a strict token budget before passing it to the Prompt Runtime.

---

## Cognitive Contract

| Section | Specification |
|---|---|
| **Objective** | Maximize signal-to-noise ratio in the LLM context window. |
| **Inputs** | Raw Context blocks (from RAG++), Agent Profile limits. |
| **Outputs** | Budgeted, token-counted context payload. |
| **Dependencies** | Tiktoken (or equivalent tokenizer), RAG++. |
| **Failure Modes** | Truncating critical instructions to fit code snippets. |
| **Recovery** | Hard prioritization rules (System Prompt > Code). |
| **KPIs** | Context Precision (%), Token Utilization Efficiency (%). |
| **Extension Points** | Pluggable context compression algorithms (e.g., LLMLingua). |

---

## Token Budgeting Strategy

If an agent has a 32k context window, the Context Engine does not simply fill it up. It budgets it strictly:

| Context Type | Budget Allocation | Eviction Policy |
|---|---|---|
| **System Prompt & Agent Contract** | 10% | **Never evicted.** |
| **Task Definition & Goal** | 5% | **Never evicted.** |
| **Architecture & Decision Memory** | 15% | Trimmed by relevance. |
| **Active File(s)** | 40% | Core files kept; secondary files compressed to outlines (symbols only). |
| **Execution History / Scratchpad** | 20% | Sliding window (oldest evicted first). |
| **Generation Buffer (Output)** | 10% | Reserved for LLM output. |

---

## Context Compression Techniques

When the RAG++ pipeline returns 50k tokens of relevant data for a 32k window, the Context Engine applies progressive compression:

1. **AST Skeletonization:** Instead of injecting the full text of a referenced file, the engine uses Tree-sitter to strip the file down to just its class and function signatures, removing all internal implementation logic.
2. **Memory Summarization:** Sends a quick request to Qwen3 (via the Background Worker) to summarize 5 long conversational turns into a single paragraph before injection.
3. **Hard Truncation:** If all else fails, trims from the bottom of the lowest-priority bucket (Execution History).

---

## Context Expiration

Context is not static during a task. If an agent emits a `file_read` tool call, the newly read file enters the `Active File` bucket, displacing the least relevant previous file. The Context Engine recalculates the budget on every single step of the Agent Lifecycle Execution Loop.

---

## Traceability
- Feeds directly into the [Prompt Runtime](prompt-runtime.md).
- Consumes output from [RAG++ Architecture](../03-architecture/rag-plus-plus.md).
