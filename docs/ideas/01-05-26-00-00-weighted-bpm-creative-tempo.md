# WeightedBpm Creative Tempo Implementations

## Problem framing

Goal: propose implementations for WeightedBpm::compute that make tempo feel reactive and expressive, using:
- current excitement magnitude per node and per band
- rate-like emphasis through dominance behavior (winner influence)
- hard BPM min and max bounds, but otherwise continuous control
- fast response (sub-beat adaptation)

Scope constraint: compute-only ideas (no changes proposed to add/tick/sample-rate paths).

## Codebase inspiration

Observed in current code:
- ExcitementData is complex (real + imaginary), represented by Complex<OrderedFloat<\S>>.
- WeightedBpm currently accumulates:
  - total and per-band excitement sums
  - total and per-band BPM sums
  - max excitement node and its bpm value (xct_max includes max_node_bpm)
- compute already derives averages but does not yet map them into a final NonZeroU16.

Implication for feasible compute designs:
- You can build tempo from a weighted blend of:
  - global centroid (bpm_total_avg)
  - per-band centroids (bpm_band_avg)
  - dominant-node anchor (max_node_bpm)
- You can create creative behavior using nonlinear weighting only inside compute, without touching upstream data flow.

## Web inspiration

Used as conceptual references for practical mapping:
- Softmax weighting for smooth winner-take-most behavior with temperature control.
  - https://en.wikipedia.org/wiki/Softmax_function
- Sigmoid-style shaping to compress extremes while preserving expressive mid-range changes.
  - https://en.wikipedia.org/wiki/Sigmoid_function
- Exponential smoothing concepts (relevant if you later add stateful damping outside compute).
  - https://en.wikipedia.org/wiki/Exponential_smoothing
- One Euro Filter concept as a high-level reference for fast response with jitter control in interactive signals.
  - https://github.com/casiez/OneEuroFilter

## Idea options (3+)

## Option 1: Thermal Winner-Centroid Blend

Concept:
- Treat each node as an energy source.
- Build node weights from excitement magnitude using softmax:
  - wi = exp(beta * ei) / sum_j exp(beta * ej)
- Map final BPM as:
  - bpm_soft = sum_i wi * bpm_i
- Blend with global average and dominant node:
  - bpm = mix(mix(bpm_total_avg, bpm_soft, a), max_node_bpm, d)

Fit with your direction:
- Directly uses magnitude and winner dominance.
- Very expressive with one "temperature" parameter beta.
- Continuous and naturally bounded by the bpm inputs before final clamp.

Feasibility:
- High. Requires local scalar extraction of excitement magnitude and one pass for stable softmax (subtract max before exp).

Risks:
- Too high beta can make tempo flicker around whichever node barely wins.
- Too low beta can feel too averaged and less performative.

Effort:
- Low to medium.

Suggested constants:
- beta in [2.0, 8.0]
- a in [0.5, 0.8]
- d in [0.1, 0.35]

## Option 2: Band-Conductor with Dominance Override

Concept:
- Build a per-band tempo candidate using band excitement:
  - band_weight_b = |xct_band_avg[b]|^p
  - bpm_band = bpm_band_avg[b]
- Compute conductor tempo:
  - bpm_conductor = sum_b band_weight_b * bpm_band / sum_b band_weight_b
- Add dominance override from the max node:
  - dominance = clamp((e_max - e_total_avg) / (e_max + eps), 0, 1)
  - bpm = mix(bpm_conductor, max_node_bpm, dominance * k)

Fit with your direction:
- Captures macro-band behavior while still letting a standout node push tempo.
- Creative but musically coherent, especially if bands represent distinct physical ranges.

Feasibility:
- High. Uses values already computed in compute.

Risks:
- If one band has structurally larger excitement scale, it may dominate forever.
- Requires selecting p and k well.

Effort:
- Low.

Suggested constants:
- p in [1.2, 2.0]
- k in [0.4, 0.8]

## Option 3: Contrast-Driven Acceleration Curve

Concept:
- Derive a contrast metric from how uneven excitement is:
  - contrast = (e_max - e_total_avg) / (e_max + eps)
- Use contrast to move away from neutral tempo toward energetic anchor:
  - bpm_neutral = bpm_total_avg
  - bpm_anchor = weighted mix of max_node_bpm and bpm_soft
  - bpm = bpm_neutral + g(contrast) * (bpm_anchor - bpm_neutral)
- Use nonlinear gain g(x), for example sigmoid-like or smoothstep, to get "snap" only when contrast is clearly high.

Fit with your direction:
- Gives explicit performance feel: low contrast stays grounded, high contrast bursts toward dominant tempo.
- Works well for fast response while avoiding constant jitter at low excitement differences.

Feasibility:
- High.

Risks:
- Needs careful shaping of g(x) or it can feel either too timid or too jumpy.

Effort:
- Medium.

Suggested constants:
- contrast threshold around 0.15 to 0.25
- max gain near 0.8

## Option 4: Real-Imag Role Split (Expressive Color Mapping)

Concept:
- Use real and imaginary excitement channels intentionally:
  - real channel drives baseline weighted tempo
  - imaginary channel modulates "agitation" amount
- Example mapping:
  - bpm_base from real-only weights
  - agitation = normalized mean imaginary magnitude
  - bpm = bpm_base + agitation * (max_node_bpm - bpm_base) * k

Fit with your direction:
- Exploits the fact excitement is complex rather than collapsing it immediately.
- Creates timbral-to-rhythmic coupling: phase-like/imag behavior can change tempo color.

Feasibility:
- Medium. Depends on whether imaginary channel already has musically meaningful semantics in your upstream system.

Risks:
- If imaginary channel is noisy or not semantically stable, tempo may become erratic.

Effort:
- Medium.

Suggested constants:
- k in [0.2, 0.6]

## Comparison summary

- Most controllable and robust: Option 1
- Most structurally musical by design bands: Option 2
- Most performative energy gestures: Option 3
- Most experimental/creative mapping: Option 4

## Recommendation

Recommended first implementation: Option 3 (Contrast-Driven Acceleration Curve), with Option 1 internally for bpm_anchor.

Why this is strongest for your stated goals:
- You wanted reactive and expressive movement, not just static weighted average.
- Contrast gives a clear musical trigger for acceleration bursts.
- Nonlinear gain creates fast feel without making every tiny excitement change move tempo equally.
- It stays compute-local and uses your already available max/avg statistics.

Quick validation plan:
1. Print or snapshot bpm output over synthetic excitement scenarios:
   - flat excitement across all nodes
   - one dominant node burst
   - rotating dominance between nodes
2. Listen for three criteria:
   - no low-level chatter in flat state
   - clear acceleration under dominance
   - return to baseline when dominance fades
3. Tune only two knobs first:
   - contrast threshold
   - max gain

## References

- https://en.wikipedia.org/wiki/Softmax_function
- https://en.wikipedia.org/wiki/Sigmoid_function
- https://en.wikipedia.org/wiki/Exponential_smoothing
- https://github.com/casiez/OneEuroFilter
