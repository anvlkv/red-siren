# Robust Instrument Config Node Generation

**Date**: 25 Apr 2026  
**Problem**: Unstable mass/volume progression in node config generation causes implausible values (e.g., 10¹⁴ kg bodies) and broken snapshots.  
**Goal**: Derive deterministic, monotonic, physically-grounded l_mm / w_kg / v_cm3 from screen estate for organo-metallic fantasy resonators.

---

## Problem Framing

Current generation loop in [config.rs](crates/common/src/instrument/config.rs#L156-L222):
```rust
w_kg *= w_kg / (n + 1) as f64;
```

This creates **exponential explosion**:
- Node 0: 1.78 kg (sensible)
- Node 6: 594 kg (plausible)
- **Node (1,1): 3.2×10¹⁷ kg** (impossible)

**Root cause**: Squaring mass at each step compounds across bands without reset.

**Snapshot issue** ([common__instrument__config__node__tests__config_1920x1080.snap](crates/common/src/instrument/config/snapshots/common__instrument__config__node__tests__config_1920x1080.snap#L403-L411)):
- hr_bpm collapses to 1–2 (inverse ∝ mass^0.25)
- body_density shoots to 10²⁵ g/cm³ (impossible for any material)
- Buoyant force becomes nonsense (denser than neutron star)

---

## Constraints & Requirements

From user:
1. **Focus only on physical params**: l_mm, w_kg, v_cm3 (derive hr, density, buoyancy).
2. **Deterministic**: no randomness; same layout → same config.
3. **Monotonicity**: strict: l_mm, w_kg, v_cm3 must increase across nodes within group, and across bands.
4. **Mass range**: mouse (~10g) to whale (~100kg+).
5. **No layout/channel changes**: work with existing screen estate mapping.
6. **Physically-inspired**: ground in resonator/string physics to justify organo-metallic timbre.
7. **Sonic goal**: dynamic spread, dark/bright timbre (scale-dependent), strong beating/chorus.

---

## Codebase Inspiration

**Current constraints** (from [config.rs](crates/common/src/instrument/config.rs) and [node.rs](crates/common/src/instrument/config/node.rs)):
- `l_mm` increments linearly per node, jumps per group: simple, stable ✓
- `v_cm3` increments linearly per node: simple, stable ✓
- **`w_kg` multiplies itself**: unstable ✗
- Derived metrics:
  - `hr_bpm(w_kg) = 241 * w_kg^-0.25` (heart rate; inverse scaling)
  - `body_density = w_kg * 1000 / v_cm3`
  - `buoyant_force = v_cm3 * fluid_density * 9.80665`

**Validation** (from tests):
- Safe frequency bounds: ~20 Hz – 20 kHz (human hearing)
- Soft bounds: ~50 Hz – 16 kHz (recommended)
- Max cumulative node count: `MAX_DBS` (power budget)
- All nodes per group must have same count

---

## Web Inspiration

**Mersenne's Laws** (vibrating string):  
$$f = \frac{1}{2L} \sqrt{\frac{T}{\mu}}$$

- f: frequency
- L: string length
- T: tension
- μ: linear mass density

**Implications**:
- Frequency ∝ √(T/μ): to increase pitch, increase tension or decrease linear density.
- Metallic bodies (bells, plates) behave differently: mode frequencies depend on geometry and stiffness, but mass still affects modal damping and harmonic spacing.
- **Beating** arises when two frequencies are close but not identical; width and beat rate are proportional to frequency difference and energy coupling.

---

## Solution Ideas

### Solution 1: Controlled Linear Progression (Safest, Fastest)

**Concept**:  
Global linear interpolation from `min_w_kg` to `max_w_kg`, distributed across all nodes. Per-group offsets ensure monotonicity across bands.

**Algorithm**:
```
min_w_kg = 0.01 kg (10g, mouse)
max_w_kg = layout.space.x / 10 + layout.space.y / 10 (e.g., 270 kg for 1920×1080)

For each node n in [0, total_nodes):
  w_kg[n] = min + (max - min) * (n / total_nodes)
  
(Optional: apply small per-group multiplier to offset ranges)
```

**Fit**:
- ✅ Simple, deterministic, reproducible.
- ✅ Monotonicity guaranteed by construction.
- ✅ Values stay in plausible range (10g–300kg).

**Feasibility**: 🟢 High.  
**Risks**: May sound artificial; no beating unless combined with frequency spread.  
**Effort**: 1–2 hours.

---

### Solution 2: Harmonic Series Scaling (Physical, Intermediate)

**Concept**:  
Back-compute linear mass density (μ) from your scale's target frequencies using Mersenne's laws, then derive node mass.

**Algorithm**:
```
For each group g:
  octave_f_base = fundamental_frequency(n_base, v_wave, L_string)
  
  For each node n in group:
    target_freq = scale.freq_n(n, octave_f_base, ...)
    
    Assume fixed tension T and string length L.
    Solve for μ: f = (1/(2L)) * sqrt(T/μ)
      → μ = T / (4 * L² * f²)
    
    w_kg[n] = μ * effective_length
    
(Add beating: vary μ smoothly toward next octave boundary)
```

**Fit**:
- ✅ Grounded in acoustic physics.
- ✅ Justifies mass by frequency requirement.
- ✅ Beating arises naturally from harmonic drift.

**Feasibility**: 🟡 Medium (algebra, validation).  
**Risks**: Complex; iterative solving; error propagation.  
**Effort**: 4–6 hours.

---

### Solution 3: Piecewise Exponential with Per-Band Reset (Practical Sweet Spot)

**Concept**:  
Each group claims a budget of mass. Within a group, apply controlled power law (exponent ~0.5–0.8) to avoid exponential explosion. Reset at group boundary.

**Algorithm**:
```
min_w_kg = 0.01 kg
max_w_kg = 200 kg
group_count = num_bands

group_step = (max_w_kg - min_w_kg) / group_count
group_min[g] = min_w_kg + g * group_step
group_max[g] = min_w_kg + (g + 1) * group_step

exponent = 0.6  (tunable in [0.4, 0.8])

For each node n in group g:
  progress = n / nodes_per_group  (in [0, 1))
  w_kg[n] = group_min[g] + (group_max[g] - group_min[g]) * progress^exponent
```

**Fit**:
- ✅ Simple formula, stable (power law doesn't explode).
- ✅ Progressive heaviness; per-group budget caps explosion.
- ✅ Tunable: adjust exponent for musical feel.

**Feasibility**: 🟢 High.  
**Risks**: Exponent choice is heuristic; may need A/B testing.  
**Effort**: 2–4 hours.

---

### Solution 4: Resonator Volume–Density Model (Physically Direct)

**Concept**:  
Model nodes as hollow metallic bodies. Volume (`v_cm3`) is already monotonic; compute body density as a smoothly increasing function, then `w_kg = volume × density`.

**Algorithm**:
```
base_density = 0.5 g/cm³  (adjustable)
max_density = 3.0 g/cm³   (adjustable, realistic for metals)

density_exponent = 0.3  (gentle curve)

For each node n:
  progress = n / total_nodes
  density[n] = base_density + (max_density - base_density) * progress^density_exponent
  
  w_kg[n] = (v_cm3[n] / 1000) * density[n]
```

**Fit**:
- ✅ Interprets nodes as physical resonant bodies.
- ✅ Density variation creates natural formant/beating.
- ✅ Density ranges are realistic (0.5–3 g/cm³ for metal/alloy).

**Feasibility**: 🟡 Medium (tuning density range).  
**Risks**: Density range too wide → beat rate too fast; too narrow → insufficient spread.  
**Effort**: 3–4 hours.

---

### Solution 5: Tension-Driven String Model (Most Organo-Metallic)

**Concept**:  
Model nodes as sections of metallic strings with increasing tension and smoothly varying linear density. Naturally produces heterodyning (beating) and justifies organo-metallic sound.

**Algorithm**:
```
L_string = layout.diagonal  (fixed)
base_tension = layout.space.x * 100  (N, tunable)
base_μ = 0.1 kg/m  (linear density, tunable)

tension_exponent = 0.5  (e.g., tension^0.5 grows slowly)
density_exponent = 0.4  (e.g., density grows even slower)

For each node n:
  progress = n / total_nodes
  
  T[n] = base_tension * (1 + progress)^tension_exponent
  μ[n] = base_μ * (1 + progress)^density_exponent
  
  w_kg[n] = μ[n] * L_string
  
  Validate: f_expected = (1/(2*L)) * sqrt(T/μ)
            f_expected ≈ scale.freq_n(...) ?
```

**Fit**:
- ✅ Justifies organo-metallic via multi-string heterodyning.
- ✅ Direct link between mass and frequency.
- ✅ Beating emerges from tension/density mismatch.

**Feasibility**: 🟡 Medium (tuning exponents, validation loop).  
**Risks**: Tight coupling; validation may require iteration.  
**Effort**: 5–8 hours.

---

## Recommendation

**Best first step**: **Solution 3 (Piecewise Exponential)**.
- Lowest risk, fastest implementation.
- Guarantees monotonicity and plausibility.
- Exponent tuning is straightforward A/B test.
- Provides baseline for comparison.

**Validation checklist**:
- [ ] All w_kg values in [0.01, 500] kg.
- [ ] Monotonic across nodes and bands.
- [ ] hr_bpm in [1, 250] range.
- [ ] body_density in [0.1, 10] g/cm³.
- [ ] Snapshot diffs show sensible values (no 10ⁿ exponents).

**Next step (if needed)**: Layer **Solution 5** afterward to justify physics and add beating.

---

## Implementation Path

1. **Isolate generation logic** (2–4 lines change):
   - Replace `w_kg *= w_kg / (n + 1)` with power-law formula.
   - Precompute min/max per group.

2. **Test against snapshot**:
   - Regenerate snapshot.
   - Inspect first and last few nodes; check decimal places, ranges.

3. **Tune exponent** (if needed):
   - Try exponent ∈ [0.4, 0.8].
   - Ear-test: does the instrument sound less monotonous?

4. **(Optional) Add density validation**:
   - Clamp body_density to [0.1, 10] to catch overflow early.

---

## References

- [Vibrating String - Wikipedia](https://en.wikipedia.org/wiki/String_vibration)
  - Mersenne's laws: frequency ∝ √(tension / linear_density).
- [Red Siren Instrument Config](crates/common/src/instrument/config.rs)
- [Red Siren Node Physics](crates/common/src/instrument/config/node.rs)
