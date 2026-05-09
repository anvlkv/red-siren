# Generating Distinct ADSR Shapes from Node Config

**Date:** 26-04-26 14:30  
**Problem:** Currently all nodes use a generic `AdsrShape::default()`, losing the opportunity to create musically distinct gongs, bells, and flutes from physical properties (mass, volume, vocal-tract length, derived values like formants and buoyancy).

---

## Problem Framing

Red Siren's instrument design encodes physical modeling parameters in each `NodeConfig`:
- `frequency` (Hz)
- `l_mm` (vocal tract length)
- `w_kg` (mass)
- `v_cm3` (volume)
- Derived: `formant_hz(n)`, `body_density_g_cm3()`, `buoyant_force()`, `hr_bpm()`

These properties determine the node's sonic character—but the envelope generator always uses a flat, one-size-fits-all `AdsrShape::default()` (attack: 0.05, decay: 0.10, sustain: 0.7, release: 0.20).

**Goal:** Map physical properties → ADSR parameters to generate resonant gongs, bells, and flutes with coherent sonic identity per node, improving expressiveness and naturalness.

---

## Codebase Inspiration

1. **Current Generation Site:** [node.rs](../../../crates/audio-system/src/system/node.rs#L119) instantiates envelopes:
   ```rust
   let shape = AdsrShape {};
   rhythm_grid_envelope::<S, _>(
       generator::create_node_generator::<S>(&node_config, values),
       shape,
   )
   ```

2. **AdsrShape Structure:** [envelope.rs](../../../crates/audio-system/src/system/grid/envelope.rs#L14-L25)  
   - `attack: f32` — fraction of total duration (0–1)
   - `decay: f32` — fraction of total duration
   - `sustain: f32` — amplitude level (0–1)
   - `release: f32` — fraction of total duration
   - `smoothness: f32` — curve blend (0=linear, 1=smoothstep)

3. **NodeConfig Computed Methods:**  
   - `formant_hz(n)` — resonant frequency (1-based)
   - `body_density_g_cm3()` — weight / volume
   - `buoyant_force(fluid_density)` — volume-based force
   - `hr_bpm()` — allometric scaling: $241 \times w_{kg}^{-0.25}$

4. **Resonance Physics:**  
   Gongs/bells exhibit exponential decay in response to excitation. Longer vocal tracts (low formants) correlate with slower oscillation; higher mass/volume → longer time constants.

---

## Web Inspiration

- **Physical Modeling & Synthesis:** Resonant instruments (bells, gongs, plate models) use damped oscillators with time constants proportional to structural mass and volume [Verkhoeven, 2002; Smith, 2010].
- **Spectral Richness:** Lower fundamental frequencies and longer vocal tracts support more harmonic energy; sustain level should correlate with formant count and density.
- **Formant-to-Envelope Mapping:** In vocal/wind synthesis, formant bandwidth inversely relates to resonance sharpness; this hints at `release` and `smoothness` curves.
- **Bell/Gong Models:** Convolution reverbs and physical models (e.g., Modalys, MOSAIC) often tie decay time to modal mass and damping ratio.

---

## Solution Ideas

### **Idea 1: Physical Time-Constant Mapping**

**Concept:**  
Generate `attack`, `decay`, `release` as fractions of a physically-derived time constant. Use mass and volume to compute a characteristic oscillation period, then scale ADSR segments proportionally.

**Pseudocode:**
```rust
fn adsr_from_physics(node: &NodeConfig) -> AdsrShape {
    // Time constant from mass and volume (lower mass/volume → faster)
    let tau = (node.w_kg * node.v_cm3).sqrt() / 1000.0;
    
    // Clamp to reasonable range (0.05–2.0 seconds conceptually)
    let tau_norm = (tau / 0.5).clamp(0.05, 2.0);
    
    // Scale attack: faster for lighter nodes
    let attack = (0.02 / tau_norm).clamp(0.01, 0.2);
    
    // Scale decay: longer for heavier nodes
    let decay = (0.10 * tau_norm).clamp(0.05, 0.3);
    
    // Sustain: higher volume → richer sustain (more resonant)
    let sustain = ((node.v_cm3 / 100.0).ln() / 5.0 + 0.5).clamp(0.3, 0.9);
    
    // Release: heavier/larger → longer decay
    let release = (0.20 * tau_norm).clamp(0.1, 0.5);
    
    AdsrShape {
        attack,
        decay,
        sustain,
        release,
        smoothness: 0.5,  // Rounded curves for resonance
    }
}
```

**Fit:**
- ✅ Directly uses physical properties (mass, volume)
- ✅ Mathematically coherent (time constants are physics-based)
- ✅ Gongs: high mass/volume → slow attack, long decay  
- ✅ Bells: moderate properties → medium attack, medium sustain
- ✅ Flutes: lower mass relative to volume → faster attack

**Feasibility:** High.  
- Single function in `NodeConfig` impl, pure math, no dependencies
- Zero allocation, real-time safe
- Deterministic and reproducible

**Risks:**
- Needs parameter tuning (tau scaling, clamp ranges) to sound musical
- May produce extreme envelopes for edge-case node configs
- Test suite will need snapshot coverage

**Effort:** 1–2 hours (impl + test snapshots)

---

### **Idea 2: Formant-Richness & Harmonic Decay Scaling**

**Concept:**  
Longer vocal tracts produce richer harmonic content (more formants). Use the first formant frequency and formant spacing to modulate `sustain` and `release`, creating a spectral-envelope tie-in.

**Pseudocode:**
```rust
fn adsr_from_formants(node: &NodeConfig) -> AdsrShape {
    // First formant (F1) as proxy for vocal tract length
    let f1 = node.formant_hz(1) as f32;
    
    // Formant spacing density: more formants in shorter tracts
    // Estimate via frequency: lower F1 → more harmonic color
    let formant_density = 1000.0 / (f1 + 100.0);
    
    // Attack inversely correlated with frequency
    let attack = (200.0 / (node.frequency as f32 + 50.0)).clamp(0.01, 0.2);
    
    // Decay tied to vocal tract resonance
    let decay = 0.10 * (1.0 + formant_density * 0.1);
    
    // Sustain: more formants → richer sustain
    let sustain = (0.5 + formant_density * 0.05).clamp(0.4, 0.85);
    
    // Release: longer for lower-frequency nodes (more ringing)
    let release = (0.15 + (f1.ln() / 50.0)).clamp(0.1, 0.4);
    
    AdsrShape {
        attack,
        decay,
        sustain,
        release,
        smoothness: 0.6,  // Smoother curves for resonant tails
    }
}
```

**Fit:**
- ✅ Uses derived properties (formants) that encode vocal-tract physics
- ✅ Gongs/bells: lower F1 (longer tract) → richer sustain, longer release
- ✅ Flutes: higher F1 (shorter tract) → tighter envelope
- ✅ Musically intuitive (harmonic density ↔ sustain)

**Feasibility:** High.  
- Compute formants once during node init, cache if needed
- Pure math, no hidden complexity
- Deterministic

**Risks:**
- Formant calculation assumes vocal-tract model (may not reflect all nodes' actual physics)
- Fine-tuning density scaling requires listening/testing
- Could produce awkward envelopes for very high-frequency nodes

**Effort:** 1.5–2 hours (impl + tuning)

---

### **Idea 3: Density-Based Damping + Allometric Release Curve**

**Concept:**  
Use `body_density_g_cm³` as a damping ratio proxy (denser = more tightly damped), and tie release to the `hr_bpm()` allometric law already implemented. This creates coherent "biological" pacing across the instrument.

**Pseudocode:**
```rust
fn adsr_from_density_and_bpm(node: &NodeConfig) -> AdsrShape {
    // Body density as damping proxy
    let density = node.body_density_g_cm3();
    
    // Clamp to reasonable material range (0.1 – 3.0 g/cm³)
    let density_norm = (density / 1.0).clamp(0.1, 3.0);
    
    // Attack inversely scales with density (denser → faster damping → quicker onset)
    let attack = (0.1 / density_norm).clamp(0.01, 0.15);
    
    // Decay: denser materials damp faster
    let decay = (0.15 / density_norm).clamp(0.05, 0.25);
    
    // Sustain: less dense (more buoyant) → richer sustain
    let sustain = (0.4 + (1.0 / density_norm) * 0.15).clamp(0.3, 0.8);
    
    // Release tied to allometric HR: slower "biological" rhythm → longer release
    let hr = node.hr_bpm() as f32;
    let release = (60.0 / (hr + 20.0) * 0.1).clamp(0.1, 0.4);
    
    // Smoothness correlates with low density (less damped → more ringing)
    let smoothness = (1.0 / density_norm).clamp(0.0, 1.0) * 0.5;
    
    AdsrShape {
        attack,
        decay,
        sustain,
        release,
        smoothness,
    }
}
```

**Fit:**
- ✅ Uses already-implemented computed properties (`body_density`, `hr_bpm`)
- ✅ Coherent "biological" model: mass and volume determine how the node "breathes"
- ✅ Gongs: low density (buoyant) → slow release, high sustain
- ✅ Bells: medium density → balanced envelope
- ✅ Flutes: potentially denser → quicker decay

**Feasibility:** High.  
- All methods already exist on `NodeConfig`
- Pure math, no new dependencies
- Real-time safe

**Risks:**
- `hr_bpm()` is a playful allometric formula (not strictly physical for audio)
- Density can be very small for low-mass nodes; division might produce extreme release values
- May need careful guarding against divide-by-zero or very small densities

**Effort:** 1–1.5 hours (impl + edge-case guarding + tests)

---

### **Idea 4: Hybrid Multi-Parameter Blending**

**Concept:**  
Create a small library of envelope "profiles" (gong, bell, flute archetypes) and interpolate between them based on a multi-parameter distance metric combining frequency, mass, volume, and density.

**Pseudocode:**
```rust
fn adsr_from_archetype_blend(node: &NodeConfig) -> AdsrShape {
    // Define three archetypes
    let gong_profile = AdsrShape {
        attack: 0.02, decay: 0.15, sustain: 0.75, release: 0.35, smoothness: 0.7,
    };
    let bell_profile = AdsrShape {
        attack: 0.08, decay: 0.12, sustain: 0.65, release: 0.22, smoothness: 0.6,
    };
    let flute_profile = AdsrShape {
        attack: 0.15, decay: 0.10, sustain: 0.55, release: 0.15, smoothness: 0.4,
    };
    
    // Compute blend weights based on physical properties
    let freq_norm = (node.frequency / 500.0).clamp(0.1, 5.0);
    let mass_norm = (node.w_kg / 0.1).clamp(0.1, 5.0);
    
    // Gong: low freq, high mass → favor gong_profile
    let gong_weight = (1.0 / freq_norm) * mass_norm;
    
    // Flute: high freq, low mass → favor flute_profile
    let flute_weight = freq_norm / mass_norm;
    
    // Bell: middle-ground
    let bell_weight = 1.0;
    
    // Normalize
    let total = gong_weight + bell_weight + flute_weight;
    let wg = gong_weight / total;
    let wb = bell_weight / total;
    let wf = flute_weight / total;
    
    // Interpolate
    AdsrShape {
        attack: gong_profile.attack * wg + bell_profile.attack * wb + flute_profile.attack * wf,
        decay: gong_profile.decay * wg + bell_profile.decay * wb + flute_profile.decay * wf,
        sustain: gong_profile.sustain * wg + bell_profile.sustain * wb + flute_profile.sustain * wf,
        release: gong_profile.release * wg + bell_profile.release * wb + flute_profile.release * wf,
        smoothness: gong_profile.smoothness * wg + bell_profile.smoothness * wb + flute_profile.smoothness * wf,
    }
}
```

**Fit:**
- ✅ Musically curated profiles (not purely algorithmic)
- ✅ Intuitive: nodes "morph" between gong, bell, flute character
- ✅ Gongs/bells/flutes are represented explicitly
- ✅ Easy to adjust profiles iteratively by ear

**Feasibility:** Medium-High.  
- Requires defining three good profiles (requires listening and tuning)
- Pure math, real-time safe
- Good for iteration and A/B testing

**Risks:**
- Profiles are taste-dependent (may need multiple iterations)
- Weighting formula is somewhat arbitrary
- If profiles don't match aesthetic goal, whole approach falls short

**Effort:** 2–4 hours (defining profiles + weight tuning + listening tests)

---

## Recommendation

**Start with Idea 1 (Physical Time-Constant Mapping)**, then iterate:

1. **Why:** Most direct physics → ADSR mapping; uses foundational properties (mass, volume); lowest cognitive load; easiest to defend and extend.
2. **Validation Plan:**
   - Impl the function in `NodeConfig::adsr_shape()` 
   - Add snapshot tests for a few representative nodes (low-mass flute-like, high-mass gong-like, mid-range bell-like)
   - Listen to the results at typical performance tempos (60–120 BPM)
   - Tune the tau scaling and clamp ranges by ear
   - If results feel musically weak, pivot to **Idea 2** (formant-based) or **Idea 4** (archetype blending)

3. **Quick Wins:** Once Idea 1 is in place, the codebase is ready for Idea 2 or 4 as refinements without major restructuring.

---

## References

- Smith, J. O. (2010). Physical Audio Signal Processing. https://ccrma.stanford.edu/~jos/pasp/
- Verkhoeven, J. (2002). Physical Modeling of Percussion Instruments. PhD Thesis, TU Delft.
- Red Siren [crates/audio-system/README.md](../../../crates/audio-system/README.md) — system architecture and real-time constraints.
- Red Siren [crates/audio-system/src/system/grid/envelope.rs](../../../crates/audio-system/src/system/grid/envelope.rs) — ADSR implementation.
- Wikipedia: [Envelope (music)](https://en.wikipedia.org/wiki/Envelope_(music)) — ADSR history and variants.
