# Repository Manifest

> **Version:** 1.0.0-draft  
> **Status:** 🚧 Draft  
> **Created:** 2026-07-14  

---

## Overview

This manifest is the authoritative inventory of the Atlas repository. It defines the exact purpose, ownership, and boundaries of every package. During the implementation phase (by Qwen3-Coder or human engineers), this manifest must be strictly followed.

---

## 1. Apps

| Name | Purpose | Owner | Dependencies | Public API | Internal API | Status |
|---|---|---|---|---|---|---|
| `apps/desktop` | Tauri React App | Frontend Team | `shared`, `sdk` | IPC Commands | React Components | 🔲 Planned |
| `apps/cli` | Command Line Interface | Core Team | `shared`, `orchestrator` | CLI Args | Command Handlers | 🔲 Planned |
| `apps/server` | Headless REST/WS API | Core Team | `shared`, `orchestrator` | REST, Websockets | HTTP Handlers | 🔲 Planned |

## 2. Core Packages

| Name | Purpose | Owner | Dependencies | Public API | Internal API | Status |
|---|---|---|---|---|---|---|
| `packages/orchestrator` | Event routing & DAG | Control Plane | `runtime-execution`, `shared` | `submit_task()` | Event loops | 🔲 Planned |
| `packages/runtime-execution` | Task lifecycle | Control Plane | `runtime-agent`, `shared` | `schedule_task()` | Queues, workers | 🔲 Planned |
| `packages/runtime-policy` | Security & limits | Control Plane | `shared` | `validate_action()` | Rule engines | 🔲 Planned |
| `packages/runtime-workflow` | Long-running workflows | Control Plane | `runtime-execution` | `start_workflow()` | State checkpointing | 🔲 Planned |
| `packages/runtime-agent` | Agent loop & pipeline | Data Plane | `runtime-prompt`, `runtime-context` | `invoke_agent()` | Pipeline stages | 🔲 Planned |
| `packages/runtime-context` | Context assembly | Data Plane | `runtime-memory`, `runtime-repo` | `resolve_context()` | Chunking, pruning | 🔲 Planned |
| `packages/runtime-memory` | Memory cache & persistence | Data Plane | `shared` | `store_memory()`, `query()` | DB connections | 🔲 Planned |
| `packages/runtime-repository` | AST & symbol graph | Data Plane | `shared` | `get_symbol_graph()` | Tree-sitter parsers | 🔲 Planned |
| `packages/runtime-prompt` | Prompt generation | Data Plane | `runtime-model` | `generate_prompt()` | Jinja/formatters | 🔲 Planned |
| `packages/runtime-model` | LLM execution (Ollama) | Data Plane | `shared` | `execute_inference()` | API clients | 🔲 Planned |
| `packages/runtime-tool` | Tool execution & sandbox | Data Plane | `shared` | `execute_tool()` | Subprocess/Docker | 🔲 Planned |
| `packages/runtime-plugin` | Extensibility sandbox | Data Plane | `sdk` | `invoke_plugin()` | WASM/Sandbox | 🔲 Planned |
| `packages/shared` | Core types and utils | Framework | None | Structs, traits, enums | N/A | 🔲 Planned |
| `packages/sdk` | Plugin extension SDK | Framework | `shared` | Traits for plugins | N/A | 🔲 Planned |

---

*Do not create any new package or module that is not listed in this manifest. If a new module is required, submit an ADR and update this manifest first.*
