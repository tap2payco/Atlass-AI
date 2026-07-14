# Runtime Compatibility Matrix

This matrix tracks the dependency graph and minimum required API versions across the Atlas runtime ecosystem. Before implementing or upgrading a runtime, verify compatibility here.

| Runtime          | Depends On                    | Minimum API Version | Note                                                                                   |
| ---------------- | ----------------------------- | ------------------- | -------------------------------------------------------------------------------------- |
| Repository Runtime| None                          | N/A                 | Foundational Intelligence Service                                                      |
| Context Runtime  | Repository Runtime            | 1.0.0               | Subscribes to `SnapshotCreated` (v1) and queries `get_snapshot`                        |
| Memory Runtime   | Repository Runtime            | 1.0.0               | Anchors vector embeddings to stable `SymbolEntity` IDs                                   |
| Planning Runtime | Context Runtime               | 1.0.0               | Constructs execution graphs from resolved Context                                      |
| Agent Runtime    | Repository + Context + Memory | 1.0.0               | Orchestrates actions using the fully assembled Control Plane                          |

*Matrix is maintained by the Atlas Architecture Board. Do not modify dependencies without an approved ADR.*
