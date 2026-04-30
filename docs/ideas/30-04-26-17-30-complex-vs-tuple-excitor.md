# Complex vs Tuple in Excitor: Idea Exploration

## Problem Framing

Current state: in the active Excitor path, complex numbers and `(re, im)` tuples are functionally equivalent for simple transport.

Goal: identify forward-looking reasons to keep or expand `Complex<S>` usage for interesting effects and DSP evolution, especially where it provides leverage over a plain tuple.

Success criteria:
- Benefits should be practical for upcoming audio features.
- Options should be compatible with existing architecture patterns in `audio-system`.
- Tradeoffs should be explicit (ergonomics, correctness, performance, maintenance).

## Codebase Inspiration

Observed patterns in this repo suggest `Complex` is already a conceptual model, not just a storage choice:

- `Excitor` control exposes values as `Complex<S>` while still accepting tuple writes.
- Legacy/adjacent system code describes real/imag as physical control dimensions (e.g., hit strength + impact position), indicating natural 2D parameter semantics.
- Analyzer/ADSR code carries excitation state as `Complex<S>` per node, implying envelope smoothing is already done in 2D.
- FFT analysis paths in the codebase naturally align with complex-domain processing expectations.

Representative files inspected:
- `crates/audio-system/src/system/excitor.rs`
- `crates/audio-system/src/_system/excitor/control.rs`
- `crates/audio-system/src/_system/input/analyzer.rs`
- `crates/audio-system/src/_system/input/adsr.rs`

## Web Inspiration

- `rustfft` examples and API model FFT buffers as complex values, reinforcing ecosystem interop around `Complex`.
- `num-complex` provides a first-class complex type and complex-float trait surface useful for reusable DSP operations.
- Analytic signal references emphasize direct access to envelope (`|z|`) and phase (`arg(z)`), which maps well to expressive audio modulation and effects.

References:
- https://docs.rs/rustfft/latest/rustfft/
- https://docs.rs/num-complex/latest/num_complex/
- https://en.wikipedia.org/wiki/Analytic_signal
- https://ccrma.stanford.edu/~jos/fp/Complex_Numbers.html

## Idea Options

### Option 1: Complex as Semantic Type for Excitation State

Concept:
Use `Complex<S>` as the canonical in-memory excitation representation, with tuple only at UI/IPC boundaries.

Why this fits:
- Matches existing analyzer + ADSR use.
- Encodes “paired dimensions that transform together” better than a generic tuple.

Feasibility:
- High. Mostly API surface alignment and helper methods.

Benefits over tuple:
- Stronger intent: operations look like signal operations, not arbitrary pair math.
- Fewer accidental swaps (`re`/`im`) and less ad-hoc glue.
- Easier to add shared helpers (`magnitude`, `phase`, `rotate`, `normalize`) once and reuse.

Risks:
- Team members unfamiliar with complex DSP may need quick onboarding.

Effort:
- Low.

### Option 2: Polar-Domain Modulation Effects (Envelope/Phase FX)

Concept:
Treat excitation as `z = r e^{j\phi}` and build effects in polar space:
- `r = |z|` as drive/intensity envelope,
- `\phi = arg(z)` as phase-like spatial/character control.

Example effects:
- “Swirl”: phase rotation proportional to energy.
- “Edge hit”: radial thresholding + phase warping.
- “Tension shimmer”: slow phase drift with magnitude-coupled depth.

Why this fits:
- Your stated goal is interesting effects.
- Complex representation makes this direct and mathematically clean.

Feasibility:
- Medium. Requires stable mappings from excitation state into audible parameters.

Benefits over tuple:
- Polar transforms are one-liners with `Complex`; tuple requires custom, error-prone trig plumbing.
- Better conceptual language for musically expressive modulation.

Risks:
- Phase wrapping artifacts if not smoothed/unwrapped.
- Needs parameter tuning to avoid unintuitive behavior.

Effort:
- Medium.

### Option 3: Frequency-Domain Ready Path (FFT Bin Algebra)

Concept:
Adopt/retain complex values along FFT-related pipelines so bin-wise operations become native:
- complex gain,
- phase offsets,
- conjugate/energy operations,
- coherent accumulation/cancellation.

Why this fits:
- Existing FFT windowing and spectrum analysis indicate future expansion potential.
- Rust FFT ecosystem already standardizes on complex buffers.

Feasibility:
- High for preparation, Medium for full feature rollout.

Benefits over tuple:
- Immediate interop with `rustfft` APIs.
- Less conversion overhead/boilerplate.
- Fewer representation mismatches during experiments.

Risks:
- Can invite overengineering if near-term work remains scalar.

Effort:
- Low to medium (depends on how far spectral features go).

### Option 4: Rotation-Matrix-Free 2D Dynamics

Concept:
Use complex multiplication as a compact 2D transform primitive:
- rotation,
- anisotropic-like shaping via chained operators,
- stable oscillatory coupling between control dimensions.

Why this fits:
- Excitation appears naturally two-dimensional in current and legacy comments.

Feasibility:
- Medium.

Benefits over tuple:
- Complex multiply encodes coupled 2D transforms tersely and consistently.
- Reduces hand-written matrix math and sign mistakes.

Risks:
- Requires clear docs for designers/devs to tweak confidently.

Effort:
- Medium.

### Option 5: Typed Layering Strategy (Best of Both)

Concept:
Keep tuple for external protocols and serde surfaces, but enforce `Complex<S>` internally in DSP/control layers.

Why this fits:
- Preserves compatibility while gaining internal DSP ergonomics.

Feasibility:
- High.

Benefits over tuple-only:
- Avoids churn in external contracts.
- Gains most internal correctness/expressiveness benefits.

Risks:
- Dual representation can drift without conversion helpers and tests.

Effort:
- Low.

## Idea Comparison

| Option | Creative effect potential | Near-term practicality | Performance outlook | Main risk |
|---|---|---|---|---|
| 1 Semantic type | Medium | High | Neutral to slight positive | Team familiarity |
| 2 Polar modulation FX | High | Medium | Neutral | Phase handling artifacts |
| 3 FFT-ready path | Medium-High | Medium-High | Positive for spectral work | Premature complexity |
| 4 2D dynamics via complex ops | High | Medium | Neutral | Tuning complexity |
| 5 Typed layering | Medium | High | Neutral | Representation drift |

## Recommendation

Strongest combined path: Option 5 + Option 2.

- Use Option 5 as a low-risk foundation: internal complex model, tuple only at boundaries.
- Build one flagship polar-domain effect prototype from Option 2 to validate creative payoff quickly.

Why this is best now:
- Minimal architectural risk.
- Immediate room for musically distinctive behavior.
- Keeps future FFT/spectral expansion straightforward.

## Quick Validation Plan

1. Standardize a tiny internal helper API around `Complex<S>` (magnitude, phase, rotate).
2. Implement one prototype effect: magnitude-driven phase rotation with smoothing.
3. A/B compare against tuple implementation on:
   - code size/clarity,
   - bug surface (component swaps, sign errors),
   - sonic range and controllability.
4. Keep tuple adapters at boundaries only; measure if internal conversions disappear.

## References

- https://docs.rs/rustfft/latest/rustfft/
- https://docs.rs/num-complex/latest/num_complex/
- https://en.wikipedia.org/wiki/Analytic_signal
- https://ccrma.stanford.edu/~jos/fp/Complex_Numbers.html
