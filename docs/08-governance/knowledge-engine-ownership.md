# Knowledge Engine Ownership

**Status**: 🔒 FROZEN
**Date**: 2026-07-16
**Version**: 1.0.0

To ensure the Knowledge Runtime operates as a deterministic, reproducible semantic compiler, engine boundaries are strictly enforced. No engine may bypass its assigned responsibilities to mutate structures it does not own. 

## Engine Ownership Matrix

| Engine | Read | Create | Modify | Delete |
|--------|------|--------|--------|--------|
| **Extraction** | `MemorySnapshot` | `RawEntity` | — | — |
| **Canonicalization** | `RawEntity`, `OntologyRegistry` | `Entity` | `EntityIdentity` (Aliases) | — |
| **Schema Validation** | `Entity`, `OntologyRegistry` | `ValidationResult` | — | Reject `Entity` |
| **Semantic Validation** | `Entity`, `OntologyRegistry` | `ValidationResult` | — | Reject `Entity` |
| **Relationship Builder** | `Entity` | `RelationshipCandidate` | — | — |
| **Relationship Validator**| `RelationshipCandidate`, `OntologyRegistry` | `KnowledgeRelationship` | — | Reject `Candidate` |
| **Conflict Resolution** | `KnowledgeRelationship` | `ConflictRecord` | Canonical Mapping | Duplicate `Entity` |
| **Inference** | `KnowledgeFact` | `KnowledgeInference` | `Confidence` | — |
| **Index** | `KnowledgeSnapshot` | `GraphIndex` | `GraphIndex` | Rebuild Index |

## Execution Contract
1. The **OntologyRegistry** is strictly read-only for all engines during the execution of a `KnowledgePipeline`.
2. Communication between engines is facilitated strictly through the `KnowledgeCompilationContext`. Engines may not perform out-of-band communication.
3. The final output of the pipeline is a completely immutable `KnowledgeSnapshot`. Downstream systems (e.g. Planning, Decision, Agent Runtimes) may only read this snapshot.
