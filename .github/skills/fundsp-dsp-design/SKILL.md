---
name: fundsp-dsp-design
description: 'Design or review FunDSP graphs, real-time control paths, and DSP structure for an instrument or audio effect in this Rust workspace.'
---

# FunDSP DSP Design

## When to Use
- Creating or refactoring synthesis, filtering, effects, or modulation graphs.
- Deciding between static and dynamic FunDSP graph construction.
- Adding real-time parameter control or performance-sensitive DSP changes.

## Procedure
1. Define the DSP goal first: generator, filter, effect chain, modulation source, mixer, or analysis node. Pick the smallest graph structure that satisfies the requirement.
2. Choose the right graph model. Use static `AudioNode`-style composition when connectivity is fixed and performance matters most; use dynamic `AudioUnit` or `Net` patterns when routing or node composition must change at runtime.
3. Express the signal path clearly with FunDSP operators such as pipe, bus, branch, and stack. Favor readable graph structure over clever but opaque expressions.
4. Set and respect sample-rate assumptions. If behavior depends on sample rate, make that explicit in the design and review how the graph will behave under different output configurations.
5. For real-time control, use the control mechanisms FunDSP provides rather than rebuilding graphs unnecessarily: shared variables, settings listeners, smoothing filters like `follow`, and frontend/backend splits for dynamic graphs when needed.
6. Prefer block processing or backend-oriented designs when the graph becomes dynamic or expensive enough that single-sample control flow would add avoidable overhead.
7. Pick the prelude intentionally. Use the 32-bit environment when throughput is the priority and the 64-bit environment when internal precision matters more than speed.
8. Finish by checking both soundness and operability: parameter ranges, clipping risk, latency implications, smoothing for live controls, and whether the graph can be updated without clicks.

## Completion Checks
- The graph model matches the mutability and performance needs.
- Control updates avoid audible zipper noise or rebuild churn.
- Sample-rate behavior is explicit.
- DSP structure is readable enough to evolve safely.
- Latency, clipping, and update artifacts have been considered.

## References
- [FunDSP crate docs](https://docs.rs/fundsp/latest/fundsp/)
- [Cargo.toml](../../../Cargo.toml)
- [crates/audio-system/Cargo.toml](../../../crates/audio-system/Cargo.toml)