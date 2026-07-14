# Repository Runtime: Consumer Guide

This engineering contract establishes the integration boundaries for the Repository Runtime (v1.0+). Any runtime (Context, Memory, Planning, Agent) that requires repository intelligence must abide by this guide. 

## 1. What Repository Runtime Guarantees
- **Authoritative Truth:** Repository Runtime is the single source of truth for the project's source code, structure, configuration, and state.
- **Incremental Consistency:** State is updated incrementally. You are guaranteed that the `RepositorySnapshot` reflects the most recent stable parse.
- **Parser Agnosticism:** You will never see a `tree-sitter` node. You will only ever receive canonical `SymbolEntity` and `AtlasSyntaxTree` models.
- **Immutability:** Snapshots, once minted, are strictly immutable. They form a linked lineage (`parent_snapshot_id`).

## 2. Stable APIs (v1.0)
Consumers should only interact with the Repository Runtime via the SDK interfaces defined in `packages/sdk/src/repository.rs`:
- `get_metadata(repo_id)`
- `get_syntax_tree(repo_id, path)`
- `get_symbols(repo_id, path)`
- `get_snapshot(repo_id)`

## 3. Stable Entities & Snapshots
The `packages/shared/src/entities/` hierarchy is stable. You can reliably deserialize the `EntityCollection` within a `RepositorySnapshot` without fear of breaking schema changes.

## 4. Stable Events
The following NATS events are stable and versioned independently:
- `RepositoryOpened`
- `RepositoryProfileUpdated`
- `RepositoryChanged`
- `SnapshotCreated`
- `SnapshotSuperseded`
- `SymbolAdded`
- `SymbolRemoved`
- `SymbolUpdated`

*Note: Always inspect the `version` field of the `EventEnvelope` (e.g. `version: 1`). Downstream processors should route based on this version.*

## 5. What Consumers Must NEVER Do
1. **Never parse files directly:** Do not use regex, grep, or secondary parsing libraries to extract symbols from files. Use the `get_symbols` API.
2. **Never watch the filesystem:** Context/Memory runtimes must not instantiate `notify` or file watchers. Listen for the `RepositoryChanged` or `SnapshotCreated` events over the event bus instead.
3. **Never mutate Snapshots:** The structures returned by the runtime are read-only.

## 6. Performance Expectations
- **Concurrency:** The Repository SDK client is strictly thread-safe (`Send + Sync`).
- **Event Flow:** During branch switches or massive refactors, the Repository Runtime may batch updates. Consumers should expect sudden bursts of `SnapshotSuperseded` events and process them asynchronously.

## 7. Migration Guidance
If the API increments to `v2.0` in the future, a migration path mapping deprecated `v1.x` traits to the new traits will be provided via ADR.
