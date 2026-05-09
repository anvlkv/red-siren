# Controller Sequence Families

**Date**: 25 Apr 2026  
**Problem**: Replace the current powers-of-two / powers-of-three scheduler tables in [crates/audio-system/src/system/node/controller.rs](crates/audio-system/src/system/node/controller.rs) with richer integer families that better fit the controller's current mixed divisible/divisor model.  
**Goal**: Keep the strong simple feel of low powers of two, replace the powers-of-three side with something more musical, and use the separate divisible/divisor outputs intelligently instead of treating both sides as the same kind of sequence.

---

## Questions Asked

1. Keep powers of two?  
Answer: open to reshaping.

2. What should replace the non-power-of-two lane?  
Answer: Fibonacci plus another sequence.

3. What contrast should the second family provide?  
Answer: more harmonic.

4. How should the two families combine?  
Answer: mix within each state rather than one family per accent state.

5. Sequence size?  
Answer: open to more than 9 if the proposed sequences justify it.

---

## Problem Framing

The current controller does **not** actually need “powers” as such. It needs:

- one indexed integer bank for `start divisible`
- one indexed integer bank for `start divisor`
- one indexed integer bank for `duration divisible`
- one indexed integer bank for `duration divisor`

Those outputs are selected by the same index mapper in [crates/audio-system/src/system/node/controller.rs](crates/audio-system/src/system/node/controller.rs), and the duration path already multiplies the selected integer by `cents / 1000`. That means the controller is better understood as a **ratio selector** than as a strict “pick one scalar from one sequence” system.

That matters because it opens a stronger design space:

- we can keep one family “organic” or recursive, like Fibonacci
- we can make the second family explicitly harmonic, using low-integer ratio material
- we can use the divisible/divisor split to form musically meaningful pairs instead of just mirroring one family on both sides

The practical implication is that [crates/audio-system/src/system/node/controller.rs](crates/audio-system/src/system/node/controller.rs#L7) should probably stop thinking in terms of `NUM_POWS` and start thinking in terms of a more general `NUM_LEVELS` or `NUM_STEPS` when this is implemented.

---

## Codebase Inspiration

### Current controller shape

From [crates/audio-system/src/system/node/controller.rs](crates/audio-system/src/system/node/controller.rs):

- The same normalized index mapping feeds all four scheduler outputs.
- Start and duration are already driven by different control mixes.
- Accent is currently what swaps table families.
- Duration divisible is the only place where the selected integer is further scaled by `cents`.

This suggests the best replacement should:

- have smooth coverage from low to high indices
- avoid explosive growth too early
- give useful low-integer ratios when one table is divided by another
- preserve enough spread for snapshot variation

### Node-derived signals available now

From [crates/common/src/instrument/config/node.rs](crates/common/src/instrument/config/node.rs):

- `hr_bpm()` gives a node-specific temporal tendency
- `body_density_g_cm3()` gives a physical density tendency
- `cents` gives a per-node duration scaler already used in the controller

So the tables do not need to encode everything. They only need to provide a **characterful lattice** for those existing continuous drives to land on.

---

## Web Inspiration

### Fibonacci as rhythmic material

The Fibonacci sequence has an actual rhythmic pedigree, not just a visual/numerological one: historical prosody work links it to counting long/short syllable combinations. That makes it a credible source for “organic pulse grouping,” not just a decorative choice.

### Harmonic series as low-integer musical structure

The harmonic series shows why small integer ratios are musically strong: low harmonics generate stable, consonant relationships, while higher harmonics become denser and less perceptually distinct. For this controller, that argues for keeping the second family centered on **small integers and ratio quality**, not just another fast-growing recurrence.

### Euclidean rhythm as a balancing principle

Euclidean rhythm is not a direct replacement for these integer tables, but it is useful as a design principle: distribute contrast evenly and avoid front-loaded clumping. That is relevant when choosing which entries deserve more density near the low end of the sequence.

---

## Idea Options

### Option 1: Fibonacci + Harmonic Ladder

**Concept**:  
Use Fibonacci for the more organic side and a curated harmonic ladder for the more consonant side.

**Candidate banks**:

- Fibonacci bank: `1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144`
- Harmonic bank: `1, 2, 3, 4, 5, 6, 8, 9, 12, 16, 18`

**Combination idea**:

- `start divisible` from Fibonacci
- `start divisor` from harmonic ladder
- `duration divisible` from harmonic ladder
- `duration divisor` from Fibonacci

This gives cross-family ratios both ways, instead of one family owning one accent state.

**Why it fits**:

- Fibonacci gives non-uniform but still readable growth.
- The harmonic ladder gives small-integer divisors that feel more musical than powers of three.
- Cross-assigning the families makes the controller's separate outputs do real work.

**Feasibility**: high  
**Risks**:

- Raw Fibonacci gets wide quickly, so the upper entries may need trimming or a longer table with gentler spacing.
- The harmonic bank is curated rather than generated, which is fine musically but less mathematically pure.

**Effort**: low

---

### Option 2: Fibonacci + Lucas Dual Recurrence

**Concept**:  
Use two closely related recurrence families: Fibonacci for one side, Lucas for the other.

**Candidate banks**:

- Fibonacci bank: `1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144`
- Lucas bank: `1, 3, 4, 7, 11, 18, 29, 47, 76, 123, 199`

**Combination idea**:

- divisible outputs prefer Fibonacci
- divisor outputs prefer Lucas
- accent flips which of start/duration gets the “cleaner” family

**Why it fits**:

- Both families share recurrence DNA, so the controller keeps one coherent identity.
- Lucas is slightly less familiar and less square than powers of two, but not as abrasive as powers of three.
- Ratios between nearby Fib/Lucas entries often feel smoother than raw ternary jumps.

**Feasibility**: high  
**Risks**:

- This is mathematically elegant, but it is not as explicitly harmonic as your stated target.
- It may sound “interesting” rather than “musically stronger.”

**Effort**: low

---

### Option 3: Fibonacci + Explicit Just-Ratio Pair Bank

**Concept**:  
Keep Fibonacci as one family, but stop forcing the second family to be a plain integer sequence. Instead, define a bank of low-integer harmonic ratios and let divisible/divisor carry numerator and denominator separately.

**Candidate harmonic ratio bank**:

- `1/1`
- `9/8`
- `5/4`
- `4/3`
- `3/2`
- `5/3`
- `15/8`
- `2/1`
- optional color tones: `7/4`, `11/8`, `13/8`

This can be represented as paired integer banks, for example:

- harmonic numerators: `1, 9, 5, 4, 3, 5, 15, 2, 7, 11, 13`
- harmonic denominators: `1, 8, 4, 3, 2, 3, 8, 1, 4, 8, 8`

Then combine them with Fibonacci intelligently:

- one output picks from Fibonacci integers
- the other picks from harmonic numerator/denominator material
- accent or duration can invert numerator/denominator emphasis

**Why it fits**:

- This uses the controller's current architecture more intelligently than any plain sequence swap.
- It is the most directly aligned with your “Fibonacci plus something more harmonic” goal.
- It makes divisible/divisor semantically meaningful as **ratio construction**, not just separate arbitrary integers.

**Feasibility**: medium-high  
**Risks**:

- This is no longer a simple “replace one sequence with another” change.
- You need to decide whether monotonic table ordering matters more than ratio identity.
- Snapshot tuning will take one more pass because some ratios may cluster perceptually.

**Effort**: medium

---

### Option 4: Fibonacci + Euclidean Meter Counts

**Concept**:  
Use Fibonacci for one family and use a bank derived from musically common Euclidean beat/step counts for the other, such as `3, 4, 5, 7, 8, 12, 13, 16`.

**Candidate Euclidean bank**:

- `1, 3, 4, 5, 7, 8, 12, 13, 16, 21, 24`

**Combination idea**:

- Fibonacci handles organic scale-up
- Euclidean counts handle more groove-like divisor selection
- duration can bias toward denser Euclidean values while start stays more Fibonacci-led

**Why it fits**:

- Gives more rhythmic asymmetry without the harshness of powers of three.
- Connects directly to beat-spacing ideas rather than only number growth.

**Feasibility**: medium  
**Risks**:

- This is rhythmically strong but less harmonic than you asked for.
- It may overlap too much with the existing rhythm control instead of complementing it.

**Effort**: medium

---

## Comparison

| Option | Character | Harmonic Quality | Architectural Fit | Risk | Effort |
| --- | --- | --- | --- | --- | --- |
| Fibonacci + Harmonic Ladder | organic + consonant | high | high | low-medium | low |
| Fibonacci + Lucas | coherent + mathematical | medium | high | medium | low |
| Fibonacci + Explicit Just-Ratio Pair Bank | most musical and intentional | very high | very high | medium | medium |
| Fibonacci + Euclidean Meter Counts | groove-oriented | medium-low | medium | medium | medium |

---

## Recommendation

**Recommend Option 3: Fibonacci + Explicit Just-Ratio Pair Bank.**

This is the strongest use of the controller's existing structure.

Reasoning:

1. The controller already emits separate divisible and divisor channels, so ratio-bank thinking is native to the design.
2. You explicitly want Fibonacci plus something more harmonic, and explicit low-integer ratios satisfy that more directly than Lucas or Euclidean counts.
3. It avoids the main weakness of powers of three: they grow fast without giving especially rich interval meaning.
4. It still leaves room to keep a powers-of-two flavor if desired, because `2/1`, `4/3`, `3/2`, and octave-related ratios preserve some of that clarity.

### Recommended implementation shape

If you turn this into code later, I would suggest:

- replace `NUM_POWS` with a generic `NUM_LEVELS`
- move from two scalar banks to either:
  - two families of scalar banks, or preferably
  - one organic scalar bank plus one paired harmonic ratio bank
- start with **11 entries**, not 9

A strong first candidate would be:

- Fibonacci: `1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144`
- Harmonic ratios: `1/1, 9/8, 5/4, 4/3, 3/2, 5/3, 15/8, 2/1, 7/4, 11/8, 13/8`

And then map them like this:

- `start divisible`: Fibonacci
- `start divisor`: harmonic denominator bank
- `duration divisible`: harmonic numerator bank
- `duration divisor`: Fibonacci

That gives both rhythmic growth and musically interpretable ratios.

---

## Quick Validation Plan

1. Run the current NodeController snapshots with the new banks and compare whether the selected bins are less edge-biased than the powers-of-three version.
2. Add a tiny controller analysis test that logs selected index histograms for all four scheduler outputs.
3. Listen for whether the duration path feels more interval-shaped and less arbitrarily stepped.
4. If the upper Fibonacci entries feel too wide, trim to 10 or 11 entries and clamp the top two indices from normal control reach.

---

## References

- [crates/audio-system/src/system/node/controller.rs](crates/audio-system/src/system/node/controller.rs)
- [crates/common/src/instrument/config/node.rs](crates/common/src/instrument/config/node.rs)
- Fibonacci sequence: rhythmic/prosodic history and recurrence properties
- Harmonic series (music): low-integer interval structure and interval strength
- Euclidean rhythm: even-distribution principle for rhythmic spacing
