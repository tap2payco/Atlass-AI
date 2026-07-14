# Repository Bootstrap Specification

> **Version:** 1.0.0-draft  
> **Status:** 🚧 Draft  
> **Created:** 2026-07-14  

---

## Overview

Before creating the physical repository, we must define exactly how the workspaces, builds, configurations, and dependencies are managed. This document acts as the final gate before the **Package Structure Freeze v1.0** and the subsequent layered repository generation.

---

## 1. Workspace Strategy

Atlas utilizes a polyglot monorepo structure, requiring explicit boundaries and shared management for three ecosystems:

- **Rust Workspace (`Cargo.toml`):**
  - Manages the Control Plane and heavy Data Plane components (Orchestrator, Execution, Repository runtimes).
  - Uses `[workspace.dependencies]` at the root to share versions (e.g., `tokio = "1.0"`, `serde = "1.0"`).
- **Python Workspace (`pyproject.toml`):**
  - Manages AI inference runtimes (Agent, Prompt, Model runtimes).
  - Uses `uv` or `poetry` as the primary workspace manager.
- **Node Workspace (`package.json`):**
  - Manages the Tauri/React desktop application.
  - Uses `pnpm` workspaces for isolated package environments.

---

## 2. Build Strategy

Every package inside Atlas must adhere to a standard build lifecycle. When Qwen3-Coder implements a module, it must test against these explicit commands.

| Package | Language | Build | Test | Lint | Format | Docs |
|---|---|---|---|---|---|---|
| `orchestrator` | Rust | `cargo build -p orchestrator` | `cargo test -p orchestrator` | `cargo clippy -p orchestrator` | `cargo fmt` | `cargo doc` |
| `runtime-execution` | Rust | `cargo build -p runtime-execution` | `cargo test -p runtime-execution` | `cargo clippy` | `cargo fmt` | `cargo doc` |
| `runtime-agent` | Python | `N/A` | `pytest packages/runtime-agent` | `ruff check` | `ruff format` | `mkdocs` |
| `runtime-prompt` | Python | `N/A` | `pytest packages/runtime-prompt` | `ruff check` | `ruff format` | `mkdocs` |
| `apps/desktop` | TS/Rust | `pnpm build` | `pnpm test` | `pnpm lint` | `prettier --write` | `N/A` |

---

## 3. Configuration Strategy

Configuration is strictly centralized. No module may arbitrarily define its own config files hidden deep in the package directory.

All configurations live in the repository root under `configs/`:

```text
configs/
├── development/      # Local testing, SQLite, Mock LLMs
├── production/       # Hardened settings, PostgreSQL
├── testing/          # CI/CD profiles, deterministic mocks
├── models/           # Ollama/vLLM endpoints, token limits
├── security/         # Sandbox policies, allowed tools
└── logging/          # OpenTelemetry config, log levels
```

Modules expect configurations to be injected at startup or fetched from the centralized `ConfigurationManager`.

---

## 4. Dependency Rules

To prevent architectural drift and circular dependencies, the following rules apply globally:

1. **Language Boundaries:** 
   - Rust packages **cannot** directly import Python packages.
   - Python packages **cannot** directly call Rust packages.
   - All cross-language communication must occur over NATS (the Orchestrator).
2. **Data Plane vs Control Plane:**
   - Data Plane runtimes (e.g., Agent Runtime) **cannot** import Control Plane runtimes (e.g., Execution Runtime).
   - Control Plane runtimes orchestrate the Data Plane.
3. **Shared Contracts:**
   - All shared data types (e.g., `AgentInvocationRequest`) live in `packages/shared`.
   - Packages depend on `shared`, but `shared` depends on **nothing**.
4. **Module Isolation:**
   - Packages interact strictly through public interfaces/traits. Internal APIs are isolated.

---

## 5. Repository Generation Layers

The repository will not be generated via an unconstrained prompt. It will be generated in 8 explicit layers:

1. **Layer 1:** Repository root (README, workspaces, gitignore).
2. **Layer 2:** Directory structure only (no code).
3. **Layer 3:** Workspace configurations (`Cargo.toml`, etc.).
4. **Layer 4:** Shared packages (Interfaces, Contracts).
5. **Layer 5:** Runtime packages (Skeleton only).
6. **Layer 6:** Tests.
7. **Layer 7:** CI/CD.
8. **Layer 8:** Implementation (Sprint 1).

---

> **Declaration: Package Structure Freeze v1.0**  
> Upon approval of this document, the directory and package structure of Atlas is frozen. Changing folders or package boundaries is highly destructive to monorepo state. Any structural modification post-freeze requires a formal ADR.
