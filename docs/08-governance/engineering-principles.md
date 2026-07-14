# Engineering Principles

> **Version:** 1.0.0  
> **Status:** ✅ Complete  
> **Last Updated:** 2026-07-14  
> **Owner:** Architecture Team  
> **Authority:** These principles govern every engineering decision in the Atlas project. No exception is made without an ADR documenting the rationale.

---

## Purpose

Engineering principles are the **non-negotiable beliefs** that shape every technical decision, code review, architecture choice, and trade-off resolution in Atlas. They are not guidelines — they are constraints.

When two paths are available and both seem reasonable, these principles determine which path to take.

---

## The Principles

### EP-01: Local-First Is Non-Negotiable

**Statement:** Every feature must work without an internet connection. No user data, code, or metadata leaves the local machine for core functionality.

**Why:** Atlas exists because cloud-dependent tools create privacy, sovereignty, and reliability risks. If Atlas requires the cloud, it has no reason to exist.

**Implications:**
- All AI inference runs locally via Ollama or equivalent
- All storage is local (PostgreSQL/SQLite, Qdrant, filesystem)
- All indexing and retrieval happens locally
- Cloud features (if added later) are always optional enhancements, never requirements
- Network calls are blocked by default for all agent operations

**Trade-off:** This limits model size and inference speed compared to cloud solutions. We accept this trade-off because privacy and sovereignty are more valuable than marginal speed improvements.

---

### EP-02: Specialization Over Generalization

**Statement:** Each component (agent, service, module) does one thing well. No component should be a Swiss Army knife.

**Why:** Specialized components are easier to test, debug, replace, and improve. A Planner Agent that also writes code is harder to maintain than separate Planner and Coder agents.

**Implications:**
- Agents have formal contracts defining exactly what they consume and produce
- Services have single responsibilities
- Modules have clear, narrow public APIs
- When a component grows beyond its original scope, it is split (see [Decision Framework](decision-framework.md))

**Trade-off:** More components means more coordination overhead. We accept this because the coordination cost is predictable and manageable, while the cost of entangled components compounds over time.

---

### EP-03: Transparency Is Trust

**Statement:** Every AI decision, reasoning step, tool invocation, and state change must be observable, traceable, and explainable.

**Why:** Developers will not trust a tool that makes opaque decisions about their codebase. Trust is built through transparency, not through magic.

**Implications:**
- Every agent action is logged with full context
- Reasoning chains are preserved and displayable
- Tool invocations show inputs and outputs
- Users can inspect memory, see retrieval results, and understand why a decision was made
- The UI prioritizes showing "why" alongside "what"

**Trade-off:** Logging and tracing add overhead. We accept this because trust is a prerequisite for adoption, and the overhead is minimal compared to model inference time.

---

### EP-04: Quality Over Speed

**Statement:** Atlas will never sacrifice correctness, security, or code quality for generation speed. A correct solution delivered in 60 seconds is superior to a buggy solution delivered in 10 seconds.

**Why:** The cost of fixing bad code always exceeds the cost of generating good code. Users who receive low-quality output lose trust permanently.

**Implications:**
- Generated code must pass linting, formatting, and type checking before delivery
- Tests are generated alongside implementation
- Code review agents validate output before presenting to users
- Performance optimization never compromises correctness
- "Fast but wrong" is always rejected in favor of "slower but right"

**Trade-off:** Users may perceive Atlas as slower than simpler tools. We mitigate this through real-time progress feedback and by demonstrating that the extra time produces measurably better results.

---

### EP-05: Fail Safe, Not Fail Fast

**Statement:** When something goes wrong, the system must protect user data and environment. Failures must be contained, recoverable, and transparent.

**Why:** An autonomous system that can modify files and run commands has the potential to cause damage. Users must trust that Atlas cannot corrupt their project.

**Implications:**
- All file changes are reversible (shadow copies, Git-based undo)
- Agent crashes are isolated — one agent crashing never cascades
- Terminal commands run in sandboxed environments
- Checkpointing enables recovery from any failure point
- Destructive operations always require user confirmation
- The system degrades gracefully when resources are constrained

**Trade-off:** Safety mechanisms add complexity and overhead. We accept this because a single incident of data loss would permanently destroy user trust.

---

### EP-06: Memory Is Architecture

**Statement:** Memory (project knowledge, decisions, patterns, history) is a first-class architectural component, not a feature to be added later.

**Why:** Without memory, every interaction starts from zero. Memory is what transforms Atlas from a stateless tool into an intelligent team member that improves over time.

**Implications:**
- Memory has dedicated storage, retrieval, and lifecycle subsystems
- Every agent has defined memory read/write permissions
- Memory updates follow validated policies (not arbitrary writes)
- Memory pruning and relevance scoring are automated
- Memory retrieval is optimized for each agent's context window

**Trade-off:** Memory management adds significant architectural complexity. We accept this because persistent intelligence is Atlas's primary competitive advantage.

---

### EP-07: Extensibility Is Architecture

**Statement:** Plugin support, custom tools, model swapping, and third-party integrations are architectural requirements, not features to be bolted on later.

**Why:** The best platforms are those their users can extend. VS Code won because of extensions. Atlas must learn from this.

**Implications:**
- Every major boundary is a well-defined interface
- Tool registration uses a plugin manifest system
- Models are accessed through adapter interfaces, not direct calls
- Events are published for all significant state changes (enabling plugin hooks)
- The Plugin SDK is maintained as a first-class deliverable

**Trade-off:** Designing for extensibility adds upfront complexity. We accept this because retrofitting extensibility into a monolithic system is exponentially more expensive.

---

### EP-08: Convention Over Configuration

**Statement:** Atlas should work excellently with zero configuration. Sensible defaults must cover 90% of use cases. Configuration exists for the remaining 10%.

**Why:** Configuration is a source of bugs, support burden, and user confusion. Every configuration option is a decision the user shouldn't have to make.

**Implications:**
- All settings have sensible defaults
- Project-type detection is automatic (language, framework, conventions)
- The `.atlas.toml` configuration file is entirely optional
- Zero-configuration setup is the primary user experience
- Configuration options are discoverable but never required

**Trade-off:** Some power users will want more configurability. We address this through the plugin system and detailed configuration documentation, not through excessive default options.

---

### EP-09: Test Everything That Matters

**Statement:** Every behavior that would break user trust if it failed must have automated test coverage. Every agent's core reasoning must be testable in isolation.

**Why:** AI systems are inherently non-deterministic. Without rigorous testing, regressions are invisible until users encounter them.

**Implications:**
- Core modules maintain > 80% code coverage
- Agent reasoning is tested through deterministic simulations
- Integration tests verify agent collaboration
- End-to-end tests verify complete task flows
- Regression tests catch quality degradation across model updates
- Performance benchmarks run in CI

**Trade-off:** Comprehensive testing slows development velocity. We accept this because shipping regressions to users is more expensive than preventing them.

---

### EP-10: Build for the Debugger

**Statement:** Every system must be designed so that when it breaks (and it will), the cause can be identified quickly.

**Why:** Complex systems fail in complex ways. If failures require hours of investigation, development velocity collapses.

**Implications:**
- Structured logging with correlation IDs across all components
- Distributed tracing through OpenTelemetry
- Every agent action includes context (why it was triggered, what it depends on, what it produced)
- Error messages include actionable context, not just stack traces
- The observability system is designed alongside the feature, not after

**Trade-off:** Observability instrumentation adds code complexity. We accept this because the alternative — blind debugging — is orders of magnitude more expensive.

---

## Principle Hierarchy

When principles conflict, this hierarchy determines precedence:

```
EP-05: Fail Safe         ← Safety always wins
    ↓
EP-01: Local-First       ← Privacy is non-negotiable
    ↓
EP-04: Quality Over Speed ← Correctness before performance
    ↓
EP-03: Transparency      ← Trust before convenience
    ↓
EP-06: Memory            ← Intelligence before simplicity
    ↓
EP-02: Specialization    ← Clean design before expedience
    ↓
EP-07: Extensibility     ← Future-proof before quick-ship
    ↓
EP-09: Test Everything   ← Confidence before velocity
    ↓
EP-10: Build for Debugger ← Observability before cleanliness
    ↓
EP-08: Convention Over Config ← Simplicity before flexibility
```

---

## Applying Principles

### In Code Reviews

Every code review should verify:

- [ ] Does this change respect EP-01 (no external network calls)?
- [ ] Does this component do one thing well (EP-02)?
- [ ] Is this action logged and traceable (EP-03)?
- [ ] Is correctness verified, not just functionality (EP-04)?
- [ ] Can this fail safely without data loss (EP-05)?
- [ ] Does this interact with memory correctly (EP-06)?
- [ ] Does this maintain extension points (EP-07)?
- [ ] Does this work without configuration (EP-08)?
- [ ] Is this behavior tested (EP-09)?
- [ ] Can this be debugged when it fails (EP-10)?

### In Architecture Decisions

Every ADR should reference which principles influenced the decision and document any principle tensions that were resolved.

---

*These principles are the foundation of the Atlas project. For architectural constraints, see [Architecture Principles](architecture-principles.md). For decision-making processes, see [Decision Framework](decision-framework.md).*
