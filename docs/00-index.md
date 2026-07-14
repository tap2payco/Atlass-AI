# Atlas Engineering Knowledge Base (EKB)

> **Version:** 1.0.0-draft  
> **Status:** Living Document  
> **Created:** 2026-07-14  
> **Classification:** Internal — Engineering Reference  
> **Authority:** This is the authoritative source of truth for the Atlas AI Engineering System. Every implementation decision, architectural change, and engineering practice must trace back to the EKB.

---

## What is Atlas?

Atlas is a **local-first AI Software Engineering Platform** powered by multiple specialized AI agents that collaborate to design, build, test, debug, document, and improve software. It is not a chatbot. It is not an autocomplete tool. It is an **autonomous engineering system**.

---

## How to Use This Knowledge Base

| You are... | Start here |
|---|---|
| **New contributor** | [Vision](01-vision/vision.md) → [Development Playbook](05-engineering/development-playbook.md) → [Implementation Guide](../IMPLEMENTATION_GUIDE.md) |
| **Architect** | [System Architecture](03-architecture/system-architecture.md) → [Architecture Principles](08-governance/architecture-principles.md) → [ADRs](appendix/adr/) |
| **AI/ML Engineer** | [Agent Architecture](03-architecture/agent-architecture.md) → [AI Reasoning](09-ai/reasoning-framework.md) → [Memory System](03-architecture/memory-system.md) |
| **Backend Engineer** | [Backend Architecture](04-subsystems/backend-architecture.md) → [API Documentation](06-specifications/api-documentation.md) → [Data Models](06-specifications/data-models.md) |
| **Frontend Engineer** | [Frontend Architecture](04-subsystems/frontend-architecture.md) → [Design System](04-subsystems/frontend-architecture.md#design-system) |
| **DevOps/Infra** | [Deployment](05-engineering/deployment-architecture.md) → [Observability](05-engineering/observability.md) → [Security](04-subsystems/security-architecture.md) |
| **Product Manager** | [Vision](01-vision/vision.md) → [PRD](02-prd/prd.md) → [Roadmap](07-roadmap/implementation-roadmap.md) |
| **Plugin Developer** | [Plugin SDK](10-plugin-sdk/plugin-sdk.md) → [Plugin Security](10-plugin-sdk/plugin-security.md) |

---

## Document Index

### 01 — Vision & Strategy

| Document | Purpose | Status |
|---|---|---|
| [Vision](01-vision/vision.md) | Mission, goals, non-goals, success criteria, competitive analysis, roadmap | ✅ Complete |

### 02 — Product Requirements

| Document | Purpose | Status |
|---|---|---|
| [PRD](02-prd/prd.md) | Functional/non-functional requirements, user stories, acceptance criteria, risks | ✅ Complete |

### 03 — Core Architecture

| Document | Purpose | Status |
|---|---|---|
| [System Architecture](03-architecture/system-architecture.md) | Overall system design, layers, boundaries, data flow | 🔲 Phase 2 |
| [Agent Architecture](03-architecture/agent-architecture.md) | All 15+ agents with formal contracts | 🔲 Phase 2 |
| [Agent Lifecycle](03-architecture/agent-lifecycle.md) | State machine: Created → Archived | 🔲 Phase 2 |
| [Orchestrator Design](03-architecture/orchestrator-design.md) | Scheduling, dependency management, conflict resolution | 🔲 Phase 2 |
| [Execution Engine](03-architecture/execution-engine.md) | Task queue, priority, retries, rollback, state machine | 🔲 Phase 2 |
| [Communication Protocol](03-architecture/communication-protocol.md) | Agent message format, pub/sub, correlation IDs | 🔲 Phase 2 |
| [Memory System](03-architecture/memory-system.md) | Working/long-term/repo/decision memory | 🔲 Phase 3 |
| [RAG Architecture](03-architecture/rag-architecture.md) | Indexing, chunking, embeddings, re-ranking | 🔲 Phase 3 |
| [AI Pipeline](03-architecture/ai-pipeline.md) | Prompt → Reasoning → Execution → Review | 🔲 Phase 3 |
| [Workspace Model](03-architecture/workspace-model.md) | Workspace, projects, sessions, repositories, artifacts | 🔲 Phase 2 |

### 04 — Subsystems

| Document | Purpose | Status |
|---|---|---|
| [Tooling Architecture](04-subsystems/tooling-architecture.md) | Terminal, Git, filesystem, Docker, plugins, sandboxing | 🔲 Phase 4 |
| [Backend Architecture](04-subsystems/backend-architecture.md) | API, auth, DI, events, persistence | 🔲 Phase 4 |
| [Frontend Architecture](04-subsystems/frontend-architecture.md) | Desktop, web, state management, design system | 🔲 Phase 4 |
| [Database Architecture](04-subsystems/database-architecture.md) | Schema, storage, migrations, backup | 🔲 Phase 4 |
| [Security Architecture](04-subsystems/security-architecture.md) | Threat model, sandboxing, permissions, audit | 🔲 Phase 4 |
| [Repository Intelligence](04-subsystems/repository-intelligence.md) | AST, dependency graph, symbol graph, call graph | 🔲 Phase 3 |

### 05 — Engineering Practices

| Document | Purpose | Status |
|---|---|---|
| [Repository Architecture](05-engineering/repository-architecture.md) | Monorepo structure, naming conventions | 🚧 Draft |
| [Repository Manifest](05-engineering/repository-manifest.md) | Authoritative inventory of all packages | 🚧 Draft |
| [Repository Bootstrap](05-engineering/repository-bootstrap.md) | Workspace setup, build rules, dependencies | 🚧 Draft |
| [Implementation Guide](../IMPLEMENTATION_GUIDE.md) | Operational handbook for implementing Atlas | 🚧 Draft |
| [Testing Strategy](05-engineering/testing-strategy.md) | Unit, integration, E2E, agent simulation | 🔲 Phase 5 |
| [Deployment Architecture](05-engineering/deployment-architecture.md) | Local, Docker, GPU/CPU, updates | 🔲 Phase 5 |
| [Observability](05-engineering/observability.md) | Metrics, tracing, logging, dashboards | 🔲 Phase 5 |
| [Coding Standards](05-engineering/coding-standards.md) | Naming, review, commit, branching | 🔲 Phase 5 |
| [Development Playbook](05-engineering/development-playbook.md) | Contributor handbook: setup → merge | 🔲 Phase 5 |

### 06 — Engineering Specification Suite (ESS)

| Document | Purpose | Status |
|---|---|---|
| [ESS Overview](06-ess/00-ess-overview.md) | Overview of machine-readable and human-readable specifications | 🚧 Draft |
| [Runtime Subsystem](06-ess/runtime/00-runtime-overview.md) | Control Plane, Data Plane, Policies, Events | 🔲 Phase 4 |
| [Service Specifications](06-ess/02-service-specifications.md) | Planner, Reasoner, Memory, Repository, Tool Registry | 🔲 Phase 4 |
| [Interface Specifications](06-ess/03-interface-specifications.md) | OpenAPI, AsyncAPI, and Event schemas | 🔲 Phase 4 |
| [Data Specifications](06-ess/04-data-specifications.md) | Data models, schemas, and diagrams for workspaces and memory | 🔲 Phase 4 |
| [Module Specifications](06-ess/05-module-specifications.md) | Package responsibilities, interfaces, testing | 🔲 Phase 4 |
| [Infrastructure Specifications](06-ess/06-infrastructure-specifications.md) | Docker, NATS, Qdrant, SQLite, PostgreSQL | 🔲 Phase 4 |
| [Developer Specifications](06-ess/07-developer-specifications.md) | How to add agents, tools, memory, plugins | 🔲 Phase 4 |
| [Engineering Matrix](06-ess/engineering-matrix.md) | Traceability: Requirement → Arch → Spec → Impl → Test | 🔲 Phase 4 |

### 07 — Roadmap

| Document | Purpose | Status |
|---|---|---|
| [Implementation Roadmap](07-roadmap/implementation-roadmap.md) | Phased implementation: ESS, Blueprint, Skeletons, Code | 🚧 Draft |
| [Future Expansion](07-roadmap/future-expansion.md) | Extension points, marketplace, cloud | 🔲 Phase 7 |
| [Architecture Evolution](07-roadmap/architecture-evolution.md) | v1 → distributed → enterprise → federated | 🔲 Phase 7 |

### 08 — Governance

| Document | Purpose | Status |
|---|---|---|
| [Engineering Principles](08-governance/engineering-principles.md) | Core principles governing all decisions | ✅ Complete |
| [Architecture Principles](08-governance/architecture-principles.md) | Architectural constraints and invariants | ✅ Complete |
| [Decision Framework](08-governance/decision-framework.md) | When/how architectural decisions are made | ✅ Complete |
| [Technology Selection Framework](08-governance/technology-selection-framework.md) | Technology evaluation + comparisons | ✅ Complete |
| [Definition of Done](08-governance/definition-of-done.md) | What "production ready" means | ✅ Complete |
| [Quality Gates](08-governance/quality-gates.md) | Gates for PRs, releases, architecture changes | ✅ Complete |
| [Risk Register](08-governance/risk-register.md) | All identified risks with mitigations | ✅ Complete |
| [Engineering Readiness Review (ERR)](08-governance/engineering-readiness-review.md) | Formal gate between specs and implementation | 🚧 Draft |
| [Architecture Freeze (v1.0)](08-governance/architecture-freeze.md) | Formal declaration of architecture stabilization | ✅ Complete |

### 09 — AI Handbook

| Document | Purpose | Status |
|---|---|---|
| [Reasoning Framework](09-ai/reasoning-framework.md) | How agents think: analyze → reflect | 🔲 Phase 3 |
| [Planning Framework](09-ai/planning-framework.md) | Task decomposition, estimation, validation | 🔲 Phase 3 |
| [Prompt Engineering](09-ai/prompt-engineering.md) | Templates, construction, optimization | 🔲 Phase 3 |
| [Tool Calling](09-ai/tool-calling.md) | Tool selection, invocation, result handling | 🔲 Phase 3 |
| [Agent Coordination](09-ai/agent-coordination.md) | Multi-agent collaboration patterns | 🔲 Phase 3 |
| [Reflection](09-ai/reflection.md) | Self-evaluation, learning, improvement | 🔲 Phase 3 |
| [Evaluation](09-ai/evaluation.md) | Benchmarks, scorecards, quality metrics | 🔲 Phase 3 |
| [Memory Updates](09-ai/memory-updates.md) | Write/validate/prune policies | 🔲 Phase 3 |
| [Project Intelligence](09-ai/project-intelligence.md) | Learning coding styles, patterns, habits | 🔲 Phase 3 |

### 10 — Plugin SDK

| Document | Purpose | Status |
|---|---|---|
| [Plugin SDK](10-plugin-sdk/plugin-sdk.md) | SDK overview, manifest, registration, lifecycle | 🔲 Phase 5 |
| [Plugin Security](10-plugin-sdk/plugin-security.md) | Permissions, sandbox, validation | 🔲 Phase 5 |
| [Plugin Marketplace](10-plugin-sdk/plugin-marketplace.md) | Versioning, distribution, discovery | 🔲 Phase 5 |

### Appendix — Architecture Decision Records

| ADR | Decision | Status |
|---|---|---|
| [ADR-000](appendix/adr/000-template.md) | Template | ✅ Complete |
| [ADR-001](appendix/adr/001-multi-agent-architecture.md) | Multi-Agent Architecture | ✅ Complete |
| [ADR-002](appendix/adr/002-why-three-models.md) | Why Three Models | ✅ Complete |
| [ADR-003](appendix/adr/003-why-qwen3-coder.md) | Why Qwen3-Coder | ✅ Complete |
| [ADR-004](appendix/adr/004-why-deepseek-r1.md) | Why DeepSeek R1 | ✅ Complete |
| [ADR-005](appendix/adr/005-why-bge-m3.md) | Why BAAI BGE-M3 | ✅ Complete |
| [ADR-006](appendix/adr/006-local-first-design.md) | Local-First Design | ✅ Complete |
| [ADR-007](appendix/adr/007-why-rust-core.md) | Why Rust for Core | ✅ Complete |
| [ADR-008](appendix/adr/008-why-python-ai-runtime.md) | Why Python for AI Runtime | ✅ Complete |
| [ADR-009](appendix/adr/009-why-tauri.md) | Why Tauri | ✅ Complete |
| [ADR-010](appendix/adr/010-why-event-driven.md) | Why Event-Driven | ✅ Complete |
| [ADR-011](appendix/adr/011-why-nats.md) | Why NATS | ✅ Complete |
| [ADR-012](appendix/adr/012-why-qdrant.md) | Why Qdrant | ✅ Complete |
| [ADR-013](appendix/adr/013-why-postgresql-sqlite.md) | Why PostgreSQL + SQLite | ✅ Complete |
| [ADR-014](appendix/adr/014-why-tree-sitter.md) | Why Tree-sitter | ✅ Complete |
| [ADR-015](appendix/adr/015-why-ollama.md) | Why Ollama | ✅ Complete |
| [ADR-016](appendix/adr/016-development-toolchain-independence.md) | Development Toolchain Independence | ✅ Complete |

---

## Traceability Matrix

Maps every original requirement area to its EKB document(s).

| # | Required Area | Primary Document(s) | ADR(s) |
|---|---|---|---|
| 1 | Vision | [Vision](01-vision/vision.md) | — |
| 2 | PRD | [PRD](02-prd/prd.md) | — |
| 3 | Technical Architecture | [System Architecture](03-architecture/system-architecture.md) | ADR-001, ADR-006, ADR-010 |
| 4 | Agent Architecture | [Agent Architecture](03-architecture/agent-architecture.md), [Agent Lifecycle](03-architecture/agent-lifecycle.md) | ADR-001, ADR-002 |
| 5 | Orchestrator Design | [Orchestrator](03-architecture/orchestrator-design.md), [Execution Engine](03-architecture/execution-engine.md) | ADR-010 |
| 6 | Memory System | [Memory System](03-architecture/memory-system.md), [Memory Updates](09-ai/memory-updates.md) | ADR-005, ADR-012 |
| 7 | RAG Architecture | [RAG Architecture](03-architecture/rag-architecture.md), [Repository Intelligence](04-subsystems/repository-intelligence.md) | ADR-005, ADR-014 |
| 8 | Tooling Architecture | [Tooling Architecture](04-subsystems/tooling-architecture.md) | — |
| 9 | Repository Architecture | [Repository Architecture](05-engineering/repository-architecture.md), [Repository Manifest](05-engineering/repository-manifest.md) | — |
| 10 | Backend Architecture | [Backend Architecture](04-subsystems/backend-architecture.md) | ADR-007, ADR-008 |
| 11 | Frontend Architecture | [Frontend Architecture](04-subsystems/frontend-architecture.md) | ADR-009 |
| 12 | Database Architecture | [Database Architecture](04-subsystems/database-architecture.md) | ADR-012, ADR-013 |
| 13 | AI Pipeline | [AI Pipeline](03-architecture/ai-pipeline.md), [Prompt Engineering](09-ai/prompt-engineering.md) | ADR-002, ADR-003, ADR-004 |
| 14 | Security Architecture | [Security Architecture](04-subsystems/security-architecture.md) | — |
| 15 | Testing Strategy | [Testing Strategy](05-engineering/testing-strategy.md), [Evaluation](09-ai/evaluation.md) | — |
| 16 | Deployment Architecture | [Deployment Architecture](05-engineering/deployment-architecture.md) | ADR-006, ADR-015 |
| 17 | Observability | [Observability](05-engineering/observability.md) | — |
| 18 | Coding Standards | [Coding Standards](05-engineering/coding-standards.md), [Development Playbook](05-engineering/development-playbook.md) | — |
| 19 | Runtime Subsystem | [Runtime Subsystem](06-ess/runtime/00-runtime-overview.md) | — |
| 20 | Service Specifications | [Service Specifications](06-ess/02-service-specifications.md) | — |
| 21 | Interface Specifications | [Interface Specifications](06-ess/03-interface-specifications.md) | — |
| 22 | Data Specifications | [Data Specifications](06-ess/04-data-specifications.md) | — |
| 23 | Module Specifications | [Module Specifications](06-ess/05-module-specifications.md) | — |
| 24 | Infrastructure Specifications | [Infrastructure Specifications](06-ess/06-infrastructure-specifications.md) | — |
| 25 | Developer Specifications | [Developer Specifications](06-ess/07-developer-specifications.md) | — |
| 26 | Implementation Roadmap | [Implementation Roadmap](07-roadmap/implementation-roadmap.md) | — |
| 27 | Future Expansion | [Future Expansion](07-roadmap/future-expansion.md), [Architecture Evolution](07-roadmap/architecture-evolution.md) | — |
| +1 | Governance | [08-governance/](08-governance/) (7 documents) | — |
| +2 | AI Reasoning | [09-ai/](09-ai/) (9 documents) | — |
| +3 | Plugin SDK | [10-plugin-sdk/](10-plugin-sdk/) (3 documents) | — |
| +4 | Communication Protocol | [Communication Protocol](03-architecture/communication-protocol.md) | ADR-010, ADR-011 |
| +5 | Workspace Model | [Workspace Model](03-architecture/workspace-model.md) | — |

---

## Runtime Ecosystem

These are the product dependencies that ship with Atlas or are required for it to run. The architecture depends strictly on these components.

| Component | Role | ADR |
|---|---|---|
| **Qwen3-Coder** | Primary implementation engineer | [ADR-003](appendix/adr/003-why-qwen3-coder.md) |
| **DeepSeek-R1** | Planning, reasoning, review | [ADR-004](appendix/adr/004-why-deepseek-r1.md) |
| **BGE-M3** | Embeddings, repo intelligence, memory | [ADR-005](appendix/adr/005-why-bge-m3.md) |
| **Rust** | Core Orchestration & Desktop | [ADR-007](appendix/adr/007-why-rust-core.md) |
| **Python** | AI Runtime & Agents | [ADR-008](appendix/adr/008-why-python-ai-runtime.md) |
| **Tauri / React** | Desktop Shell & Frontend | [ADR-009](appendix/adr/009-why-tauri.md) |
| **PostgreSQL / SQLite** | Primary Database (Prod / Local) | [ADR-013](appendix/adr/013-why-postgresql-sqlite.md) |
| **Qdrant** | Vector Database | [ADR-012](appendix/adr/012-why-qdrant.md) |
| **Tree-sitter** | Repository Parsing | [ADR-014](appendix/adr/014-why-tree-sitter.md) |
| **NATS** | Event Messaging | [ADR-011](appendix/adr/011-why-nats.md) |
| **Ollama** | Local Model Inference | [ADR-015](appendix/adr/015-why-ollama.md) |

---

## Development Ecosystem

These are the engineering tools used to build Atlas. They are explicitly excluded from the Runtime Ecosystem to prevent vendor lock-in and ensure toolchain independence ([ADR-016](appendix/adr/016-development-toolchain-independence.md)).

- **Antigravity** (Agentic IDE assistant)
- **Claude** (AI Architect & Senior Engineering Consultant)
- **Git** (Version control)
- **VS Code** (Primary IDE)
- **Docker** (Builds and sandboxing)
- **CI/CD Pipelines** (Testing and deployment)

---

## Conventions

- **Relative links** connect all documents — no absolute paths within the EKB
- **ADR references** appear in every document that relies on a major decision
- **Mermaid diagrams** are used wherever visual clarity exceeds prose
- **Tables** are used for structured comparisons, contracts, and specifications
- **Version headers** appear on every document for change tracking
- **Status badges** track document maturity: 🔲 Planned → 🚧 Draft → ✅ Complete

---

*This index is automatically updated as new documents are added to the EKB.*
