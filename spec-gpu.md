# Stage Perimetry ↔ Lake Deformation Spec (GPU Implementation)

## Goal

Design a GPU-first deformation architecture for:

1. Chunked perimeter updates (no full mesh rewrite each edit)
2. Interpolating deformation from outer perimeter toward inner rings in the vertex shader
3. Efficient theta-ray addressing for CPU→GPU upload paths
4. Batched stage-perimetry writes with minimal invalidation
5. Optional chunked **Y-only theta-ray override** from an external source passed to `Lake`

This document is the GPU counterpart to `spec.md` and is intended for high-vertex-count scalability.

---

## Current constraints and context

- Rendering uses `@react-three/fiber` with `frameloop="demand"`.
- Stage perimeter is mutable through imperative geometry updates.
- Lake must follow perimeter changes without remounting.
- Future edits require batched perimeter mutations with immediate visual feedback.
- Lake requires optional fast path to copy **Y only** from external source data, in theta-ray chunks.

---

## Proposed architecture (GPU-first)

## 1) PerimetryStore remains CPU source of truth

Keep the same stable, imperative store shape as in CPU spec:

- `base: Float32Array` — immutable base perimeter (`thetaCount` points, xyz packed)
- `current: Float32Array` — mutable deformed perimeter (xyz packed)
- `thetaCount: number`
- `version: number`
- `listeners: Set<(ranges, version) => void>`

`transact()` semantics stay identical:

1. Apply all writes
2. Merge dirty theta ranges
3. One stage `position.needsUpdate = true`
4. One `invalidate()`
5. Increment `version`
6. Notify subscribers once

GPU path consumes these dirty ranges for texture upload rather than per-vertex CPU lake writes.

---

## 2) GPU data model

Use textures (or buffer textures where available) as deformation inputs:

### Required textures

1. **Perimeter texture** (`perimeterTex`)
   - Size: `thetaCount x 1`
   - Format: RGBA float (store xyz in rgb)
   - Updated only for dirty theta ranges

2. **Inner anchor texture** (`innerAnchorTex`)
   - Size: `thetaCount x 1`
   - Format: RGBA float (store xyz)
   - Usually static unless topology/base anchor changes

3. **Radial weight texture** (`radialWeightTex`) or uniform array
   - Size: `radialCount x 1`
   - Format: R float
   - Static unless `phiSegments` / interpolation curve changes

### Optional texture

4. **Source Y texture** (`sourceYTex`)
   - Size: `thetaCount x radialCount`
   - Format: R float (or RGBA float with Y in R)
   - Used only when Y override is enabled
   - Updated in theta chunks from external ref

---

## 3) Lake mesh topology + shader attributes

Prefer stable, custom `BufferGeometry` with explicit integer-like indices:

- Per-vertex attributes:
  - `aTheta` in `[0, thetaCount - 1]`
  - `aRadial` in `[0, radialCount - 1]`
  - optional base position if needed for fallback/debug

These attributes allow direct shader sampling without CPU-side deformation loops.

### Why this layout

- CPU only updates narrow texture regions for dirty theta ranges
- Vertex shader computes final position per vertex
- No large CPU `position` buffer rewrites during deformation

---

## 4) Shader deformation model

For each vertex:

1. `outer = sample(perimeterTex, theta)`
2. `inner = sample(innerAnchorTex, theta)`
3. `w = sample(radialWeightTex, radial)` (or computed falloff)
4. `pos = inner + (outer - inner) * w`
5. If `useSourceY`: `pos.y = sample(sourceYTex, theta, radial)`

This preserves CPU-generated X/Z logic conceptually while moving interpolation to GPU.

### Conceptual GLSL

```glsl
vec3 outer = texelFetch(perimeterTex, ivec2(iTheta, 0), 0).xyz;
vec3 inner = texelFetch(innerAnchorTex, ivec2(iTheta, 0), 0).xyz;
float w = texelFetch(radialWeightTex, ivec2(iRadial, 0), 0).x;

vec3 p = mix(inner, outer, w);

if (uUseSourceY > 0.5) {
  float y = texelFetch(sourceYTex, ivec2(iTheta, iRadial), 0).x;
  p.y = y;
}

transformed = p;
```

---

## 5) Dirty-range upload pipeline (CPU → GPU)

Lake subscribes to `PerimetryStore` dirty ranges and queues them.

Per frame (`useFrame`):

- Process up to `uploadRayBudgetPerFrame` theta rays
- Upload only modified texels in `perimeterTex`
- Mark texture `needsUpdate` once per chunk
- If queue non-empty, call `invalidate()`

### Upload strategy

- Maintain a CPU-side float buffer mirroring `perimeterTex`
- For each dirty theta `i`, write `current[i]` into CPU texture buffer
- Perform partial upload when supported; otherwise upload whole texture but with bounded enqueue processing
- Batch all changes in current frame before a single `needsUpdate`

---

## 6) Chunked Y-only copy pipeline (GPU version)

`Lake` accepts optional external packed xyz ref and exposes chunk enqueue API.

### API shape

```ts
type PositionRef = React.MutableRefObject<Float32Array | null>; // xyz packed theta-major-compatible

type ThetaRange = { start: number; end: number }; // [start, end), modulo thetaCount

interface LakeProps {
  yCopySourceRef?: PositionRef;
  yCopyRayBudgetPerFrame?: number; // default ~128
}

interface LakeHandle {
  enqueueYCopyRange(start: number, end: number): void;
  enqueueYCopyRanges(ranges: ThetaRange[]): void;
}
```

### Semantics

For each queued theta `i`:

1. Read source xyz ray from `yCopySourceRef.current`
2. Copy only Y into CPU-side `sourceYTex` buffer for that theta/radial span
3. Batch texture update once per processed chunk
4. Keep `uUseSourceY = true` when override should apply
5. If queue remains, call `invalidate()`

### Safety

- If source ref is `null`, skip
- If topology incompatible (`thetaCount`, `radialCount` mismatch), skip and warn once
- No allocations in hot loops
- Preserve shader-calculated X/Z

---

## 7) Data flow

1. External system mutates perimeter through `PerimetryStore.transact(...)`
2. Stage perimeter geometry updates once per transaction
3. Store emits merged dirty theta ranges
4. Lake enqueues ranges for `perimeterTex` updates
5. Optional: caller enqueues Y-copy ranges; Lake writes Y into `sourceYTex` in chunks
6. Vertex shader deforms all lake vertices each draw from textures
7. Visual updates appear without remounting and without per-vertex CPU writes

---

## 8) API and integration changes

### Stage context

- Keep/replace with:
  - `perimetryStore: PerimetryStore`

### Lake internals

- Stable custom geometry (`useMemo`) with `aTheta`, `aRadial`
- `ShaderMaterial` (or `onBeforeCompile`) with deformation uniforms/textures
- Dirty range queue for perimeter texture updates
- Separate Y-copy queue for source Y texture updates
- Demand-loop aware chunk processing + `invalidate()` while queues remain

---

## 9) Performance notes (GPU path)

- CPU work scales with dirty theta count, not full vertex count
- GPU handles interpolation for full mesh in vertex stage
- Avoid reallocating texture buffers; reuse typed arrays
- Batch queue drains and texture `needsUpdate`
- Tune independent budgets:
  - `uploadRayBudgetPerFrame` for perimeter updates
  - `yCopyRayBudgetPerFrame` for source Y updates

Potential bottlenecks:

- Full-texture uploads on platforms lacking efficient partial updates
- Float texture support constraints on low-end/mobile GPUs

Mitigations:

- Capability checks + fallback material/path
- Coarser budgets or reduced topology on constrained devices

---

## 10) Rollout plan (GPU)

### Phase 1 — Preserve CPU store contract

Deliverables:

- Keep `PerimetryStore` + transaction batching semantics
- Ensure clean dirty-range notifications for consumers

### Phase 2 — Shader-ready lake geometry

Deliverables:

- Introduce stable custom lake geometry with `aTheta`/`aRadial`
- Keep existing visual parity (initially with static textures)

### Phase 3 — Perimeter texture deformation

Deliverables:

- Add `perimeterTex`, `innerAnchorTex`, `radialWeightTex`
- Deform lake fully in vertex shader
- Wire dirty-range queue to texture updates

### Phase 4 — Y-only GPU override path

Deliverables:

- Add `sourceYTex` + queue APIs `enqueueYCopyRange(s)`
- Implement chunked Y uploads from external ref
- Add compatibility checks and warn-once guards

### Phase 5 — Tuning + fallback hardening

Deliverables:

- Budget tuning and frame-time profiling
- Fallback to CPU path if required capabilities unavailable
- Document behavior and limits

---

## Acceptance criteria (GPU)

- Stage perimeter mutation remains batched with single visible update cycle.
- Lake deformation no longer requires per-vertex CPU writes each update.
- Perimeter changes propagate through chunked texture uploads.
- Inner-ring interpolation is performed on GPU and matches expected falloff.
- Y-only override can be applied from external ref in theta chunks without altering X/Z.
- Demand-loop rendering remains responsive with bounded per-frame upload work.

---

## Capability checks and fallback

At runtime, verify support for required texture formats and vertex texture sampling.

If unsupported:

- Log concise capability warning
- Fall back to CPU deformation path (`spec.md` architecture)
- Keep external API (`PerimetryStore`, enqueue methods) unchanged where possible

This preserves behavior while allowing GPU acceleration where available.
