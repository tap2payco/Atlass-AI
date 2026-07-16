# Memory Runtime L3 Certification

**Status**: 🔒 FROZEN
**Date**: 2026-07-16
**Version**: 1.0.0

## Certification Summary
The Memory Runtime has officially achieved L3 Certification, establishing it as the stable, generic cognitive memory manager for the Atlas platform. It exposes a single, cohesive public API (`MemoryRuntime`) while encapsulating all storage, lifecycles, and ranking logic internally.

## Public Interface Freeze
The following structures and contracts are now strictly frozen and may only be altered via an explicit Architecture Decision Record (ADR):

### Public Facade
- `MemoryRuntime::store`
- `MemoryRuntime::retrieve`
- `MemoryRuntime::update`
- `MemoryRuntime::archive`
- `MemoryRuntime::delete`
- `MemoryRuntime::snapshot`
- `MemoryRuntime::query`

### Domain Types
- `MemoryRecord`
- `MemoryIdentity`
- `MemoryContent`
- `MemoryMetadata`
- `MemoryClass` (Transient, Persistent, Immutable, Derived, Cached, Reference)
- `MemoryTraits` (supports_search, supports_versioning, supports_decay, supports_archival, supports_reinforcement)
- `MemoryRelationship` (Strictly structural: DerivedFrom, VersionOf, Parent, Child, SnapshotOf, GeneratedFrom, Duplicates, Replaces)

### Core DTOs
- `RetrievalExplanation`
- `MemorySnapshot`
- `ConsolidationDecision`

## Ownership Boundaries
This freeze strictly documents the separation of concerns between Memory and Knowledge to prevent architectural drift.

### Memory Runtime Owns
- **Storage** (via `MemoryRepository` wrapping Capability-driven Adapters)
- **Lifecycle & Forgetting** (Pipelines enforced via `lifecycle.rs`)
- **Consolidation** (Policy-driven movement between states)
- **Retrieval & Ranking** (Composable strategy and scoring pipelines)
- **Reinforcement** (Score mutation upon retrieval)
- **Structural Relationships** (Provenance, lineage, parent/child)

### Knowledge Runtime (Sprint 4) Owns
- **Entity Extraction & Resolution**
- **Ontology & Schema**
- **Semantic & Causal Relationships** (Supports, Contradicts, DependsOn)
- **Inference & Derivations**
- **Knowledge Graphs & Indexing**
- **Conflict Resolution**

Knowledge Runtime consumes read-only `MemorySnapshot`s. It must never directly invoke storage adapters, manipulate `MemoryMetadata` directly, or bypass the `MemoryRuntime` facade.
