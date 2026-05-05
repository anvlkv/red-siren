# Mixer Up-mix / Down-mix Implementation Ideas

**Date:** 2026-05-05  
**File:** `crates/audio-system/src/system/mixer.rs`  
**Goal:** Implement the `todo!()` up-mix arm and improve the existing down-mix arm inside `Mixer::tick`.

---

## Problem Framing

The `Mixer<S, I, O>` node sits at the boundary between the 2-channel instrument subnet and the CPAL output device. The system always creates it as `Mixer<S, U2, NumType>` (stereo in → N channels out), so **up-mixing is the primary path**. The down-mix arm exists but currently only copies matched channels with no energy correction — it never normalises.

Requirements gathered:
- **Primary task:** up-mix stereo (U2) → any NumChannels (1–8)
- **Down-mix correction:** energy-preserving (normalise by contributor count)
- **Up-mix derivation:** phantom center (L+R average for missing Center channel)
- **Real-time constraints:** standard `tick` — no allocs, no locks per frame

---

## Codebase Inspiration

| Location | Relevant pattern |
|---|---|
| [system.rs](../../crates/audio-system/src/system.rs#L48) | `create_mixer::<S, U2, NumType>()` — mixer always receives stereo from the subnet |
| [mixer.rs](../../crates/audio-system/src/system/mixer.rs#L90) | `NumChannels::channel_index` + layout slices already encode speaker positions |
| [output.rs](../../crates/audio-system/src/rt/cpal/stream/output.rs#L262) | `write_interleaved` already expects only L+R from the generator; extra channels are zeroed — up-mixing inside the mixer is the correct layer to fill them |
| [node/formant.rs](../../crates/audio-system/src/system/node/formant.rs#L148) | `WDMixer` example of a wet/dry blend using only arithmetic, no allocs |

The output stream's `write_interleaved` zeros any channels beyond index 1, so the mixer is the **only** place multi-channel audio reaches the CPAL buffer. Whatever the mixer puts in `Frame<f32, O>` is what the device receives.

---

## Web Inspiration

- **Phantom center** — a psychoacoustic phantom image at the midpoint of L+R. With equal-amplitude signals, `C = (L + R) × (1/√2) ≈ 0.707 × (L + R)`. The `-3 dB` coefficient preserves energy relative to a direct channel. (Wikipedia: Phantom center, Pan law)
- **ITU-R BS.775** — the reference standard for stereo→5.1 down-mix/up-mix coefficients: direct L/R at 1.0, phantom center at `1/√2`, surrounds derived from L/R at attenuated levels. (Wikipedia: Surround sound § Standard configurations)
- **Mid/side (Haas) decorrelation** — surrounds can be derived as the difference signal `S = (L − R) × k`, which creates a sense of spatial envelopment without phase-cancellation artefacts on mono summing. Classic upmix technique for music.
- **Energy-preserving down-mix** — normalise each output channel by the number of input channels contributing to it: `out[i] /= contributor_count[i]`. Prevents loudness jumps on fold-down.

---

## Idea Options

### Idea A — Channel-Match-and-Derive (extend current pattern)

**Concept:** Keep the existing `channel_index` loop pattern for matched channels (direct copy), then add a second pass that fills unmatched output channels by rule:
- `Center` → `(FL + FR) × 0.707` (phantom center at −3 dB)
- `RearLeft` / `SideLeft` → `FL × 0.5`
- `RearRight` / `SideRight` → `FR × 0.5`
- `SubWoofer` → `(FL + FR) × 0.5`

Down-mix fix: count contributors per output slot; divide after the summation loop.

**Fit:** Directly extends the existing `a > b` / `a < b` structure, minimal new code surface. Stays close to the current `NumChannels` enum abstraction.

**Feasibility:** High. No new types or state. A `[u8; O::USIZE]` contributor counter avoids allocation if bounded at compile time; for the fixed small channel counts (max 8) an `[u8; 8]` stack array indexed up to `O::ISIZE` suffices.

**Risks:**
- The rear/side attenuation (0.5 = −6 dB) is not ITU-standard; it is a rough approximation. Fine for a chime instrument, but may sound tame on true surround systems.
- `SubWoofer` gets a flat mix with no low-pass — not a real LFE channel, but acceptable without a filter.

**Effort:** ~40 lines. Modify the existing `a < b` arm and add contributor-count normalisation to the `a > b` arm.

---

### Idea B — Static Coefficient Table

**Concept:** Replace the runtime channel-logic entirely with a static `[[f32; 8]; 8]` mixing matrix, one per `(I, O)` layout combination. Populate them once as `const` or `lazy_static!` values with proper ITU-R BS.775-derived coefficients. The `tick` body becomes a simple double-loop matrix-vector multiply.

Example stereo→5.1 matrix (rows = output channels FL/FR/C/LFE/RL/RR, cols = input FL/FR):
```
         FL      FR
FL       1.000   0.000
FR       0.000   1.000
C        0.707   0.707
LFE      0.500   0.500
RL       0.500   0.000
RR       0.000   0.500
```

**Fit:** Cleanest separation of mixing policy from mechanism. All coefficients are visible and auditable in one place. Easy to tune per layout pair.

**Feasibility:** Medium-high. Requires defining matrices for all 64 (I, O) pairs the macro can instantiate, though in practice only a handful are used. `const` float arrays are stable Rust.

**Risks:**
- More upfront data entry for all layout combinations.
- The double-loop over `[u8; 8]` indices is 64 multiplications worst-case per tick sample — negligible, but less zero-overhead than Idea A.
- Must align matrix row/column indices with the `layout()` slice ordering exactly or produce wrong channel routing.

**Effort:** ~80 lines of data + ~15 lines of loop. Significant coefficient definition effort; mechanical but error-prone without tests.

---

### Idea C — Mid/Side Spatial Spread (psychoacoustic envelopment)

**Concept:** Decompose the stereo input into a **mid** signal `M = (L + R) × 0.707` and a **side** signal `S = (L − R) × 0.707` (mid-side transform). Then assign:

| Output channel | Signal |
|---|---|
| `FrontLeft` | `L` (direct) |
| `FrontRight` | `R` (direct) |
| `Center` | `M` (phantom center) |
| `SideLeft` | `S` (side signal, positive polarity) |
| `SideRight` | `-S` (side signal, negative polarity for decorrelation) |
| `RearLeft` | `M × 0.5 + S × 0.25` (ambient blend) |
| `RearRight` | `M × 0.5 − S × 0.25` (ambient blend, decorrelated) |
| `SubWoofer` | `M × 0.5` (low-proxy) |

Down-mix: same contributor-count normalisation as Idea A.

**Fit:** Musically superior for a noise-chime instrument — the `S` signal carries spatial width; feeding it to surrounds creates a natural sense of the instrument filling a room. Red Siren's nodes produce pitched resonances with inherent stereo width, so the side signal will not be noise.

**Feasibility:** High. Only four scalar ops per sample (`M`, `S`, negation, blends). No allocs, purely arithmetic. The M/S decomposition is a standard `1/√2` multiply.

**Risks:**
- `SideLeft = S` and `SideRight = -S` are anti-phase; on mono summing they cancel, which is correct but surprising if someone routes a surround output back to stereo without decoding.
- Subjective tuning of rear-blend coefficients (0.25, 0.5) should be validated by ear. They are chosen to keep rear energy below front energy by ~6 dB.
- This idea does not generalize as cleanly as Idea B for all possible `(I, O)` pairs beyond the stereo-in case.

**Effort:** ~25 lines for the up-mix arm. Simplest implementation.

---

## Idea Comparison

| Criterion | A — Match+Derive | B — Coefficient Table | C — M/S Spread |
|---|---|---|---|
| Code volume | Medium | High (data) | Low |
| Correctness | Good (approx ITU) | Best (explicit) | Good (psychoacoustic) |
| Perceptual quality | Adequate | Adequate | Best for music |
| Generalisability | High | Highest | Medium (stereo-in focus) |
| Auditability | Medium | High | Medium |
| Effort | Low | Medium | Lowest |
| Snapshot-testable | Yes | Yes | Yes |

---

## Recommendation

**Idea C (Mid/Side Spatial Spread)** for the up-mix arm.  
**Idea A contributor-count normalisation** for the down-mix fix.

Rationale: Red Siren is a spatial chime instrument, not a film decoder. The M/S decomposition produces richer spatial envelopment from the instrument's inherent stereo width with the least code. The down-mix fix is self-contained and orthogonal — apply Idea A's normalisation regardless of which up-mix strategy is chosen.

If exact ITU-R compliance later becomes a requirement (e.g., for broadcast), Idea B's coefficient tables can be introduced at that point as a drop-in replacement for the up-mix arm.

### Quick Validation Plan

1. Write a `#[test]` using `insta_fun::assert_audio_unit_snapshot!` with `InputSource::VecByChannel(vec![sine_left, sine_right])` at sample rate 100 Hz for each supported output channel count (1, 2, 3, 4, 5, 6, 8).
2. Check that `FrontLeft == input_left` and `FrontRight == input_right` for all up-mix cases (direct pass-through).
3. Check that `Center ≈ (L+R) × 0.707` (phantom center).
4. Check that down-mixing stereo→mono produces `(L+R) / 2` (energy-preserving fold).
5. Accept SVG snapshots with `cargo insta review` to lock channel routing behaviour.

---

## References

- ITU-R Recommendation BS.775: *Multichannel stereophonic sound system with and without accompanying picture* — defines stereo→5.1 mixing coefficients (−3 dB center, −6 dB surrounds).
- Wikipedia: [Surround sound](https://en.wikipedia.org/wiki/Surround_sound) — channel layouts, LFE notation, bass management.
- Wikipedia: [Phantom center](https://en.wikipedia.org/wiki/Phantom_center) — psychoacoustic model for center derivation.
- Mid/side processing: standard studio technique; `M = (L+R)/√2`, `S = (L−R)/√2`.
- [`write_interleaved`](../../crates/audio-system/src/rt/cpal/stream/output.rs#L245) — current CPAL output path that zeros channels ≥ 2.
