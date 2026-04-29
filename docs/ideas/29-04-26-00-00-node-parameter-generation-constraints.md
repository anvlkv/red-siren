# Node Parameter Generation Under Monotonic Constraints

**Date:** 29-04-26 00:00  
**Problem:** Move node physical-parameter generation out of `Config::try_from(Layout)` and into `NodeConfig`, improve test-node generation, and redesign the model so it can always satisfy these constraints:

1. density increases with frequency
2. weight decreases with frequency
3. `l_mm` is derived from volume
4. room size is computed from volume, with a minimum of `15 m^3`
5. `hr_bpm` increases with frequency

---

## Questions Asked

1. Implement or ideate: ideation only for now.
2. Room size representation: propose only for now.
3. Strict priorities: density up with frequency, `hr_bpm` up with frequency, `l_mm` from volume, room size from volume with `15 m^3` minimum.
4. Test nodes: mirror production-derived values.
5. Modeling style: show both physically-plausible and deterministic monotonic approaches.

---

## Problem Framing

The current split is backwards for the requested direction:

- `NodeConfig` in [crates/common/src/instrument/config/node.rs](../../crates/common/src/instrument/config/node.rs) is mostly a data holder with derived accessors.
- `Config::try_from(Layout)` in [crates/common/src/instrument/config.rs](../../crates/common/src/instrument/config.rs) computes `w_kg`, `v_cm3`, and injects `l_mm` from layout progression.
- `new_test_node(f)` in [crates/common/src/instrument/config/node.rs](../../crates/common/src/instrument/config/node.rs) uses fixed placeholder physical values rather than the production model.
- The audio layer already treats node volume as a room-size input: `room_size_m3 = node_config.v_m3()` in [crates/audio-system/src/system/node.rs](../../crates/audio-system/src/system/node.rs).

That leads to four mismatches with your target constraints:

1. `w_kg` currently increases with frequency, not decreases.
2. `hr_bpm()` currently depends on mass as $241 \times w_{kg}^{-0.25}$, so if mass increases, heart rate decreases.
3. `l_mm` currently comes from screen/layout spacing, not from volume.
4. Room size is currently just `v_m3()`, which is far below a realistic minimum room volume and has no lower clamp.

The central design decision is therefore not just “move formulas to `NodeConfig`”, but “define one authoritative node-parameter model that all generation paths use”.

---

## Codebase Inspiration

### 1. Current generation ownership

In [crates/common/src/instrument/config.rs](../../crates/common/src/instrument/config.rs), the layout loop computes:

- `frequency`
- `phase`
- `cents`
- normalized frequency progress `t`
- `w_kg` from a geometric interpolation
- `body_density`
- `v_cm3 = mass / density`
- `l_mm` from a running spatial cursor

This is the current implementation anchor for the move.

### 2. Current downstream assumptions

`NodeConfig` is consumed as already-computed physical data. The clearest downstream example is [crates/audio-system/src/system/node.rs](../../crates/audio-system/src/system/node.rs):

- `room_size_m3 = node_config.v_m3()`
- `damping_time = 1.0 / (node_config.hr_bpm() as f64 / 60.0)`

That means any redesign will change sound behavior, not just snapshots.

### 3. Snapshot evidence

The snapshots in [crates/common/src/instrument/config/snapshots/common__instrument__config__node__tests__config_360x800.snap](../../crates/common/src/instrument/config/snapshots/common__instrument__config__node__tests__config_360x800.snap) show the present monotonic direction clearly:

- low-frequency node: `w_kg = 0.02`, `hr = 641`
- high-frequency node: `w_kg = 150000`, `hr = 13`

This is the exact opposite of your target for weight and heart rate.

### 4. Test helper weakness

`new_test_node(f)` currently hard-codes:

- `l_mm = 170.0`
- `w_kg = 1.1`
- `v_cm3 = 10.0`

So test nodes are not representative of production nodes, and they cannot validate monotonic constraints.

---

## Web Inspiration

### 1. Allometric scaling

From the allometry references:

- power-law mappings are the standard way to keep cross-scale behavior monotonic and tunable
- many biological rates scale as $y = kx^a$
- heart rate often decreases with mass, roughly with exponent $-1/4$

Implication: if you want `hr_bpm` to increase with frequency while weight decreases with frequency, it is cleaner to define heart rate directly from frequency, not indirectly from weight.

Reference:
- https://en.wikipedia.org/wiki/Allometry

### 2. Density fundamentals

Density is simply $\rho = m / V$.

Implication: if density must increase while weight decreases, then volume must decrease fast enough to keep density increasing. That gives a stable construction:

- choose decreasing mass from frequency
- choose increasing density from frequency
- derive volume as $V = m / \rho$

Reference:
- https://en.wikipedia.org/wiki/Density

### 3. Formants and resonant spaces

Formants are resonant peaks tied to resonator length/shape, and rooms can also be treated as resonant spaces.

Implication: deriving `l_mm` from volume is acoustically coherent if `l_mm` becomes an effective resonator length rather than a literal screen distance.

Reference:
- https://en.wikipedia.org/wiki/Formant

### 4. Room volume and reverb

Reverberation time is coupled to room volume, and room-size parameters are usually handled in cubic meters with meaningful lower bounds.

Implication: `room_size_m3` should be a separate derived property from node body volume, not raw `v_m3()`.

Reference:
- https://en.wikipedia.org/wiki/Reverberation

---

## Idea Options

## Idea 1: Deterministic Monotonic Model

**Concept**

Create a single `NodePhysicalModel` in `node.rs` driven by normalized frequency progress $t \in [0,1]$. Every physical property is derived from monotonic closed-form functions of `t`.

Suggested construction:

- density increases with `t`
- weight decreases with `t`
- volume derives from `weight / density`
- `l_mm` derives from volume via a cube-root length scale
- `hr_bpm` derives directly from frequency or `t`
- room size derives from volume with a floor of `15.0`

Example shape set:

$$
\rho(t)=\rho_{min}\left(\frac{\rho_{max}}{\rho_{min}}\right)^t
$$

$$
w(t)=w_{max}\left(\frac{w_{min}}{w_{max}}\right)^t
$$

$$
V(t)=\frac{1000\,w(t)}{\rho(t)}
$$

$$
l_{mm}(t)=k \cdot V(t)^{1/3}
$$

$$
hr(t)=hr_{min} + (hr_{max}-hr_{min}) \cdot s(t)
$$

where `s(t)` can be linear or smoothstep.

**Fit**

- Satisfies all five constraints by construction.
- Makes `NodeConfig::from_generated(...)` the single authority.
- Makes test nodes trivial to mirror from production.

**Feasibility**

High.

**Risks**

- Least physically faithful.
- `l_mm` will stop reflecting screen geometry, so formant ranges will shift.
- Existing snapshots and some sound behavior will change sharply.

**Effort**

Low.

---

## Idea 2: Hybrid Physical Model

**Concept**

Keep the same architectural move into `NodeConfig`, but use a more physically-legible chain:

1. derive decreasing body mass from frequency using inverse power law
2. derive increasing material density from frequency using direct power law
3. derive body volume from mass and density
4. derive `l_mm` from equivalent spherical or tubular volume
5. derive room size from body volume with a perceptual scale and `15 m^3` floor
6. derive `hr_bpm` directly from frequency, not mass

Example:

$$
w(f)=w_{ref}\left(\frac{f}{f_{ref}}\right)^{-\alpha}, \quad \alpha > 0
$$

$$
\rho(f)=\rho_{ref}\left(\frac{f}{f_{ref}}\right)^\beta, \quad \beta > 0
$$

$$
V(f)=\frac{1000\,w(f)}{\rho(f)}
$$

For `l_mm`, treat the node as an equivalent sphere or tube:

$$
r \propto V^{1/3}, \quad l_{mm} = c_l \cdot V^{1/3}
$$

For room size:

$$
room\_size\_m3 = \max(15.0, c_r \cdot V_m3^{\gamma})
$$

with $0 < \gamma < 1$ to compress the scale.

**Fit**

- Satisfies all five constraints.
- Preserves a physically readable story.
- Keeps formulas tunable through a few exponents and reference constants.

**Feasibility**

High.

**Risks**

- More tuning work than Idea 1.
- If `l_mm` comes entirely from volume, formants may no longer correlate with screen position or layout sweep.
- Needs explicit tests to lock monotonic behavior.

**Effort**

Medium.

---

## Idea 3: Split Acoustic Length From Spatial Layout

**Concept**

Keep your requested `l_mm` as an acoustic length derived from volume, but stop overloading it as a proxy for spatial placement. If layout still matters visually or musically, preserve that separately in generation context rather than inside `l_mm`.

Architecture:

- `Config` computes only layout-bound values: key, phase, cents, frequency, and any spatial index
- `NodeConfig::generate_physics(...)` computes `w_kg`, `v_cm3`, `l_mm`, `hr_bpm`-compatible state, room-size helper
- if needed later, a separate field or transient generation context carries screen-distance information

This is the cleanest way to satisfy `l_mm`-from-volume without losing all layout semantics.

**Fit**

- Best architectural separation.
- Avoids conflating physical resonator length with screen distance.
- Makes the model easier to reason about and test.

**Feasibility**

High, but slightly broader than a formula-only change.

**Risks**

- Might require minor downstream code changes if anything implicitly treated `l_mm` as a spatial measure.
- Snapshot churn will be larger.

**Effort**

Medium.

---

## Idea 4: Two-Tier Test Helper Strategy

**Concept**

Redesign test node creation around the same production generator, then layer explicit override helpers on top.

Recommended helpers:

- `NodeConfig::new_test_node(frequency)`
  - delegates to the same generation model as production
  - auto-generates key, phase, cents, and all physical properties consistently
- `NodeConfig::new_test_node_with(...)`
  - builder or override-based helper for tests that need exact physical values

This is not a full standalone generation model; it is the supporting strategy that should accompany any of Ideas 1 to 3.

**Fit**

- Directly solves your item 2.
- Eliminates the current fake physical defaults.
- Makes monotonic tests meaningful.

**Feasibility**

Very high.

**Risks**

- Some tests may need to become more explicit if they previously relied on fixed dummy values.

**Effort**

Low.

---

## Idea Comparison

| Idea | Main Strength | Main Weakness | Constraint Fit | Effort |
| --- | --- | --- | --- | --- |
| 1. Deterministic monotonic model | Simplest way to guarantee all rules | Least physically plausible | Excellent | Low |
| 2. Hybrid physical model | Best balance of plausibility and control | Needs tuning | Excellent | Medium |
| 3. Split acoustic length from spatial layout | Cleanest architecture | Slightly broader refactor | Excellent | Medium |
| 4. Two-tier test helper strategy | Fixes test realism immediately | Depends on another model choice | Supporting change | Low |

---

## Recommendation

**Strongest option:** combine **Idea 2 + Idea 3 + Idea 4**.

That gives the cleanest long-term design:

1. move all physical generation into `NodeConfig`
2. define a small `NodePhysicalModel` API in `node.rs`
3. make `Config::try_from(Layout)` pass only generation inputs, not hard-coded physics outputs
4. treat `l_mm` as acoustic length derived from volume, not as layout distance
5. add a dedicated derived method for room size, for example `room_size_m3()`, instead of reusing raw `v_m3()`
6. make `new_test_node(f)` call the same generator as production

### Why this is the best fit

- It satisfies every requested monotonic rule.
- It fixes the architectural ownership problem, not just the formulas.
- It keeps a plausible physical story without forcing you into biologically inconsistent `hr_bpm` derivation from mass.
- It gives the audio system a cleaner abstraction: body volume and room volume are not the same thing.

### Concrete design rule set

Use one canonical order:

1. normalize frequency into `t`
2. derive increasing density from `t`
3. derive decreasing weight from `t`
4. derive volume from density and weight
5. derive `l_mm` from volume
6. derive room size from volume with `max(15.0, ...)`
7. derive `hr_bpm` directly from frequency or `t`

### Quick validation plan

1. Add monotonic tests over sorted frequency for density, inverse weight, `l_mm`, room size, and `hr_bpm`.
2. Snapshot a few canonical layouts before and after.
3. In audio, compare old `room_size_m3 = v_m3()` against new compressed room-size mapping to avoid giant sonic jumps.
4. Keep test helpers split into “production-derived” and “explicit override” styles.

---

## References

### Workspace

- [crates/common/src/instrument/config.rs](../../crates/common/src/instrument/config.rs)
- [crates/common/src/instrument/config/node.rs](../../crates/common/src/instrument/config/node.rs)
- [crates/audio-system/src/system/node.rs](../../crates/audio-system/src/system/node.rs)
- [crates/common/src/instrument/config/snapshots/common__instrument__config__node__tests__config_360x800.snap](../../crates/common/src/instrument/config/snapshots/common__instrument__config__node__tests__config_360x800.snap)
- [docs/ideas/25-04-26-12-00-config-node-generation.md](../25-04-26-12-00-config-node-generation.md)

### Web

- https://en.wikipedia.org/wiki/Allometry
- https://en.wikipedia.org/wiki/Density
- https://en.wikipedia.org/wiki/Formant
- https://en.wikipedia.org/wiki/Reverberation
