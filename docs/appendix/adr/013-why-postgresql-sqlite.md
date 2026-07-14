# ADR-013: Why PostgreSQL + SQLite

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Technology, Database

---

## Context

Atlas needs relational storage for project metadata, task execution queues, audit logs, configuration, and agent state. The system must support two distinct deployment profiles:
1. **Solo Developer (Local):** Wants zero configuration, instant startup, and no background services.
2. **Team/Enterprise (Distributed):** Needs high concurrency, massive data scaling, and centralized state.

## Decision

**We will support a dual database strategy: SQLite for local-first/embedded deployments, and PostgreSQL for distributed/production deployments.**

The persistence layer will be abstracted via the Repository Pattern so business logic is unaware of the underlying database.

## Alternatives Considered

### Alternative 1: SQLite Only

**Pros:** Perfect for the local-first requirement; zero configuration; single file.
**Cons:** Struggles with high concurrency (database locked errors) if multiple distributed agents are trying to update the execution queue simultaneously. Cannot scale for the enterprise roadmap phase.

### Alternative 2: PostgreSQL Only

**Pros:** Enterprise-grade reliability, excellent JSONB support, handles massive concurrency.
**Cons:** Requires installing and running a database server. Violates the "zero-configuration desktop app" experience for individual developers.

### Alternative 3: NoSQL / Document Store (e.g., MongoDB)

**Pros:** Flexible schema.
**Cons:** Loses ACID guarantees on complex multi-table transactions (e.g., updating a task state while appending an audit log and changing agent status). Slower relational queries.

## Rationale

The dual strategy is chosen because:
1. **Best of Both Worlds:** We fulfill the local-first, zero-friction install requirement with SQLite, while preserving the migration path to enterprise scale with PostgreSQL.
2. **SQLx Abstraction:** Rust's `sqlx` library makes it manageable to support both dialects. By keeping our SQL relatively standard and avoiding database-specific extensions in the core domain, we can maintain both adapters with minimal overhead.
3. **Schema Migrations:** Tools like `sqlx` or `sea-orm` handle migrations for both databases using the same declarative schema models.

## Consequences

### Positive
- Frictionless onboarding for solo users.
- Clear path to scale for enterprise deployments.
- Enforces strict Clean Architecture (AP-01), as the domain cannot rely on DB-specific features (like Postgres triggers).

### Negative
- **Maintenance Overhead:** The team must maintain and test two separate database adapters and ensure SQL queries are compatible with both dialects.
- **Feature Constraint:** We cannot heavily utilize PostgreSQL-only features (like `LISTEN/NOTIFY` or advanced array operators) without writing complex fallback logic for SQLite.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | SQLite fulfills this perfectly. |
| AP-02: Hexagonal Arch | ✅ | Enforces strict port/adapter separation for data access. |

## Related Documents

- [Technology Selection Framework](../08-governance/technology-selection-framework.md)
- [Database Architecture](../../04-subsystems/database-architecture.md)
