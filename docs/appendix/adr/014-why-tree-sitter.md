# ADR-014: Why Tree-sitter

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Technology, Architecture

---

## Context

Atlas must deeply understand the source code in the user's repository. Relying purely on raw text and LLM context windows is insufficient and error-prone. We need to parse files into Abstract Syntax Trees (ASTs) to extract classes, functions, imports, and references for accurate semantic search, context gathering, and targeted code modification.

## Decision

**We will use Tree-sitter as the primary parsing engine for repository intelligence.**

## Alternatives Considered

### Alternative 1: Language Server Protocol (LSP)

**Pros:** Incredibly accurate, understands cross-file types and references natively.
**Cons:** Extremely heavy. Requires launching a separate background process for every language in the repository (e.g., `tsserver`, `rust-analyzer`, `pylsp`). LSPs are notoriously resource-hungry and prone to crashing, which would severely degrade the local-first experience.

### Alternative 2: Regex-based parsing / Ctags

**Pros:** Blazing fast, zero dependencies.
**Cons:** Highly inaccurate. Cannot properly handle nested structures, closures, or modern language syntax.

### Alternative 3: Language-specific AST parsers (e.g., Babel for JS, `ast` module for Python)

**Pros:** Perfect accuracy for the target language.
**Cons:** Requires maintaining integration with 20+ different tools written in different languages. A logistical nightmare.

## Rationale

Tree-sitter is selected because:
1. **Incremental Parsing:** It updates the syntax tree instantly as files change, making it fast enough to run on every keystroke/file save without noticeable CPU load.
2. **Ubiquity:** It has grammars for over 200 programming languages, providing a unified C API to query ASTs regardless of the language.
3. **Query Language:** Tree-sitter provides a powerful Lisp-like query language that allows us to write standard patterns to extract functions, classes, and imports across any supported language.
4. **Rust Integration:** Tree-sitter is written in C but has exceptional, first-class Rust bindings, fitting perfectly into our core orchestrator.

## Consequences

### Positive
- Fast, unified parsing of almost any programming language.
- Enables precise code chunking for RAG (e.g., chunking by function rather than arbitrary line counts).
- Low resource footprint compared to LSPs.

### Negative
- **Lack of Deep Semantics:** Tree-sitter only understands syntax (the structure of a single file). It does not understand cross-file types (e.g., it knows `foo.bar()` is a method call, but doesn't know where `bar` is defined). We must build a symbol resolution layer on top of Tree-sitter to handle cross-file references.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | Fully offline, compiled into the application. |
| EP-04: Quality Over Speed | ✅ | Ensures we provide accurate structural context to the LLM. |

## Related Documents

- [Technology Selection Framework](../08-governance/technology-selection-framework.md)
- [Repository Intelligence](../../04-subsystems/repository-intelligence.md)
