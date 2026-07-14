# Definition of Done

> **Version:** 1.0.0  
> **Status:** ✅ Complete  
> **Last Updated:** 2026-07-14  
> **Owner:** Architecture Team  
> **Upstream:** [Engineering Principles](engineering-principles.md), [Quality Gates](quality-gates.md)

---

## Purpose

This document defines **what "done" means** at every level of Atlas development. "Done" is not a subjective judgment — it is a checklist. If any item is unchecked, the work is not done.

---

## Definition of Done: Code Change (PR Level)

A code change is **done** when ALL of the following are true:

### Correctness
- [ ] The change implements the intended behavior as described in the ticket/task
- [ ] All new code paths have been manually tested by the author
- [ ] Edge cases have been identified and handled
- [ ] Error cases return appropriate error types with actionable messages

### Testing
- [ ] Unit tests cover all new public functions and methods
- [ ] Integration tests cover cross-module interactions (if applicable)
- [ ] All existing tests pass (zero regressions)
- [ ] Code coverage for modified files is ≥ 80%
- [ ] Tests are deterministic (no flaky tests)

### Code Quality
- [ ] Code follows the [Coding Standards](../05-engineering/coding-standards.md)
- [ ] Linting passes with zero warnings
- [ ] Formatting is applied (rustfmt, black, prettier)
- [ ] No compiler/type-checker warnings
- [ ] No `TODO` or `FIXME` without an associated ticket
- [ ] No hardcoded values — constants or configuration used

### Documentation
- [ ] Public APIs have doc comments (Rust: `///`, Python: docstrings)
- [ ] Complex logic has inline comments explaining "why," not "what"
- [ ] README updated if the change affects setup or usage
- [ ] CHANGELOG entry added for user-facing changes

### Security
- [ ] No secrets, credentials, or tokens in the code
- [ ] File system operations are path-validated (within sandbox)
- [ ] Network calls are justified and documented (if any)
- [ ] Input validation is present for all external data

### Architecture
- [ ] Change respects [Architecture Principles](architecture-principles.md)
- [ ] Domain layer has no infrastructure imports
- [ ] No shared mutable state between agents
- [ ] Events are used for cross-boundary communication

### Review
- [ ] PR description explains "why," not just "what"
- [ ] At least one reviewer has approved
- [ ] All review comments are addressed or explicitly deferred (with ticket)

---

## Definition of Done: Feature

A feature is **done** when ALL of the following are true:

- [ ] All code changes for the feature are merged (each meeting PR-level DoD)
- [ ] End-to-end test covers the complete user flow
- [ ] Feature works on all target platforms (Windows, macOS, Linux)
- [ ] Feature works in both SQLite and PostgreSQL modes
- [ ] Performance meets the targets in the [PRD](../02-prd/prd.md)
- [ ] UI is accessible (keyboard navigation, screen reader compatible)
- [ ] Error states have been tested and produce user-friendly messages
- [ ] Feature documentation is written or updated
- [ ] Feature is demonstrated in a dev environment

---

## Definition of Done: Agent

An agent is **done** when ALL of the following are true:

- [ ] Formal contract is defined (Consumes, Produces, Tools, Memory, SLA, KPIs)
- [ ] Agent passes all contract-based tests
- [ ] Agent handles failure cases (model timeout, invalid input, tool failure)
- [ ] Agent respects its permission boundaries (no unauthorized tool usage)
- [ ] Agent's reasoning is logged and traceable
- [ ] Agent's memory access follows defined patterns
- [ ] Agent has been tested in isolation (unit) and in collaboration (integration)
- [ ] Agent's performance meets its SLA
- [ ] Agent's KPIs are measurable via the evaluation framework
- [ ] Agent can be started, stopped, paused, and resumed without data loss

---

## Definition of Done: Bounded Context

A bounded context is **done** when ALL of the following are true:

- [ ] All entities, value objects, and domain events are defined
- [ ] All port interfaces are defined and documented
- [ ] At least one adapter exists for each port
- [ ] Domain logic has zero infrastructure dependencies
- [ ] Context communicates with other contexts only through events or defined query APIs
- [ ] Context has its own test suite with ≥ 80% coverage
- [ ] Context can be developed and tested independently
- [ ] Context's public API is versioned

---

## Definition of Done: Sprint

A sprint is **done** when ALL of the following are true:

- [ ] All committed items meet feature-level DoD
- [ ] No critical or high-severity bugs are open
- [ ] CI pipeline is green on all platforms
- [ ] Performance benchmarks show no regression
- [ ] Sprint review/demo completed
- [ ] Sprint retrospective completed
- [ ] Task board is clean (no items in ambiguous states)

---

## Definition of Done: Release

A release is **done** when ALL of the following are true:

- [ ] All sprint-level DoD criteria are met
- [ ] Release candidate has been tested on all platforms
- [ ] All critical and high-severity bugs from previous release are fixed
- [ ] Migration scripts are tested (database, configuration)
- [ ] CHANGELOG is complete and accurate
- [ ] Release notes are written for users
- [ ] Installation/upgrade path is tested
- [ ] Rollback procedure is documented and tested
- [ ] Security audit is completed for new features
- [ ] Performance benchmarks are within acceptable ranges
- [ ] All quality gates pass (see [Quality Gates](quality-gates.md))

---

## Definition of Done: Documentation (EKB)

A document is **done** when ALL of the following are true:

- [ ] Content is technically accurate and current
- [ ] Diagrams (Mermaid) render correctly
- [ ] Cross-references to other documents are valid
- [ ] ADR references are included where decisions are referenced
- [ ] Document follows the EKB formatting conventions
- [ ] At least one reviewer has approved
- [ ] Master index is updated with document status

---

## "Production Ready" Definition

A component is **production ready** when:

1. ✅ It meets the relevant DoD (PR + Feature + Agent/Context as applicable)
2. ✅ It has survived at least one week of dog-fooding (used by the development team on real projects)
3. ✅ It has been profiled for memory leaks and resource exhaustion
4. ✅ It handles 10x expected load without degradation
5. ✅ It recovers from crashes within 5 seconds
6. ✅ It has zero known critical bugs
7. ✅ It has a monitoring dashboard with alerts for key metrics
8. ✅ It has a runbook for common failure scenarios

---

*This document defines completion criteria. For the gates that enforce them, see [Quality Gates](quality-gates.md). For the standards that guide implementation, see [Coding Standards](../05-engineering/coding-standards.md).*
