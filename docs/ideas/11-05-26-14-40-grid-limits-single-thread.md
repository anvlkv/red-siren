# Grid Limits Ideation: Single-Thread, 10 ms Audibility Constraint

## Problem framing

Inputs available in GridLimits::new:
- num_threads: usize
- cpu_frequency_mhz: u64
- sample_rate_hz: u32

User constraints for this ideation:
- Ignore num_threads for this computation.
- Tick means DSP sample frame chunk.
- Hard constraint is only MIN_AUDIBLE_DURATION_MS = 10.0.
- Output requested: formulas and algorithms only.

Outputs to compute:
- max_bpm
- min_ticks_per_phase

Unknowns that must be explicit in any formula:
- ticks_per_second relation to sample_rate_hz depends on chunk size in samples.
- phases_per_beat must be defined by musical model (or fixed to 1 as a policy).

---

## Codebase inspiration

From crates/common/src/config/grid_limits.rs:
- GridLimits has exactly two outputs: max_bpm and min_ticks_per_phase.
- MIN_AUDIBLE_DURATION_MS is currently the only in-file hard timing constant.

From crates/common/src/config/context.rs:
- Context provides sample_rate_hz and cpu_frequency_mhz alongside num_threads.
- This suggests GridLimits should be derivable from machine + audio context without dynamic probes.

Implication:
- A deterministic constructor can be purely formula-based and stable across sessions.

---

## Web inspiration

Relevant references:
- Sampling theory: sample rate is discrete-time basis for time resolution and scheduling assumptions.
  - https://en.wikipedia.org/wiki/Sampling_(signal_processing)
- Audio rendering chunk behavior in practice: WebAudio commonly processes fixed-size quanta (often 128 frames), reinforcing chunk-aware timing formulas.
  - https://developer.mozilla.org/en-US/docs/Web/API/AudioWorkletProcessor/process
- Timing jitter concept: practical schedulers should reserve margin against timing variance.
  - https://en.wikipedia.org/wiki/Jitter

Takeaway:
- Use sample-rate-derived timing as the backbone.
- Prefer formulas that can include a chunk-size term and optional jitter/headroom factors.

---

## Idea options

## Option 1: Pure audibility-bound formulas (minimal model)

Concept:
- Compute limits only from 10 ms audibility and sample-rate timing resolution.
- Treat CPU frequency as informational, not limiting.

Formulas:
- Let:
  - D_min_ms = 10
  - S = sample_rate_hz
  - C = samples_per_tick (policy constant, e.g. 1 for per-sample tick or 128 for block tick)
  - phi = phases_per_beat (policy constant)
- Tick duration in ms:
  - tick_ms = 1000 * C / S
- Minimum ticks per phase:
  - min_ticks_per_phase = ceil(D_min_ms / tick_ms)
  - equivalent: ceil(D_min_ms * S / (1000 * C))
- Maximum BPM from minimum phase duration:
  - phase_ms = 60000 / (bpm * phi)
  - max_bpm = floor(60000 / (D_min_ms * phi))

Fit:
- Perfectly matches your current explicit hard constraint.

Feasibility:
- Very high. No calibration needed.

Risks:
- max_bpm ignores CPU and may be unrealistically high for complex DSP.
- Sensitive to policy choice for C and phi.

Effort:
- Low.

---

## Option 2: Audibility plus CPU-budget cap (analytic hybrid)

Concept:
- Keep Option 1 as baseline.
- Add a CPU-derived upper cap on tick rate, then map to BPM and ticks/phase.

Formulas:
- Let:
  - F = cpu_frequency_mhz * 1e6 cycles/s
  - u in (0,1) = target utilization budget (example 0.5 to 0.8)
  - k = effective cycles per tick (calibration constant per algorithm complexity)
- CPU-limited tick rate:
  - tick_rate_cpu_max = (F * u) / k
- Audio-geometry tick rate:
  - tick_rate_audio = S / C
- Effective allowable tick rate:
  - tick_rate_eff = min(tick_rate_audio, tick_rate_cpu_max)
- Effective tick duration:
  - tick_ms_eff = 1000 / tick_rate_eff
- Then:
  - min_ticks_per_phase = ceil(D_min_ms / tick_ms_eff)
  - max_bpm = floor(60000 / (D_min_ms * phi)) then optionally reduce by CPU safety factor if phase work scales with tempo.

Fit:
- Better balance of quality and performance.

Feasibility:
- Medium-high if k is measured once and stored as a config constant.

Risks:
- k varies by DSP graph complexity and platform behavior.
- cpu_frequency_mhz is an imperfect proxy for real throughput.

Effort:
- Medium.

---

## Option 3: Empirical auto-tune at startup (measurement-driven)

Concept:
- Measure actual per-tick processing time on target device for representative workload.
- Derive safe tick budget from observed timing percentiles.

Algorithm:
1. Run a short synthetic workload over N blocks.
2. Measure per-tick compute time t_tick_ms and jitter.
3. Select robust bound, for example p95 or p99 of t_tick_ms.
4. Apply headroom factor h (for example 1.2).
5. Enforce:
   - h * t_tick_ms <= tick_ms_eff
6. Solve for safe tick rate and derive:
   - min_ticks_per_phase = ceil(D_min_ms / tick_ms_eff)
   - max_bpm based on audible bound and optional runtime cap from phase computation budget.

Fit:
- Strong for real-world stability when CPU frequency is noisy or misleading.

Feasibility:
- Medium. Needs benchmark harness and deterministic warm-up.

Risks:
- Startup delay.
- Measurement noise across thermal states.

Effort:
- Medium-high.

---

## Option 4: Policy-table + interpolation (product-oriented)

Concept:
- Curate tested tiers by sample rate and coarse CPU bands.
- Interpolate or snap to nearest stable profile.

Algorithm:
- Table keyed by sample_rate_hz and cpu_frequency_mhz bands:
  - profile_i => (max_bpm_i, min_ticks_per_phase_i)
- Optional interpolation:
  - linear in log-frequency domain for smoother transitions.
- Always clamp by 10 ms audibility formula.

Fit:
- Great when you value predictable UX over pure theory.

Feasibility:
- High once you collect baseline test data.

Risks:
- Maintenance cost as DSP complexity evolves.

Effort:
- Medium.

---

## Idea comparison

- Option 1
  - Strength: simplest and deterministic
  - Weakness: no performance awareness
  - Best when: early-stage model clarity matters most

- Option 2
  - Strength: principled quality-performance balance
  - Weakness: depends on choosing a realistic cycles-per-tick constant
  - Best when: you want static formulas with some hardware adaptation

- Option 3
  - Strength: closest to real device behavior
  - Weakness: more engineering complexity
  - Best when: runtime robustness is critical across varied devices

- Option 4
  - Strength: practical and predictable
  - Weakness: requires periodic re-tuning
  - Best when: product consistency and controlled rollout are priorities

---

## Recommendation

Recommended now: Option 2 (audibility + CPU-budget cap), with Option 1 as fallback path.

Why:
- It respects your explicit hard constraint (10 ms) as non-negotiable.
- It uses cpu_frequency_mhz in a meaningful but bounded way.
- It stays deterministic, easy to test, and avoids startup auto-calibration complexity.

Quick validation plan:
1. Choose policy constants C and phi explicitly.
2. Sweep sample_rate_hz across 44.1k, 48k, 96k and check monotonic behavior.
3. Sweep cpu_frequency_mhz bands and verify no unstable jumps.
4. Confirm outputs satisfy audible lower-bound invariants for all cases.

---

## References

- https://en.wikipedia.org/wiki/Sampling_(signal_processing)
- https://developer.mozilla.org/en-US/docs/Web/API/AudioWorkletProcessor/process
- https://en.wikipedia.org/wiki/Jitter
