# ADR-017: Conversation and Tool-Call Model Contract

> **Status:** ✅ Accepted
> **Date:** 2026-11-05
> **Author:** Architecture Team

## Context
`shared::model::ModelRequest` carried a single `prompt: String` plus an optional system prompt, and `ModelResponse` returned a single `content: String`. That cannot express:

- multi-turn history (`atlas-cli`'s server flattened chat history into one prompt string with `"User:" / "Agent:"` prefixes);
- tool definitions offered to the model;
- tool calls returned by the model, and their results fed back.

Every agent loop studied (Goose, Codex CLI; see `docs/appendix/research/goose-codex-reference-study.md`) is built on exactly those three things. Sprints 5–9 (Planning, Decision, Execution, Workflow and Agent runtimes) cannot be implemented on the old contract. The provider trait also had a separate `tool_call()` path that returned one tool call with no history; nothing used it.

## Decision
Replace the prompt-string contract with a conversation contract in `shared::model`:

- `Message { role: Role, content: Vec<ContentBlock> }`, where `Role` is `User | Assistant` and `ContentBlock` is `Text | ToolCall | ToolResult`.
- `ModelRequest` replaces `prompt` with `messages: Vec<Message>`, keeps `system_prompt` separate (as both reference systems do), and adds `tools: Vec<ToolSpec>`.
- `ModelResponse` keeps `content` (the concatenated assistant text) and adds `tool_calls: Vec<ToolCall>` and `stop_reason: StopReason`. `ModelResponse::to_message()` produces the assistant `Message` to append to history.
- Tool calls carry a caller-visible `id`. Providers that don't return ids (Ollama) get Atlas-generated ones, so results can always be matched to calls.
- The unused `ModelProvider::tool_call()` method and the runtime-internal `ToolDefinition` / `ToolCallResponse` types are removed. Tool use goes through `infer()`.
- `ModelProvider::supports_tool_use()` (default `false`) gates routing: a request with tools only goes to providers that return `true`.
- Responses to requests that include tools are not cached, because tool loops depend on external state.

## Consequences
- Breaking change to `ModelRequest` / `ModelResponse`. All in-repo callers are migrated in the same change.
- `ModelRequest::text(...)` keeps single-prompt calls short.
- The agent runtime (next step) can append `response.to_message()` and `Message::tool_results(...)` to history and loop.
- Streaming (`execute_stream`) still streams text only. Streaming tool calls is out of scope here.
