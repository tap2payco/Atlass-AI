# Model Runtime

> **Version:** 1.0.0  
> **Status:** 🚧 Draft  
> **Last Updated:** 2026-07-27  
> **Owner:** Data Plane Team  
> **Upstream:** [Cognitive Architecture](cognitive-architecture.md)

---

## Objective

To define how Atlas selects, routes, and falls back across inference providers without any upstream runtime ever naming a model or provider directly. The Model Runtime is the single point of contact for all LLM and embedding inference in Atlas.

---

## Cognitive Contract

| Section | Specification |
|---|---|
| **Objective** | Route inference requests by capability, not by provider name. |
| **Inputs** | `ModelRequest { capability, profile, policy, prompt, ... }` |
| **Outputs** | `ModelResponse { content, metadata: ExecutionMetadata }` |
| **Dependencies** | Ollama (local), Fireworks AI API (optional external fallback). |
| **Failure Modes** | All providers fail, circuit breakers open, API key invalid, no local model installed. |
| **Recovery** | Circuit breaker half-open probe; external fallback if policy allows. |
| **KPIs** | Cache hit rate, fallback frequency, P95 latency per capability. |
| **Extension Points** | Add providers by implementing `ModelProvider` trait; add capabilities by updating `ProfileRegistry`. |

---

## Design Principle

Atlas does not say:

```
Call Ollama with deepseek-r1:7b
```

Atlas says:

```
Need: Reasoning
Profile: Balanced
```

The Model Runtime decides everything else.

---

## Routing Architecture

```
ModelRuntime::execute_inference(ModelRequest)
        │
        ▼
InferenceCache ──hit──► return (cache_hit = true)
        │ miss
        ▼
ProviderRegistry.resolve_ordered(capability, profile, policy)
        │   ordered: [Local providers first, External last]
        ▼
Router ──► CircuitBreaker per provider
        │
        ├── OllamaProvider.infer()  ──ok──► cache → return
        │         ↓ fail
        └── FireworksProvider.infer() ──ok──► cache → return
                      ↓ fail
                 Err(AllProvidersFailed)
```

---

## Capability × Provider Ladder

| Capability | Local Primary | Cloud Fallback |
|---|---|---|
| `Reasoning` | `deepseek-r1:7b` (Ollama) | `deepseek-r1` (Fireworks) |
| `CodeGeneration` | `qwen2.5-coder:7b` (Ollama) | `qwen2p5-coder-32b` (Fireworks) |
| `CodeAnalysis` | `qwen2.5-coder:7b` (Ollama) | `qwen2p5-coder-32b` (Fireworks) |
| `Summarization` | `deepseek-r1:7b` (Ollama) | `deepseek-r1` (Fireworks) |
| `Extraction` | `qwen2.5-coder:3b` (Ollama) | `llama-v3p1-8b` (Fireworks) |
| `Embedding` | `bge-m3` (Ollama) | **None — local only** |
| `Classification` | `qwen2.5-coder:3b` (Ollama) | `llama-v3p1-8b` (Fireworks) |

---

## Capability Profiles

Each capability supports multiple quality tiers. The caller specifies `CapabilityProfile` in `ModelRequest`.

| Profile | Intent |
|---|---|
| `Fast` | Lowest latency; smallest qualifying model |
| `Balanced` | Default balanced cost/quality |
| `HighQuality` | Best answer; may use a larger model or external proactively |
| `OfflineOnly` | Per-request privacy: never use external |
| `LowMemory` | Prefer the model with the lowest VRAM footprint |

---

## Execution Policies

The caller specifies `ExecutionPolicy` to control routing behaviour.

| Policy | Behaviour |
|---|---|
| `PreferLocal` | Local first; external fallback allowed (default) |
| `RequireLocal` | Fail if no local provider succeeds |
| `AllowFallback` | Skip to external if local is even degraded |
| `LowLatency` | Fastest path; may skip quality checks |
| `LowCost` | Smallest model satisfying the capability |
| `HighReliability` | Retry aggressively; extended timeout |

---

## Circuit Breaker

Each provider has an independent circuit breaker with three states:

```
Closed ──(3 consecutive failures)──► Open
  ▲                                     │
  │                             (30s recovery window)
  │                                     │
  └──(probe succeeds)──── HalfOpen ◄───┘
                │
        (probe fails)
                │
              Open (timer reset)
```

Thresholds are configurable via `ModelRuntimeConfig`:

| Parameter | Default | Env var |
|---|---|---|
| Failure threshold | 3 | `ATLAS_MODEL_CB_THRESHOLD` |
| Recovery window | 30s | `ATLAS_MODEL_CB_RECOVERY` |

---

## Inference Cache

Prompt responses are cached using a fast xxHash of deterministic request fields.

**Cache key includes:** `capability`, `profile`, `prompt`, `system_prompt`, `max_tokens`, `temperature`  
**Cache key excludes:** `request_id`, `trace_id`, `stream`

A cache hit sets `ExecutionMetadata.cache_hit = true` and skips all provider calls entirely.

| Parameter | Default | Env var |
|---|---|---|
| Max entries | 256 | `ATLAS_MODEL_CACHE_MAX` |
| TTL | 300s | `ATLAS_MODEL_CACHE_TTL` |

---

## Privacy Mode

```yaml
privacy_mode: true
```

When enabled globally in `ModelRuntimeConfig`, all `External` tier providers are removed from the routing pool before any request. No inference data ever leaves the machine.

Per-request privacy is available via `ExecutionPolicy::RequireLocal` or `CapabilityProfile::OfflineOnly`.

---

## Streaming Contract

Callers that need streaming output call `execute_stream` instead of `execute_inference`:

```rust
let (tx, mut rx) = tokio::sync::mpsc::channel(64);
runtime.execute_stream(request, tx).await?;

while let Some(chunk) = rx.recv().await {
    print!("{}", chunk);
}
```

The `ModelProvider::stream()` method is a required part of the trait — every adapter must implement it.

---

## Telemetry Events

Emitted via `observability::EventRecorder` with stable event names:

| Event name | When |
|---|---|
| `model.inference.started` | Before calling any provider |
| `model.inference.completed` | After a successful response |
| `model.inference.failed` | After a provider error |
| `model.provider.fallback` | When switching to the next provider |
| `model.circuit.opened` | When a circuit breaker trips open |
| `model.circuit.recovered` | When a breaker closes after successful probe |
| `model.cache.hit` | When a cached response is returned |
| `model.embedding.completed` | After a successful embedding call |

---

## Local Model Discovery

At startup, `LocalModelRegistry::discover()` queries `GET /api/tags` on the Ollama instance and maps discovered models to capabilities by name pattern. No model names are hardcoded into the routing logic.

If Ollama is unreachable, the local registry is empty. The router then falls through to Fireworks (if available and policy allows), or returns `AllProvidersFailed`.

---

## Future Evolution

- `runtime-embedding` — when embedding demand grows, `ModelCapability::Embedding` can be extracted into a dedicated crate without changing any caller code (the `ModelRuntime` trait surface stays stable).
- `ToolCalling` — `ModelProvider::tool_call()` is already on the trait; providers that support function calling implement it without modifying the routing layer.
- Additional providers — implement `ModelProvider`, register in `ModelRuntime::build()`. Zero changes to callers.

---

## Traceability

- Serves the `Thinking Loop` and `Execution Loop` defined in [Cognitive Architecture](cognitive-architecture.md).
- Referenced by [Reasoning Engine](reasoning-engine.md) (`DeepSeek-R1 inference endpoint`).
- `InferencePort` extension point defined in [Cognitive Architecture §Extension Points](cognitive-architecture.md).
