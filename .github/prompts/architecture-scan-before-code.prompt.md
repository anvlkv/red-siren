---
description: "Scan Red Siren architecture before code changes so work starts from the backend command surface and audio runtime docs instead of jumping straight into implementation."
argument-hint: "What area are you planning to change?"
---

# Architecture Scan Before Code Changes

You are working in the Red Siren workspace. Before making or proposing code changes, do a short architecture scan grounded in the current codebase.

## Goals
- Build context before implementation.
- Start from stable system boundaries, not stale feature descriptions.
- Identify the real integration points across Tauri, frontend, shared types, and audio runtime.
- Include a focused BOM slice so proposed changes map to implemented materials, not assumptions.

## Required Reading Order
1. Read [AGENTS.md](../../AGENTS.md).
2. Read [README.md](../../README.md) for build and run commands.
3. Read [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs) to map the backend startup flow, plugins, setup sequence, and Tauri command surface.
4. Read [crates/audio-system/README.md](../../crates/audio-system/README.md) for runtime boundaries, quality gating, and backend split.
5. Read [crates/audio-system/src/system/README.md](../../crates/audio-system/src/system/README.md) before reasoning about DSP graph structure or excitement routing.
6. Read [crates/frontend/src/app.rs](../../crates/frontend/src/app.rs) and [crates/frontend/src/routes.rs](../../crates/frontend/src/routes.rs) to understand the frontend shell, theme setup, and route entry points.
7. Read [docs/feature-bom.md](../../docs/feature-bom.md) to ground the scan in the current implemented feature inventory.
8. If the requested area is specific, then read only the most relevant follow-up files for that subsystem.

## What To Produce First
Before editing code, provide:
1. A brief architecture map of the relevant subsystem.
2. The likely files or crates involved, with a short reason for each.
3. Any shared command, event, config, or runtime boundaries that the change will cross.
4. Any mismatch between the user's description and the current code/docs.
5. The focused skill or skills that should be used next, if any.
6. A scoped BOM excerpt for the requested area (implemented materials only).

## Scope Rules
- Keep the scan short and high-signal.
- Trust current code and repo docs over chat descriptions when they differ.
- Do not restate large README sections; link and summarize.
- Do not start implementation until the architecture scan is complete.
- If the request is purely local and the scan shows that clearly, say so and keep the follow-up narrow.
- BOM entries must be concrete and traceable to existing files/symbols; avoid speculative roadmap items.

## Output Format
Use this structure:

### Architecture Map
- ...

### Likely Touch Points
- ...

### Risks Or Unknowns
- ...

### BOM Slice (Implemented)
- Material: ...
- Layer: ...
- Source: ...
- Why it matters for this change: ...

### Next Step
- ...

## Useful Workspace References
- [AGENTS.md](../../AGENTS.md)
- [README.md](../../README.md)
- [src-tauri/src/lib.rs](../../src-tauri/src/lib.rs)
- [crates/audio-system/README.md](../../crates/audio-system/README.md)
- [crates/audio-system/src/system/README.md](../../crates/audio-system/src/system/README.md)
- [crates/frontend/src/app.rs](../../crates/frontend/src/app.rs)
- [crates/frontend/src/routes.rs](../../crates/frontend/src/routes.rs)
- [docs/feature-bom.md](../../docs/feature-bom.md)