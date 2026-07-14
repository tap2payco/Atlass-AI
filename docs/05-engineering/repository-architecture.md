# Repository Blueprint & Architecture

> **Version:** 1.0.0-draft  
> **Status:** 🚧 Draft  
> **Created:** 2026-07-14  

---

## 1. Repository Philosophy

Atlas is structured as a **monorepo**. This approach ensures that all runtimes, frontends, schemas, and configurations are versioned together, enabling atomic commits across the Control Plane and Data Plane.

**Design Principles:**
1. **Strict Modularity:** Packages do not leak internals. Public APIs must be explicitly defined.
2. **Deterministic Dependencies:** Packages cannot have circular dependencies.
3. **Architecture as Code:** Schemas and machine-readable contracts are stored alongside the code that implements them.
4. **No Hidden Logic:** Every module maps directly to a specification in the Engineering Specification Suite (ESS).

---

## 2. Directory Tree Structure

The physical layout strictly adheres to the frozen architecture.

```text
Atlas/
│
├── .github/                   # CI/CD Pipelines
├── apps/                      # Executables and Entrypoints
│   ├── desktop/               # Tauri + React desktop shell
│   ├── cli/                   # Command-line interface
│   └── server/                # Headless API server
│
├── packages/                  # Core Subsystems
│   ├── orchestrator/          # Event router and DAG manager
│   ├── runtime-execution/     # Execution Runtime
│   ├── runtime-agent/         # Agent Runtime
│   ├── runtime-context/       # Context Runtime
│   ├── runtime-memory/        # Memory Runtime
│   ├── runtime-repository/    # Repository Runtime
│   ├── runtime-model/         # Model Runtime
│   ├── runtime-prompt/        # Prompt Runtime
│   ├── runtime-tool/          # Tool Runtime
│   ├── runtime-policy/        # Policy Runtime
│   ├── runtime-workflow/      # Workflow Runtime
│   ├── runtime-plugin/        # Plugin Runtime
│   ├── evaluation/            # Benchmarks and evaluation tools
│   ├── sdk/                   # Plugin and Extension SDK
│   └── shared/                # Core domain types and utilities
│
├── schemas/                   # Machine-readable contracts (OpenAPI, JSON Schema, AsyncAPI)
│   ├── events/
│   ├── models/
│   └── policies/
│
├── docs/                      # Engineering Knowledge Base (EKB)
├── tests/                     # Cross-package Integration and E2E Tests
├── scripts/                   # Build and development utilities
├── configs/                   # Default configuration templates
│
├── ENGINEERING_BOARD.md       # Live implementation dashboard
├── Cargo.toml                 # Rust workspace configuration
├── pyproject.toml             # Python workspace configuration
└── package.json               # Node/Frontend workspace configuration
```

---

## 3. Workspace Build Systems

Atlas uses a polyglot monorepo strategy:
- **Rust Workspace (`Cargo.toml`):** Manages all high-performance and control plane components (Orchestrator, Execution, Repository, Tool runtimes).
- **Python Workspace (`pyproject.toml`):** Manages AI inference and Python-native logic (Agent, Prompt, Model runtimes).
- **Node Workspace (`package.json`):** Manages the Tauri/React desktop frontend.

---

## 4. Testing Layout

- **Unit Tests:** Co-located with the source code inside each package (e.g., `packages/runtime-agent/tests/`).
- **Integration Tests:** Located in `tests/integration/`, testing cross-package events via NATS.
- **E2E Tests:** Located in `tests/e2e/`, testing the full user flow from the Desktop/CLI down to the model execution.

---

## 5. Documentation Standards

Code must be self-documenting but strictly annotated:
- **Rust:** Every public trait and struct must have `///` rustdoc annotations.
- **Python:** Every public class and method must have NumPy/Google style docstrings.
- **Traceability:** Every module must link back to its specification in `docs/06-ess/`.

*Once this blueprint is approved, generating the physical repository structure becomes a purely mechanical process.*
