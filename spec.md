# Stage Perimetry ↔ Lake Deformation Spec

## Goal

Design an efficient deformation architecture for:

1. Chunked lake updates (not full-ring every time)
2. Interpolating deformation from outer perimeter toward inner rings
3. Efficient read/write access by **theta rays**
4. Batched stage-perimetry writes with minimal invalidation
5. Chunked **Y-only theta-ray writes** in `Lake`, copied from an external ref passed to `Lake`

---

## Decision log (locked)

These decisions are now part of the spec:

1. **Theta model:** logical rays are `segments` (no seam duplication in store).
2. **Topology change behavior:** on topology change, recreate store + lake geometry, clear queues, and mark full-theta dirty once.
3. **Stage context:** keep `setStageSegments` in context.
4. **Interpolation falloff default:** `smoothstep`.
5. **Interpolation/Y behavior:** interpolation writes **X/Z only**, Y comes from optional source ref pipeline.
6. **Lake imperative API:** expose `LakeHandle` via `forwardRef`.
7. **Y-copy mismatch warnings:** warn on every incompatible copy attempt (no one-time suppression).
8. **Chunk control naming:** one shared per-frame **slice size** (not separate budgets).
9. **Minimum chambers/segments:** initialize `nChambers` to 3 and clamp stage segments to at least 3.
10. **Slice windows:** interpolation (XZ) and Y-copy use independent `start/end` ranges and independent queue cursors; they are not assumed to match.

---

## Current code anchors

- Stage context + perimetry accessors: [`src/components/Stage.tsx`](src/components/Stage.tsx)
- Lake geometry: [`src/components/World/Lake.tsx`](src/components/World/Lake.tsx)
- Dynamic segment driver (`nChambers -> setStageSegments`): [`src/components/World/World.tsx`](src/components/World/World.tsx)
- Demand frameloop source: [`src/App/App.tsx`](src/App/App.tsx)

---

## Current constraints and context

- Rendering uses `@react-three/fiber` with `frameloop="demand"`.
- Stage perimeter is mutable through imperative geometry updates.
- Lake currently uses `THREE.RingGeometry`, but needs to follow perimeter changes without remounting.
- Future edits require mutating perimeter points in batches and seeing updates immediately.
- Lake also needs an optional fast path to copy **Y coordinates only** from an external source ref, in theta-ray chunks.
- **No backwards compatibility required** with `stagePerimetryPointsGetter/Setter`.

---

## Proposed architecture

## 1) `PerimetryStore` (single source of truth)

Create a stable, imperative store inside `Stage` (no React state for geometry data):

- `thetaCount: number` — logical rays, equal to `segments`
- `base: Float32Array` — immutable logical perimeter (`thetaCount * 3`, xyz packed)
- `current: Float32Array` — mutable deformed logical perimeter (`thetaCount * 3`)
- `version: number` — increments once per committed transaction
- `listeners: Set<(ranges, version) => void>`

Expose via context:

```ts
type ThetaRange = { start: number; end: number }; // [start, end), modulo thetaCount

interface PerimetryStore {
  thetaCount: number;

  getPoint(i: number): Float32Array;      // view of current xyz at theta i
  getBasePoint(i: number): Float32Array;  // view of base xyz at theta i

  transact(fn: (tx: PerimetryTx) => void): void;
  subscribe(fn: (dirty: ThetaRange[], version: number) => void): () => void;
}

interface PerimetryTx {
  setPoint(i: number, x: number, y: number, z: number): void;
  setRange(
    start: number,
    end: number,
    f: (i: number, base: Float32Array) => [number, number, number]
  ): void;
}
```

### Stage geometry seam rule

`THREE.CircleGeometry` perimeter has a duplicated seam vertex. Store does **not**.

- Logical `i` is `0..thetaCount-1`
- Stage perimeter write index is `i + 1`
- Duplicated seam vertex mirrors `i=0` after transaction commit

### Transaction semantics

`transact()` must:

1. Apply all point/range writes to `current`
2. Collect dirty theta indices/ranges
3. Merge contiguous ranges
4. Flush logical values into stage geometry perimeter buffer (including seam mirror)
5. Do **one** stage `position.needsUpdate = true`
6. Do **one** `invalidate()`
7. Increment `version`
8. Notify subscribers once with merged dirty ranges

This provides batched stage updates and avoids per-point invalidation.

---

## 2) Lake deformation pipeline (dirty-range queue + shared slice size)

Lake subscribes to `PerimetryStore` and queues dirty theta ranges.

Per frame (`useFrame`), process queued work using **one shared numeric** `raySliceSizePerFrame`, but with **independent slice windows** per pipeline:

- interpolation queue for XZ (`start/end` for interpolation)
- Y-copy queue (`start/end` for Y)

These windows/cursors are independent and may refer to different theta spans in the same frame.

For each processed interpolation theta `i`:

- apply interpolation to **X/Z only**

For each processed Y-copy theta `i`:

- copy **Y only** from source ref

After frame chunk:

- Set `lakePosition.needsUpdate = true` once if any ray changed
- If queue still non-empty, call `invalidate()` (required with demand loop)

This guarantees bounded frame cost with single slice-size control.

---

## 3) Theta-ray memory layout (required)

### Custom theta-major ring geometry

Use custom ring geometry with vertex order:

- `i = theta index` (`0..thetaCount`) where `thetaCount` seam row duplicates `0`
- `j = radial index` (`0..phiSegments`)
- `vertexIndex = i * radialCount + j`, `radialCount = phiSegments + 1`

Then one theta ray is contiguous in memory:

- `rayStart = i * radialCount * 3`
- `ray = pos.subarray(rayStart, rayStart + radialCount * 3)`

Benefits:

- Fast contiguous ray read/write
- Natural fit for chunked dirty theta processing
- Natural fit for chunked Y-only copy processing
- Better cache locality than strided writes

---

## 4) Interpolation model (X/Z only, smoothstep default)

For each interpolation-dirty theta `i`:

- `outer = perimetry.current[i]`
- `inner = lakeInnerAnchor[i]` (precomputed from base ring)
- For each radial `j`:
  - `t = j / phiSegments`
  - `w = smoothstep(t)` (default)
  - interpolate `x/z` from `inner -> outer`
  - leave `y` untouched (Y pipeline owns Y)

### Precompute for efficiency

- `radialWeight[j]` as `Float32Array` (`smoothstep` values)
- optional `innerAnchor[i*3..i*3+2]` cache
- no allocations inside frame loop

---

## 5) Chunked Y-copy pipeline

`Lake` accepts an optional ref to source positions for Y-only writes.

### API shape (`Lake`)

```ts
type PositionRef = React.MutableRefObject<Float32Array | null>; // xyz packed, theta-major-compatible

interface LakeProps {
  baseline: number;
  innerRadius: number;
  phiSegments: number;
  yCopySourceRef?: PositionRef;
  raySliceSizePerFrame?: number; // shared slice size, default 128
}

interface LakeHandle {
  enqueueYCopyRange(start: number, end: number): void; // [start, end), modulo thetaCount
  enqueueYCopyRanges(ranges: ThetaRange[]): void;
}
```

### Semantics

For each Y-copy-dirty theta `i`:

1. Read source ray from `yCopySourceRef.current`
2. Read destination lake ray
3. Copy Y component per radial index only (`dst[k + 1] = src[k + 1]`)
4. Preserve destination `X/Z`

### Safety rules

- If `yCopySourceRef.current == null`, skip copy.
- If source topology is incompatible (different theta/radial counts), skip and **warn each attempt**.
- No allocations in hot loop.

---

## 6) Topology change behavior

When topology inputs change (`segments`, `phiSegments`, radii):

1. Recreate store and geometry (no backward compatibility required)
2. Clear pending interpolation and Y-copy queues
3. Enqueue full-theta dirty range once so lake fully refreshes
4. `invalidate()` for visible refresh in demand loop

---

## Data flow

1. External system mutates perimeter through `PerimetryStore.transact(...)`
2. Stage perimeter geometry is updated once per transaction
3. Store emits merged dirty theta ranges
4. Lake enqueues interpolation dirty ranges
5. Optional caller enqueues Y-copy ranges via `LakeHandle`
6. Lake frame loop drains interpolation and Y queues in bounded slices (`raySliceSizePerFrame`) with independent start/end windows
7. Mesh updates become visible without remounting

---

## API changes

### Stage context (`src/components/Stage.tsx`)

- Replace `stagePerimetryPointsGetter/Setter` with:
  - `perimetryStore: PerimetryStore`
- Keep existing:
  - `stageSegments`
  - `setStageSegments`

### Lake internals (`src/components/World/Lake.tsx`)

- Stable geometry (`useMemo`, created once per topology params)
- Subscription effect to capture dirty ranges
- `forwardRef` with `LakeHandle` (`enqueueYCopyRange(s)`)
- Per-frame slice processing using shared `raySliceSizePerFrame` value with separate interpolation/Y queue cursors
- Optional `yCopySourceRef` reader for Y-only writes

### World defaults (`src/components/World/World.tsx`)

- Initialize `nChambers` with `3`
- Clamp segment writes to `>= 3` before `setStageSegments`

---

## Performance notes

- Avoid React state for per-vertex data.
- Avoid rebuilding geometry except when topology changes.
- Batch `needsUpdate` and `invalidate` calls.
- Reuse typed arrays and loop-local scalars.
- Keep frame work bounded using shared slice size.
- XZ and Y pipelines can process different theta windows in the same frame (independent `start/end`).

---

## Rollout plan (phased)

### Phase 1 — Store + batched stage writes

Deliverables:

- Implement `PerimetryStore` in [`src/components/Stage.tsx`](src/components/Stage.tsx)
- Route all perimeter mutations through `transact`
- Emit merged dirty theta ranges

### Phase 2 — Lake dirty-range chunk updates

Deliverables:

- Subscribe lake to store in [`src/components/World/Lake.tsx`](src/components/World/Lake.tsx)
- Queue interpolation dirty ranges
- Drain interpolation and Y queues in `useFrame` using shared `raySliceSizePerFrame` and independent `start/end` windows

### Phase 3 — Theta-major custom ring geometry

Deliverables:

- Replace `THREE.RingGeometry` with custom theta-major buffer in [`src/components/World/Lake.tsx`](src/components/World/Lake.tsx)
- Add fast theta-ray accessors returning contiguous subarrays

### Phase 4 — X/Z interpolation + Y-copy + tuning

Deliverables:

- Add smoothstep radial interpolation for X/Z
- Add Y-only theta-chunk copy from `yCopySourceRef`
- Expose `LakeHandle` via `forwardRef`
- Tune `raySliceSizePerFrame` and profile frame-time spikes

---

## Acceptance criteria

- Stage perimeter can be mutated in batches with one visible update cycle.
- Lake updates do not require React re-render or remount.
- Lake updates can be limited to per-frame slices, with independent XZ and Y slice windows.
- Inner rings deform smoothly in X/Z using smoothstep falloff.
- Theta-ray read/write API exists and is efficient (contiguous in theta-major mode).
- `Lake` can copy **Y only** from a passed ref for queued theta chunks, without changing `X/Z`.
- Topology changes trigger clean rebuild + full refresh.
- World segment control remains in context, and startup avoids invalid `< 3` segment topology.

---
