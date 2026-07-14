# ADR-016: Development Toolchain Independence

> **Status:** ✅ Accepted  
> **Date:** 2026-07-14  
> **Author:** Architecture Team  

## Context
Atlas relies on advanced AI agents to author code, documentation, and perform reasoning. During the development of Atlas itself, tools like Antigravity, Claude, and other proprietary AI assistants are used to accelerate engineering (authoring EKB, generating implementation scaffolding, etc.). However, tying the production runtime of Atlas to these proprietary or external development tools would violate the core requirement of being an independent, self-owned, local-first platform.

## Decision
**Development tools (Claude, Antigravity, Cursor, Codex, etc.) are not runtime dependencies of Atlas.**

## Principles
- Atlas must not depend on proprietary AI services for its core execution.
- Development assistants may be used to accelerate engineering during the build phase.
- The runtime architecture must remain independent of the tools used to build it.
- Every generated artifact must conform to the Engineering Knowledge Base (EKB).

## Consequences
- Atlas can be developed with any IDE or AI assistant.
- The implementation can later be maintained by different tools without architectural changes.
- Vendor lock-in is avoided.
- The distinction between the **Development Ecosystem** (tools used to build Atlas) and the **Runtime Ecosystem** (the dependencies required to run Atlas) is strictly enforced in the architecture.
