# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users
Two audiences share one interface:
- **Developers** running Atlas on their own machine against their own repositories. They want to see what the agent did: each tool call, its arguments and output, the session, and context details.
- **People who don't use a terminal**, who ask questions about a project and approve changes in plain language.

The default view stays plain. Technical detail is available on demand rather than shown up front.

## Product Purpose
Atlas is a local-first autonomous software-engineering agent. You link a project folder, ask questions or request changes, and the agent reads, searches and, with your approval, edits files and runs commands. Success means the user can trust what the agent did, because every action is visible and every change was approved.

## Positioning
Atlas runs locally by default. A local Ollama model is used first, and Fireworks is only a fallback that can be disabled with privacy mode. Conversations are saved as append-only sessions on disk and can be resumed. Every write or command goes through an explicit approval step.

## Operating Context
- The web chat is served by `atlas-cli server`. Its default port is 8080.
- The page streams agent protocol events from `POST /api/agent` as NDJSON:
  - session configured;
  - tool call begin and end;
  - tool declined;
  - approval request;
  - context compacted;
  - agent message;
  - turn complete;
  - error.
- The same agent is also available from the terminal (`atlas agent`) and over stdio (`atlas agent --serve-stdio`).
- Local models can be slow: a turn may take a minute or more on a CPU.

## Stack
Plain HTML, CSS and JavaScript with no build step, served directly by `atlas-cli server` from `apps/atlas-dashboard/`.

## Capabilities and Constraints
- The agent reads and searches freely. File edits and commands need the user's approval for each call. A declined call is reported back to the model.
- Sessions persist per workspace and can be resumed by id or as `last`.
- Context compaction summarises older history when the conversation nears the model's context window.
- **Open decision:** whether a model can be picked per chat. The server currently routes by capability, and `ATLAS_OLLAMA_MODEL` overrides that.

## Evidence on Hand
No testimonials, metrics or customers exist. None should be invented.

## Product Principles
1. **Show the work.** Every tool call and its result is visible and inspectable.
2. **Nothing changes without consent.** Writes and commands are approved one call at a time.
3. **Honest state.** Show real progress and real errors, never simulated phases or canned answers.
4. **Plain first, detail on demand.**
5. **Local and private by default.**
