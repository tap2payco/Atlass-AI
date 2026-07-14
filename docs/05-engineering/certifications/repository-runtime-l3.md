# Runtime Certification Record

**Runtime Name:** Repository Runtime  
**Certification Level:** L3 (Robust / Feature Complete)  
**Status:** Certified & Frozen for Downstream Consumption  

## Version Information
- **API Version:** v1.0.0 (FROZEN)
- **Implementation Version:** v1.0.0
- **Schema Version:** v1.0.0
- **Certified Event Versions:** 
  - `RepositoryOpened`: v1
  - `RepositoryProfileUpdated`: v1
  - `RepositoryChanged`: v1
  - `SnapshotCreated`: v1
  - `SnapshotSuperseded`: v1
  - `SymbolAdded`: v1
  - `SymbolRemoved`: v1
  - `SymbolUpdated`: v1

## Engineering Quality Gates
- **Benchmark Suite Version:** v1.0.0 (Discovery Budget, Incremental Parse Latency, Memory Limits established)
- **Conformance Suite Version:** v1.0.0 (Arch Tests passing)
- **Certification Date:** 2026-07-14
- **Reviewer:** Atlas Architecture Board

## Outstanding Limitations
- Memory Snapshot store is currently entirely in-memory and unbounded. A future implementation update (invisible to API consumers) will introduce SQLite paging.
- Incremental parsing capabilities rely entirely on `notify` file watching; remote repository polling via HTTP is not currently supported in v1.0.0.

## Certification Declaration
> **Repository Runtime v1.0 is the authoritative source of repository intelligence for Atlas.**

Downstream runtimes (Context, Memory, Planning) are now approved to integrate against `packages/sdk/src/repository.rs` and the emitted NATS Event stream. No downstream runtime may duplicate repository discovery, parsing, or synchronization logic.
