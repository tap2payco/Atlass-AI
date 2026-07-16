# Execution Core Specification

> **Status:** Active  
> **Version:** 1.0.0  

## Responsibilities
The `execution-core` package provides pure orchestration and execution mechanics for the Atlas Platform. It is entirely domain-agnostic and does not contain any Atlas-specific knowledge (no repositories, no context packages, no planning logic).

## Ownership
This package owns:
- `PipelineStage<I, O>`: Generic stage definitions.
- `Middleware<I, O>`: Wrapping behavior (logging, timeouts, retries).
- `Executor<T, R>` & `Scheduler<T, R>`: Abstract execution constraints and scheduling policies.
- `ExecutionContext`: Request tracking, trace IDs, correlation IDs, and cancellation tokens.
- `Validator<T>`: Emits rich `ValidationReport` diagnostics.
- `Cache<K, V>`: Implementation-agnostic caching interface.
- `RetryPolicy` & `SchedulingPolicy`: Platform-wide configurations.

## Extension Points
Every runtime in Atlas relies on this core by implementing the generic traits.
- **Middlewares** can be stacked sequentially.
- **Schedulers** decide how concurrent tasks mapped by the Executor run based on `SchedulingPolicy`.
- **Caches** can be backed by SQLite, Redis, or in-memory layers.

## Guarantees
- **Type Safety**: Fully generic traits enforce type-safe pipelines without dynamic dispatch overhead where possible.
- **Cancellation**: Every execution accepts an `ExecutionContext` which enforces timeouts and graceful termination.
- **No Domain Coupling**: It is guaranteed that this crate can be lifted into any non-Atlas Rust project.

## Lifecycle & Failure Model
- The execution core does not boot or shut down. It is a library of mechanics used by the `Runtime Kernel`.
- Failures bubble up via generic `Result` constraints and are trapped/managed by standard `Middleware` (e.g. `RetryMiddleware`).
