# Prompt Runtime

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-14  
> **Owner:** AI Team  
> **Upstream:** [Context Engine](context-engine.md)

---

## Objective
To dynamically assemble, format, and serialize prompts for inference at runtime. In Atlas, prompts are not static text files; they are compiled data structures built on the fly based on the Agent Profile, the current Context Engine output, and the specific model's dialect requirements (e.g., Qwen vs DeepSeek).

---

## Cognitive Contract

| Section | Specification |
|---|---|
| **Objective** | Compile budgeted context into model-specific prompts. |
| **Inputs** | Budgeted Context Payload, Agent Profile, Tool Definitions. |
| **Outputs** | Formatted array of Messages (Role/Content) or Raw String. |
| **Dependencies** | Context Engine. |
| **Failure Modes** | Malformed XML tags breaking model parsing. |
| **Recovery** | Schema validation before inference. |
| **KPIs** | Prompt Compilation Latency (< 10ms). |
| **Extension Points** | Dialect adapters for new LLMs (e.g., Llama 3 formatting). |

---

## The Assembly Pipeline

The Prompt Runtime builds the final prompt in a strict hierarchical order to maximize the LLM's attention mechanism (which favors the beginning and end of long contexts).

```text
1. SYSTEM INSTRUCTIONS
   (Agent Persona, Core Directives, Output Format)

2. GLOBAL CONSTRAINTS
   (User Preferences, Architecture Memory)

3. AVAILABLE TOOLS
   (JSON Schemas of allowed tools)

4. REPOSITORY CONTEXT
   (AST Skeletons, Graph Context)
   
5. ACTIVE FILE CONTEXT
   (Full text of files being modified)

6. EXECUTION HISTORY
   (Previous tool calls and results)

7. THE IMMEDIATE TASK
   (What the agent needs to do right now)
```

---

## Dynamic Constraints Formulation

Because Atlas uses [Formal Contracts](../03-architecture/agent-architecture.md), the Prompt Runtime dynamically translates those contracts into natural language constraints.

*Example:* If the Agent Contract states `Forbidden Tools: ["git_push"]`, the Prompt Runtime dynamically generates the block:
> `<constraints> You are strictly forbidden from committing or pushing code. You may only output diffs. </constraints>`

## Model Dialect Translation

Different models expect different prompt structures for maximum efficiency.
- **DeepSeek-R1:** Prefers minimal system prompts and explicit `<thought_process>` XML tags in the user prompt to trigger its reasoning loops.
- **Qwen3-Coder:** Excels with standard OpenAI-style ChatML formatting (`<|im_start|>system...`) and strict JSON schema injection for tool calling.

The Prompt Runtime acts as an adapter, translating the universal Atlas context payload into the specific dialect of the active model.

---

## Traceability
- Consumes output from the [Context Engine](context-engine.md).
- Drives the raw input to the [Execution Engine](../03-architecture/execution-engine.md) Inference Port.
