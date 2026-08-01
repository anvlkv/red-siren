# Unified Migration Spec: Prop-Driven Lake/Stage API + GPU Deformation

Date: 2026-07-18  
Status: **Migration plan only** (no implementation in this document)

---

## Goal

Unify all discussed changes into one migration target:

1. Move to **prop-driven deformation inputs** (no imperative Lake API).
2. Keep Stage perimeter mutation batched and efficient.
3. Support GPU-first lake deformation while preserving deterministic CPU-side query access.
4. Expose reliable access to lake deformation data at specific rays:
   - per-theta **radius offsets**
   - per-theta/per-radial **Y values**

---

## Locked decisions

### API/Input semantics

1. Y batch payloads are full center→perimetry profiles along each ray.
2. Y writes are **delta Y** (additive).
3. Batch addressing is contiguous theta spans.
4. Stage XZ input is radius-only.
5. Stage radius input is **delta from base radius**.
6. Batches may wrap across seam.
7. Y payload data type: `Float32Array`.
8. Remove imperative Lake API entirely.
9. Remove legacy `yCopySourceRef` path entirely (no backward compat).

### Validation/error policy

10. Duplicate batch IDs: **throw**.
11. Invalid payload/topology: **throw**.
12. Errors: **propagate**.

### Query/read semantics

13. Local-space query values are sufficient.
14. Mutable view return for ray data is acceptable.
15. Simpler query behavior preferred: queries read from canonical CPU mirrors (including newly ingested updates even before GPU upload completes).

---

## High-level architecture

Use a dual-layer model:

- **Canonical CPU deformation state** (authoritative for reads + upload source)
- **GPU textures/shader deformation** (authoritative for rendering performance)

This avoids expensive GPU readback while keeping rendering scalable.

---

## Data model

## Stage perimetry (existing store retained)

`PerimetryStore` remains CPU source of truth for stage perimeter points and dirty-range notifications.

- `thetaCount`
- `base` perimeter xyz (`thetaCount * 3`)
- `current` perimeter xyz (`thetaCount * 3`)
- transactional batched writes + merged dirty ranges

## Lake canonical deformation mirrors (new)

1. `radiusDeltaByTheta: Float32Array(thetaCount)`
2. `yDeltaByThetaRadial: Float32Array(thetaCount * radialCount)`

Where:
- `radialCount = phiSegments + 1`
- layout for Y mirror: ray-major contiguous (`theta * radialCount + radial`)

These arrays are the query source and the upload source for GPU textures.

---

## Prop-driven API shape

## Batch types

```ts
export interface RadiusDeltaBatch {
  id: number;                 // unique, strictly increasing within radius channel
  start: number;              // modulo thetaCount
  deltaRadius: Float32Array;  // contiguous rays, one value per ray
}

export interface YDeltaBatch {
  id: number;          // unique, strictly increasing within Y channel
  start: number;       // modulo thetaCount
  deltaY: Float32Array; // flattened contiguous rays, full radial profile
                        // length = rayCount * radialCount
}
```

## Component-level intent

- Stage ingest path receives `radiusDeltaBatches`.
- Lake ingest path receives `yDeltaBatches`.
- No imperative `ref` enqueue APIs.

Exact placement (Stage vs World prop boundary) may vary, but behavior and contracts are fixed.

---

## Processing semantics

## A) Stage radius-delta ingestion

For each unseen `RadiusDeltaBatch`:

1. Validate (`id`, finite values, non-empty payload).
2. For each `k`:
   - `theta = (start + k) mod thetaCount`
   - read base point/direction at theta
   - compute `targetRadius = baseRadius + deltaRadius[k]`
   - write perimeter X/Z from direction * targetRadius
   - preserve expected Y policy from base perimeter
3. Commit in one `perimetryStore.transact(...)`.
4. Store emits merged dirty ranges once.

## B) Lake interpolation channel (XZ)

- Subscribe to `PerimetryStore` dirty ranges.
- Queue interpolation work by theta ranges.
- Per frame, process bounded rays.
- Interpolate X/Z inner→outer with smoothstep radial weights.
- Leave Y untouched in this channel.

## C) Lake Y-delta channel

For each unseen `YDeltaBatch`:

1. Validate shape/topology:
   - `deltaY.length % radialCount === 0`
   - derived `rayCount > 0`
   - finite values only
2. Apply into canonical `yDeltaByThetaRadial` mirror (additive at addressed indices).
3. Queue affected theta span for GPU texture upload.

Per frame, drain queued Y upload rays in bounded slices.

---

## GPU deformation model

## Required dynamic textures

1. `radiusDeltaTex` — size `thetaCount x 1`, float channel(s)
2. `yDeltaTex` — size `thetaCount x radialCount`, float channel(s)

## Required static inputs

- base inner anchors / base ray direction info
- radial weights (`smoothstep` default)
- topology uniforms (`thetaCount`, `radialCount`)

## Vertex shader intent

Per vertex (theta, radial):

1. sample radius delta at theta
2. derive outer XZ from base direction and `(baseRadius + delta)`
3. interpolate XZ using radial weight
4. sample Y delta at (theta, radial)
5. final local position uses interpolated XZ + additive Y delta

Channels remain orthogonal by construction.

---

## Upload queues + frame budget

Maintain independent queues/cursors:

- interpolation XZ queue (dirty theta ranges)
- radius texture upload queue
- Y texture upload queue

Use shared per-frame slice size by default (can split later if needed).

Frame rules:

- batch writes per queue slice
- set texture/attribute update flags once per frame per target
- `invalidate()` while any queue still has work

---

## Query API (lake deformation reads)

Expose local-space reads from canonical CPU mirrors:

```ts
interface LakeDeformationReadApi {
  getRadiusOffset(theta: number): number;
  getYAt(theta: number, radial: number): number;
  getYRay(theta: number): Float32Array; // mutable view accepted
}
```

Behavior:

- queries reflect ingested CPU state immediately (including work not yet uploaded to GPU), which is the simplest deterministic model.

---

## Topology-change behavior

On `segments/thetaCount`, `phiSegments/radialCount`, or relevant geometry topology changes:

1. Recreate stage store/geometry + lake textures/mirrors as needed.
2. Clear all queues.
3. Reinitialize mirrors to baseline.
4. Enqueue full refresh work once.
5. Invalidate for visible refresh.
6. Any stale incoming batch for old topology must throw.

---

## Validation and failure policy

Fail-fast with thrown propagated errors for:

- duplicate or non-monotonic IDs (within a channel)
- malformed payload lengths
- empty invalid payloads
- non-finite numeric values
- topology mismatches

No warn-and-skip behavior for contract violations.

---

## File-level migration targets

- `src/components/World/Lake.types.ts`
  - remove legacy imperative/ref types
  - add `YDeltaBatch`, new props, read API types

- `src/components/World/Lake.queue.ts`
  - keep range queue for interpolation
  - add payload/upload queue helpers for Y and radius texture updates

- `src/components/World/Lake.tsx`
  - remove `forwardRef`/imperative handle/yCopySourceRef
  - ingest `yDeltaBatches`
  - maintain CPU mirrors
  - drive GPU upload queues
  - keep bounded frame processing

- `src/components/Stage.perimetry.ts`
  - retain transactional store semantics
  - add helper for `RadiusDeltaBatch` application

- `src/components/Stage.tsx`
  - ingest `radiusDeltaBatches`
  - enforce strict ID and payload validation

- `src/components/World/World.tsx` (and producers)
  - migrate callers to prop-driven batch arrays
  - remove imperative lake update flows

- shader/material module(s) (new or refactored)
  - add texture uniforms + vertex deformation logic

---

## Phased rollout

### Phase 1 — API migration (prop-driven, CPU path parity)

- Introduce batch types and strict validation.
- Remove imperative Lake API and legacy Y-copy path.
- Keep rendering behavior functionally equivalent.

### Phase 2 — Canonical CPU mirrors + query API

- Add `radiusDeltaByTheta` and `yDeltaByThetaRadial`.
- Implement local-space read methods.
- Ensure reads are deterministic under demand loop.

### Phase 3 — GPU deformation scaffold

- Add shader-ready geometry attributes (`theta`, `radial`) if needed.
- Wire static uniforms/weights/anchors.

### Phase 4 — Dynamic texture uploads

- Add `radiusDeltaTex` and `yDeltaTex`.
- Implement bounded dirty-range uploads.
- Keep frame cost controlled with queue slices.

### Phase 5 — Hardening + fallback

- Capability checks for float textures/vertex sampling.
- Fallback path where necessary while preserving public API.
- Profile and tune budgets.

---

## Acceptance criteria

1. Stage accepts contiguous wrapped radius-delta batches and updates perimeter correctly.
2. Lake accepts contiguous wrapped full-profile Y-delta batches via props.
3. No imperative lake update API remains.
4. XZ and Y channels remain orthogonal.
5. Local read API returns radius offsets and Y data at requested rays.
6. Reads are available from canonical CPU state without GPU readback.
7. GPU deformation path renders from texture-driven deltas with bounded per-frame upload work.
8. Invalid inputs fail fast with thrown propagated errors.

---

## Notes

- This unified spec supersedes the old imperative Y-copy assumptions in `spec-gpu.md`.
- Keep implementation surgical and incremental; preserve transaction/dirty-range principles already established.
