# Implementation Guide

This guide is the operational handbook for implementing Atlas. Whether you are a human engineer, Antigravity, or Qwen3-Coder, you must follow this playbook exactly. 

**This is not architecture.** This is the mechanical process for building Atlas.

---

## 1. How to Start a Sprint

Before touching any code, check the `ENGINEERING_BOARD.md` to identify the current active sprint and module (e.g., `Sprint 1: Repository Runtime`).

Ensure that the previous sprint is marked as **🟢 Complete** across all quality gates. We do not multi-task sprints.

## 2. How to Select a Module

Find the module assigned to the current sprint.
1. Read the **Repository Manifest** (`docs/05-engineering/repository-manifest.md`) to verify ownership, dependencies, and expected APIs.
2. Read the **Engineering Specification Suite** (`docs/06-ess/`) for the module you are building. This is the contract you are fulfilling.

## 3. How to Implement Against the EKB

Do not invent architecture while coding.

If the specification says the `Agent Runtime` requires a `TokenPolicy`, you must implement it exactly as defined. If you discover the specification is missing crucial details, **stop coding**.

- **Implementation Prompt:** *Always remind AI agents:* "Atlas Architecture Baseline v1.0 is frozen. Implement only the specified module according to the EKB. Do not redesign the architecture."

### 2. Contracts Over Implementation

Atlas components communicate exclusively through well-defined contracts (Events, Traits, Schemas). No module should know *how* another module does its job, only *what* it returns.

### 3. Parser Independence Principle

> **External runtimes must never depend on parser implementation details. Repository Runtime owns parsing; every other runtime consumes stable entities, repository profiles, syntax abstractions, and immutable snapshots.**

This ensures the Repository Runtime remains modular and parser engines (e.g. tree-sitter) can be swapped out transparently.

## 4. How to Propose an ADR

If you encounter an impossible implementation hurdle or a fatal architectural flaw:
1. Do not "hack" a workaround in the code.
2. Draft an **Architecture Decision Record (ADR)** using `docs/appendix/adr/000-template.md`.
3. Submit the ADR to the Architecture Team for review.
4. Only after the ADR is accepted may you change the implementation approach.

## 5. How to Run Tests

Before marking a module ready, you must run the standardized build and test commands defined in the **Repository Bootstrap Specification** (`docs/05-engineering/repository-bootstrap.md`).

Example (Rust Control Plane):
```bash
cargo build -p <module>
cargo test -p <module>
cargo clippy -p <module>
```

Example (Python Data Plane):
```bash
pytest packages/<module>
ruff check
```

## 6. How to Update the Engineering Board

As you progress through a sprint, actively update `ENGINEERING_BOARD.md`.

- Update the **Tests** column with line coverage percentage.
- Update the **Benchmarks** column once SLAs are met.
- Mark **Review** as "Yes" when approved.

## 7. How to Know When a Module is Complete

A module is complete only when it satisfies the **Implementation Definition of Done**.

| Requirement | Required |
|---|---|
| Public interfaces implemented | ✅ |
| Unit tests passing | ✅ |
| Integration tests passing | ✅ |
| Benchmarks executed | ✅ |
| Conformance checklist passed | ✅ |
| Logging integrated | ✅ |
| Metrics exported | ✅ |
| Configuration validated | ✅ |
| Error handling complete | ✅ |
| Documentation updated | ✅ |
| Engineering Board updated | ✅ |

## 8. Sprint Reviews

At the end of every sprint, you must produce a short engineering review answering:

1. What was implemented?
2. What changed?
3. What risks remain?
4. What ADRs were added (if any)?
6. Is the module ready for the next dependent runtime?

## 9. Runtime Readiness Review

Before a runtime can officially be considered mature (L3) and acts as a foundation for dependent runtimes, it must pass the Runtime Readiness Review.

### Repository Runtime L3 Criteria
- [ ] **Discovery works:** Correctly identifies workspace roots, ignores `.gitignore`.
- [ ] **Repository profiling works:** Identifies languages, frameworks, toolchains.
- [ ] **Parsing succeeds:** `tree-sitter` successfully generates ASTs wrapped in `AtlasSyntaxTree`.
- [ ] **Symbol extraction succeeds:** AST maps to canonical `SymbolEntity`.
- [ ] **Incremental synchronization works:** `notify` events map to `ChangeType` and trigger partial updates.
- [ ] **Snapshot lineage works:** Snapshots correctly point to `parent_snapshot_id`.
- [ ] **Event emission works:** Correct, versioned schemas emitted for lifecycle events.
- [ ] **Benchmarks meet targets:** Open latency, parse latency, and memory metrics established.
- [ ] **Conformance tests pass:** No architectural violations (e.g., Repository Runtime does not import Agent Runtime).

Only after passing this review should implementation on downstream runtimes (like Context Runtime) begin.
