# ADR-009: Why Tauri

> **Status:** Accepted  
> **Date:** 2026-07-14  
> **Categories:** Technology, Frontend

---

## Context

Atlas requires a primary user interface. As a local-first application, it needs to run on the desktop (Windows, macOS, Linux) with deep access to the local filesystem, terminal, and system processes.

## Decision

**We will use Tauri as the desktop application framework, combined with React (TypeScript) for the frontend UI.**

## Alternatives Considered

### Alternative 1: Electron

**Pros:** Industry standard (VS Code, Discord), massive ecosystem, guaranteed rendering consistency across platforms.
**Cons:** Bundles a full Chromium instance and Node.js runtime. This results in massive binary sizes (150MB+) and high baseline RAM usage (300MB+), which competes directly with the AI models for local resources.

### Alternative 2: Flutter

**Pros:** Excellent performance, consistent rendering, compiles to native code.
**Cons:** Uses Dart (smaller ecosystem than React/TS). Desktop support is solid but integrating closely with a Rust backend requires complex FFI bridging.

### Alternative 3: Native UI (e.g., Swift/macOS, WPF/Windows)

**Pros:** Best possible performance and OS integration.
**Cons:** Requires maintaining three separate codebases, slowing down feature development significantly.

## Rationale

Tauri is selected because:
1. **Resource Efficiency:** Tauri uses the OS's native webview (WebView2 on Windows, WebKit on macOS, WebKitGTK on Linux). This keeps the binary size small (~10-20MB) and RAM usage drastically lower than Electron. In an AI application, every megabyte of RAM saved is a megabyte that can be used for context windows or model weights.
2. **Rust Native:** Tauri's backend is written in Rust. Because Atlas's core orchestrator is also Rust ([ADR-007](007-why-rust-core.md)), the UI framework and the core system integrate seamlessly without fragile bridges.
3. **Web Ecosystem:** We can use React, TailwindCSS, and the massive NPM ecosystem for building the UI, leveraging abundant frontend talent.
4. **Security:** Tauri has a strong security model, explicitly requiring allow-listing for API access to the underlying OS, aligning with our security requirements.

## Consequences

### Positive
- Tiny footprint, preserving resources for AI models.
- Single unified Rust backend.
- Write-once-run-anywhere UI using standard web technologies.

### Negative
- **Browser Inconsistencies:** Because Tauri uses the OS native webview, CSS/JS behaviors can occasionally differ between Windows (Edge/Chromium) and macOS (Safari/WebKit), requiring cross-browser testing.

## Compliance

| Principle | Compliant? | Notes |
|---|---|---|
| EP-01: Local-First | ✅ | Builds self-contained desktop executables. |

## Related Documents

- [Technology Selection Framework](../08-governance/technology-selection-framework.md)
- [Frontend Architecture](../../04-subsystems/frontend-architecture.md)
