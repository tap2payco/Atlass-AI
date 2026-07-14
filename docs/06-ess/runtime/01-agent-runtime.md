# 01 Agent Runtime

> **Version:** 1.0.0-draft  
> **Status:** 🚧 Draft  
> **Created:** 2026-07-14  

---

## Overview
The Agent Runtime is the core execution loop for AI agents in Atlas. It is a **Stateless Data Plane** component that manages the prompt lifecycle, tool execution boundaries, and context resolution for an individual agent invocation.

---

## Runtime Dependency Rules

Every invocation explicitly defines its dependency boundaries:
- **May Call:** Prompt Runtime, Context Runtime, Tool Runtime, Memory Runtime, Plugin Runtime.
- **Cannot Call:** Model Runtime (Ollama/vLLM directly), Database (SQLite/PostgreSQL directly), Filesystem (directly), Orchestrator.

---

## Identity

Every execution in the Agent Runtime must have a cryptographically verifiable identity. Nothing executes anonymously.

- **Invocation ID:** Unique ID for this specific execution attempt.
- **Session ID:** Ties to the broader user session.
- **Task ID:** The orchestrator task this invocation fulfills.
- **Workflow ID:** Optional. Ties to a long-running DAG.
- **Agent ID:** The specific agent profile being used (e.g., `Architect`, `Coder`).
- **Model ID:** The target model for this invocation (e.g., `deepseek-r1`, `qwen3-coder`).
- **Workspace ID:** Boundary scope for the agent.
- **Repository ID:** Target repository.
- **Correlation ID:** For distributed tracing.
- **Trace ID:** OpenTelemetry trace identifier.

---

## Runtime Context

The Agent Runtime receives a strictly defined, immutable `AgentInvocationRequest` containing:

- **System Context:** Current time, OS, core configuration.
- **Architecture Context:** Active ADRs and system constraints.
- **Repository Context:** Bounded view of the codebase.
- **Memory Context:** Relevant prior learnings or decisions.
- **Retrieved Context:** RAG results provided by the Context Runtime.
- **User Request:** The prompt or task from the user.
- **Execution Constraints:** 
  - **Security Policy:** Sandbox rules.
  - **Token Budget:** Maximum tokens allowed.
  - **Timeout:** Maximum execution time in milliseconds.
  - **Cancellation Token:** For early termination.

---

## Runtime Pipeline

The agent executes through a strict, multi-stage internal pipeline. It does not simply pass text to an LLM.

```text
Receive Invocation
       ↓
Validate Identity & Policies
       ↓
Resolve Context (Context Runtime)
       ↓
Retrieve Memory (Memory Runtime)
       ↓
Assemble Prompt (Prompt Runtime)
       ↓
Execute Model (Prompt Runtime → Model Runtime)
       ↓
Validate Output Syntax & Types
       ↓
Execute Tools (Tool Runtime)
       ↓
Review Results
       ↓
Emit Events (AgentCompleted / AgentFailed)
       ↓
Persist Memory (Memory Runtime)
       ↓
Return Result
```

---

## Runtime Policies

The Agent Runtime governs its behavior via externalized policies provided by the Policy Runtime:

- **Retry Policy:** Defines exponential backoff and max attempts for network/model failures.
- **Cancellation Policy:** Defines how graceful termination is handled.
- **Memory Policy:** Dictates what insights must be persisted.
- **Prompt Policy:** Defines system prompt injections.
- **Security Policy:** Defines the sandbox, tool allowlist, and data classification.
- **Token Policy:** Defines truncation and budget limits.
- **Timeout Policy:** Defines hard limits on invocation execution.
- **Fallback Policy:** Defines model fallback chains (e.g., if DeepSeek fails, fallback to Qwen).

---

## Runtime Capability Registry

Agents are selected by the Execution Runtime based on capabilities, not hardcoded logic.

**Example Capabilities:**
- `Planning`
- `CodeGeneration:Rust`
- `DependencyAnalysis`
- `SecurityReview`

The Agent Runtime verifies the assigned `Agent ID` has the required capabilities to fulfill the task.

---

## Runtime Resource Manager

The Agent Runtime monitors resources for its specific invocation and reports limits to the Execution Runtime:
- **Token Budget:** Tracks input/output token usage per invocation.
- **Time In-Flight:** Wall-clock time.
- **Memory (RAM):** Sandbox memory limits (if applicable).

---

## Runtime Security

Validated via the **Policy Runtime**:
- **Allowed Tools:** Explicit list (e.g., `[view_file, run_command]`).
- **Forbidden Tools:** Explicit list (e.g., `[delete_repository]`).
- **Filesystem Scope:** e.g., `/workspace/Atlas/apps/server` (Read/Write).
- **Network Scope:** e.g., `DENY_ALL` except `localhost:4222`.
- **Secrets Access:** Redacted by default.
- **Sandbox Profile:** e.g., `restricted-agent-v1`.

---

## Runtime Contracts (Machine-Readable)

The authoritative contracts for the Agent Runtime live in the `schemas/` directory.

**See Schemas:**
- [`schemas/agent/AgentInvocationRequest.schema.json`](schemas/agent/AgentInvocationRequest.schema.json)
- [`schemas/agent/AgentInvocationResponse.schema.json`](schemas/agent/AgentInvocationResponse.schema.json)
- [`schemas/config/ExecutionPolicy.schema.json`](schemas/config/ExecutionPolicy.schema.json)

---

## Runtime Conformance Checklist

- [ ] Public interface defined
- [ ] Inputs documented
- [ ] Outputs documented
- [ ] Events documented
- [ ] Errors documented
- [ ] Metrics documented
- [ ] Policies referenced
- [ ] Security reviewed
- [ ] Tests defined
- [ ] Benchmarks defined
- [ ] Machine-readable contracts linked
- [ ] ADR traceability complete
