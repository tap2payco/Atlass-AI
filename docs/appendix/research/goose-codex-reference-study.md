# Reference Study: Goose and Codex CLI → Atlas Agent Runtime

> **Status:** Research note (input to a future ADR — not an architecture change)
> **Date:** 2026-11-05
> **Sources:** `aaif-goose/goose` (formerly `block/goose`) @ `bfe8ccd` (2026-11-04), `openai/codex` @ `388aee4` (2026-11-05). Both Apache-2.0.
> **Method:** Source reading of shallow clones (§3–5), plus REA static inspection of the official Windows x64 release binaries (§7). Nothing was executed.

---

## 1. Why these two

Both are production Rust agent systems that already do what Atlas Sprints 5–9 describe
(planning, execution, workflow, agent runtimes). Goose is the closest philosophical match
(local-first, MCP-native, llama.cpp backend). Codex is the strongest reference for
**safe execution** (sandboxing, approval policy, command rules).

## 2. Atlas starting point (observed in this repo)

| Area | Atlas today | Gap |
|---|---|---|
| Model contract | `ModelRequest { prompt: String, system_prompt, … }` (`packages/shared/src/model.rs:88`) | No message history, no tool definitions, no tool-call output. **An agent loop cannot be built on this contract.** |
| Agent | `AgentRuntime::invoke(agent, task, context: String) -> String` (`packages/sdk/src/agent.rs`) | String in / string out; no turn, event, or cancellation model. |
| Tools | `ToolRuntime::execute(tool_id, args: &str) -> String` (`packages/sdk/src/tool.rs`) | No schema type, approval, sandbox, or parallelism metadata. |
| Pipeline | `SimplePipeline` — linear `T → T` stages (`packages/execution-core/src/pipeline.rs`) | No looping, yielding, or cancellation. |
| Capability registry | `resolve()` returns first provider; priority sort is a comment (`packages/capability-registry/src/registry.rs`) | — |
| Bus | Trait only (`packages/runtime-bus`) | No implementation. |
| Context providers | git / docs / issue / knowledge-graph / memory / plugin are `unimplemented!()` | Memory and knowledge runtimes exist but aren't wired in. |
| Planning … Agent runtimes (Sprints 5–9) | Marked L3 on `ENGINEERING_BOARD.md` | **No packages exist.** |

## 3. Goose — what to take

**Core:** `crates/goose-agent/src/machine.rs` (~355 lines) — a generic `StateMachine<S, E>`.

How it works:

1. The machine holds an **ordered list of `Step`s**: many `Operation`s, then one `Inference` step last.
2. `step()` walks the list; the **first operation that returns `Applied`** produces a `StepResult { effects, status }`. Others return `NotApplicable`.
3. Effects are applied by an `EffectHandler` (persist messages, emit events); the session is **reloaded from storage**; repeat while `status == Continuing`, stop on `Yielded` / `Cancelled`.
4. Every operation can contribute **tools** (`inference_tools`) and **prompt parts** (`prompt_parts`) to the next model request. Duplicate tool names across operations are a hard error.
5. All state — e.g. whether a tool call is `Unchecked / Executable / Blocked / AwaitingUser / Responded` (`tool_call_state.rs`) — is **derived from the persisted conversation**, not held in memory. Operations may leave small "operation notes" in message metadata. Result: any turn can be resumed or replayed after a crash.
6. Cancellation uses a `CancellationToken`; `finalize()` synthesizes "Tool call was interrupted" responses for open calls so the transcript stays valid.

Operation order in production (`crates/goose/src/agents/state_machine_builder.rs`):
ToolApproval → PreToolHook → Skills → Recipe → **ToolExecution** → RemainingToolCalls → UnhandledToolCalls → Subagent (bounded concurrency) → Steer → BangShell → ContextRelevance → **Compaction** → ToolPairCompaction → Doctor → Project → Retry → EmptyResponse → StopHook → ExitOnError → MaxTurns → *Inference*.

Other pieces worth reading:

- `ToolApproval` trait → `Allow | Deny{reason} | Ask{prompt}` (`goose-agent/src/tool_approval.rs`); permission store + read-only-annotation inspector in `goose/src/permission/`.
- Compaction: auto-compact at a threshold of the context limit (default cap 225k tokens), plus **tool-pair summarization** that summarizes old tool request/response pairs in batches of 10 (`goose/src/context_mgmt/mod.rs`).
- `goose-local-inference`: llama.cpp (`llama-cpp-2`, CUDA/Vulkan features) + MLX, with **tool-call emulation and parsing** for local models that lack native tool calling (`tool_emulation.rs`, `native_tool_parsing.rs`). Directly relevant to Atlas's local-first goal.

## 4. Codex — what to take

**Core:** `codex-rs/core/src/session/turn.rs::run_turn` and `session/handlers.rs::submission_loop`.

- **Submission/Event protocol** (`codex-rs/protocol/src/protocol.rs`): clients send `Op`s (`TurnInput`, `Interrupt`, `RecoverTurn`, …) and receive `EventMsg`s. The agent core is UI-agnostic; the CLI, TUI and app server are all clients of the same protocol. This is the clean seam Atlas's `runtime-bus` is reaching for.
- **Turn loop:** pre-sampling compaction → sample → if the model emitted tool calls (`needs_follow_up`) or input is pending, loop; if the token limit is reached mid-turn, auto-compact and continue.
- **Approval policy** (`AskForApproval`): `UnlessTrusted | OnRequest (default) | Granular{sandbox, rules, skill, request_permissions, mcp_elicitations} | Never`.
- **Sandbox policy** (`SandboxPolicy`): `ReadOnly | WorkspaceWrite{writable_roots, network_access} | ExternalSandbox | DangerFullAccess`. `WritableRoot` keeps `.git` (especially `.git/hooks`) and the agent's own config **read-only even inside a writable root**, which blocks privilege escalation through hooks.
- **Tool runtime traits** (`core/src/tools/sandboxing.rs`): every tool implements `Approvable` (what approval it needs) + `Sandboxable` (sandbox preference, `escalate_on_failure`) + `ToolRuntime::run(req, attempt, ctx)`. `ToolOrchestrator` runs approval → sandboxed attempt → on sandbox denial, ask to retry unsandboxed (with fresh approval).
- **Exec policy rules** (`codex-rs/execpolicy`): per-command `Allow | Prompt | PromptUser | Forbidden`, parsed from a rules file.
- **Parallel tool calls** (`core/src/tools/parallel.rs`): one `RwLock<()>`. Tools marked parallel-safe take a read guard; everything else takes the write guard. Concurrent reads, exclusive writes, at almost no cost.
- Platform sandboxes: `linux-sandbox` (Landlock/seccomp, `bwrap`), macOS Seatbelt, `windows-sandbox-rs`. Atlas devs are on Windows, so note that crate.
- **Rollouts** (`codex-rs/rollout`): an append-only session log. Same replay property as Goose, different mechanism.
- Local models: `ollama` and `lmstudio` provider crates.

## 5. Recommended mapping onto Atlas

Order matters: step 1 unblocks everything else.

1. **Replace the prompt-string model contract.** Add `Message { role, content: Vec<ContentBlock> }` with `Text | ToolRequest{id, name, args: Value} | ToolResponse{id, result}`, plus `tools: Vec<ToolSpec{name, description, input_schema}>` on `ModelRequest`, and tool calls on `ModelResponse`. Port Goose's tool-call emulation for local models (DeepSeek-R1 / Qwen3-Coder via llama.cpp or Ollama).
   *Status 2026-11-05: done ([ADR-017](../adr/017-conversation-and-tool-call-model-contract.md)), except tool-call emulation.*
2. **New `runtime-agent` package = Goose-style operation state machine.** Each Atlas "cognitive state" in `docs/09-ai/cognitive-state-machine.md` (Understanding, Planning, Execution, Review, Reflection…) becomes an `Operation` that applies when its precondition holds in the conversation. Model routing per state stays with `runtime-model`'s capability routing. Keep state derived from the persisted conversation (use `runtime-memory`'s store) so turns are resumable.
   *Status 2026-11-05: core done in `packages/runtime-agent`: state machine, tool execution, max turns, inference, cancellation and resume, with an in-memory `SessionStore`. Not done yet: cognitive-state operations, a `runtime-memory`-backed store, and wiring into `atlas-cli`.*
   *Status 2026-11-06: wired into `atlas-cli`. Sessions are saved by `FileSessionStore`: one append-only JSON-lines file per session, in Codex's rollout style rather than `runtime-memory`, whose records are not a conversation log. The files live in `%LOCALAPPDATA%\atlas\sessions` (or `$ATLAS_HOME/sessions`) and are tagged with their workspace. Resume with `atlas agent --resume last|<id>` or `sessions` / `resume` in the REPL. A cut-off last line from a crash is skipped on load.*
3. **Upgrade `ToolRuntime` to Codex's shape:** `ToolSpec` + `Approvable` + `Sandboxable` + `parallel_safe: bool`, executed by an orchestrator in `execution-core` with the RwLock parallelism trick. Fold `capability-registry` in as the tool/provider lookup (and implement the priority sort).
   *Status 2026-11-06: tool runtime done in `runtime-agent`, not `execution-core`. Tools declare a `ToolEffect` (`ReadOnly` / `WritesWorkspace` / `RunsProcesses`; the default is the most powerful). Consecutive read-only calls run in parallel and everything else runs alone, in call order, with a per-call timeout and an output cap. `atlas-cli` adds `write_file`, `edit_file` (refuses `..` and `.git`) and `run_command` behind `--access read|write|full`. Not done: `Approvable`/`Sandboxable` (step 4), the `capability-registry` fold, and killing a command's child processes on timeout.*
4. **Adopt Codex's policy enums almost verbatim** (`ApprovalPolicy`, `SandboxPolicy`, protected `.git`/config subpaths) in `platform-contracts`. Start with `WorkspaceWrite` + `OnRequest` as defaults; actual OS sandboxing can come later behind the `Sandboxable` trait.
   *Status 2026-11-06, first part done:*
   - *`runtime-agent` has a `ToolApprover` hook. Every non-read-only call is put to it, and a denied call returns the user's reason to the model.*
   - *`atlas-cli` asks `[y]es/[n]o/[a]lways` by default (`--approve ask|auto`, or `approve` in the REPL) and denies when there is no terminal.*
   - *`run_command` runs in a kill-on-close Windows Job Object, so a timeout, a cancel or the command finishing ends its whole process tree. It strips secret-looking environment variables (`*_KEY`, `*_TOKEN`, `*_SECRET`, `*_PASSWORD`, `*_CREDENTIALS`; keep specific ones with `ATLAS_COMMAND_KEEP_ENV`).*
   - *Not done: restricted tokens and workspace ACLs, network blocking (WFP), process-group kill on Unix, and the policy enums in `platform-contracts`.*
5. **Make `runtime-bus` carry an Op/Event protocol** (`TurnInput`, `Interrupt`, `ApprovalResponse` / `MessageDelta`, `ToolCallBegin/End`, `ApprovalRequest`, `TurnComplete`). `atlas-cli` and `atlas-dashboard` become clients of it.
6. **Compaction** as an operation (threshold + tool-pair summarization), backed by `runtime-context` budgeting.
   *Status 2026-11-06, first part done:*
   - *`CompactionOperation` runs when a new request arrives and the estimated request (about 4 characters per token) is over 75% of the context window. It summarises everything before that request and appends a `ContentBlock::Summary` covering it. The model then sees the latest summary plus the newer messages (`model_view`); summaries chain, and stored history is never rewritten.*
   - *A failed summary call does not block the turn and is not retried within that request.*
   - *`atlas-cli` enables it with the same window it now requests from Ollama: `ATLAS_OLLAMA_NUM_CTX`, default 8192, where previously Ollama used its own small default and silently dropped old context. `ATLAS_CONTEXT_TOKENS` overrides the window.*
   - *Inside a long tool loop, `fit_tool_outputs` keeps each request within the window: older tool outputs are replaced by a short note, oldest first, and if that is not enough the latest outputs are cut, keeping their start and end. Calls and text are kept, and only what is sent changes. The summary request is fitted the same way.*
   - *Not done: real token counting and `runtime-context` budgeting.*
7. Then wire the stubbed context providers (memory, knowledge graph, git) as operations contributing `prompt_parts`.

The board's Sprint 5–9 rows should be reset to **L0** until these packages exist.

## 6. Unknowns / not verified

- Neither system was built or run. §7 is static inspection of shipped binaries only.
- Goose's `Inference` implementation (`goose-agent/src/inference.rs`) and Codex's `run_sampling_request` internals were not read in depth.
- Both projects move fast (daily commits); file paths above are valid at the stated commits only.

## 7. Shipped-binary inspection (REA 6.8.0)

Static, file-format-level reads of the official release builds. No disassembly, no execution.

| | Goose | Codex |
|---|---|---|
| Release / file | v1.57.0 `goose.exe` (from `goose-x86_64-pc-windows-msvc.zip`) | rust-v0.166.0 `codex-x86_64-pc-windows-msvc.exe` |
| Size / SHA-256 | 171 MB / `ee39aee3…1145242` | 372 MB / `9d563ee4…8ca4eb42` |
| Authenticode | **None.** No security directory (`ev_57d2a238…`) | Signed: **OpenAI OpCo, LLC** via Microsoft ID Verified CS EOC CA 03 (Microsoft's managed signing service), 3-day leaf cert valid 2026-10-27…10-30 (`ev_7924591f…`). Chain not cryptographically verified. |
| cargo-auditable dep list | Absent (`ev_6961921b…`) | Not read. The file exceeds REA's 256 MiB limit for this check. |
| C runtime | Dynamic MSVC CRT (`MSVCP140`, `VCRUNTIME140`): C++ is linked in. Strings `ggml_backend`, `gguf`, `llama_model_load` are present, consistent with llama.cpp being built into the CPU build. | Static CRT, no MSVCP |
| CFG | `IMAGE_GUARD_CF_INSTRUMENTED` (inferred from linker flag) | same |

**The main finding is the Windows sandbox, read from the import tables** (Goose: 33 DLLs / 603 functions; Codex: 25 DLLs / 492 functions):

- **Goose has no Windows OS sandbox.** For process control it imports only job objects (`CreateJobObjectW`, `AssignProcessToJobObject`, `TerminateJobObject`), which is enough to kill a whole process tree. There are no restricted tokens, ACL edits or firewall APIs. Its safety on Windows is the approval layer only. Secrets go to Windows Credential Manager (`CredReadW` / `CredWriteW`).
- **Codex ships a real Windows sandbox**, matching `codex-rs/windows-sandbox-rs`:
  - `CreateRestrictedToken`, `SetTokenInformation`, `CreateProcessAsUserW`: commands run under a reduced-privilege token.
  - `DeriveCapabilitySidsFromName`: capability SIDs (AppContainer-style identity).
  - `SetEntriesInAclW`, `SetNamedSecurityInfoW`: ACL grants on the workspace directories (the `WorkspaceWrite` policy enforced in the OS).
  - Job objects plus a pseudoconsole (`ResizePseudoConsole`) for a contained interactive shell.
  - `fwpuclnt.dll` (Windows Filtering Platform) for network blocking. The main exe imports only engine open, transactions and **delete** calls. `FwpmFilterAdd0` (`windows-sandbox-rs/src/wfp.rs:332`) and `NetUserAdd` (`setup_provisioning/sandbox_users.rs:131`) are **not** imported by `codex.exe`. This means firewall-rule and sandbox-user creation run in the separate signed `codex-windows-sandbox-setup` / `-service` executables. That is an inference from import absence plus source.
  - `tdh.dll` / `OpenTraceW` / `ProcessTrace`: it consumes ETW events, probably to observe sandboxed processes. That purpose is an inference.

**Implications for Atlas:**
1. The `Sandboxable` trait (§5 step 4) is not theoretical on Windows. Codex shows a workable recipe: restricted token, plus workspace ACLs, plus a job object, plus WFP filters.
2. **Split privileges the way Codex does.** Keep the main `atlas` binary unprivileged. Put one-time elevated setup (sandbox user, firewall provider) in a separate small, signed helper.
3. **Start where Goose is:** job objects for reliable process-tree kill, plus approval gating, plus Credential Manager for provider keys. That is cheap and gives a credible first milestone before the full sandbox.
4. Sign release binaries. Goose ships unsigned on Windows, which triggers SmartScreen warnings; Codex uses short-lived managed-signing certs.

**Not covered:** no decompilation (Ghidra/IDA not installed); no Codex dependency list (size limit); the sandbox helper executables were not downloaded; import presence does not show that a code path runs.
