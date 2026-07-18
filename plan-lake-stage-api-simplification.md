# Lake/Stage API Simplification Plan (No Implementation Yet)

Date: 2026-07-18  
Status: Planning only (no runtime implementation changes in this plan)

---

## Objective

Simplify the deformation API so it is intent-driven and prop-driven:

1. **Lake Y updates** are submitted as contiguous ray batches containing full center→perimetry profiles, applied as **delta Y**.
2. **Stage perimeter updates** are submitted as contiguous ray batches of **radius deltas from base radius**.
3. Lake continues to interpolate **XZ only** from inner anchors to outer perimeter.
4. Keep XZ and Y channels orthogonal.

---

## Locked decisions

These are now fixed for this change:

1. Y batch payloads are **full arrays per ray** (all radial points center→perimetry).
2. Y semantics are **delta Y** (additive updates).
3. Batch addressing is **contiguous** rays.
4. Stage XZ input is **radius-only** (not raw XYZ perimeter points).
5. XZ and Y channels remain **orthogonal**.
6. Remove imperative API; switch to **prop-driven** ingestion.
7. Remove old Y-copy/ref path entirely; **no backward compatibility**.
8. Validation policy is **throw and skip** (initial request), then finalized as **throw and propagate**.
9. Radius values are **delta from base radius**.
10. Contiguous batches **may wrap across seam**.
11. Y payload type is `Float32Array`.
12. Duplicate batch IDs should **throw**.
13. Errors should **propagate**.

---

## Final API contract (target)

## 1) Stage radius-delta batches

Add a batch type on Stage/perimetry side:

```ts
export interface RadiusDeltaBatch {
  id: number;                 // unique, increasing in this channel
  start: number;              // theta start; modulo normalized
  deltaRadius: Float32Array;  // one value per contiguous ray
}
```

Semantics:

- `theta = (start + k) mod thetaCount`
- `deltaRadius[k]` is relative to base radius at that theta.
- Seam wrapping is natural via modulo.

## 2) Lake Y-delta batches

Add a batch type on Lake side:

```ts
export interface YDeltaBatch {
  id: number;          // unique, increasing in this channel
  start: number;       // theta start; modulo normalized
  deltaY: Float32Array; // flattened rays: ray-major, full radial profile
}
```

Definitions:

- `radialCount = phiSegments + 1`
- `rayCount = deltaY.length / radialCount` (must be integer > 0)

Apply rule:

- For each addressed ray sample: `dstY += deltaY[...]`
- X/Z unchanged by this pipeline.

## 3) Lake props simplification

`Lake` should be prop-driven only:

- Remove:
  - imperative `LakeHandle`
  - `forwardRef` control path
  - `enqueueYCopyRange(s)`
  - `yCopySourceRef`
- Add/keep:
  - `yDeltaBatches?: readonly YDeltaBatch[]`
  - `raySliceSizePerFrame?: number`

---

## Runtime processing model

## Stage pipeline (radius deltas -> perimeter XZ)

For each unseen `RadiusDeltaBatch`:

1. Validate batch (`id`, finite values, non-empty payload).
2. For each `k` in `deltaRadius`:
   - `theta = (start + k) mod thetaCount`
   - Read base point at `theta`.
   - Compute base radial direction in XZ plane.
   - `targetRadius = baseRadius + deltaRadius[k]`.
   - Write X/Z from direction * `targetRadius`; preserve expected Y policy (base Y).
3. Apply writes in one `perimetryStore.transact(...)`.
4. Store emits merged dirty theta ranges once.

## Lake interpolation pipeline (existing concept retained)

- Subscribe to Stage store dirty ranges.
- Queue interpolation dirty ranges.
- Per frame, process bounded by `raySliceSizePerFrame`.
- Interpolate **X/Z only** using precomputed smoothstep radial weights.
- If theta 0 touched, mirror seam row.

## Lake Y-delta pipeline (new prop-driven path)

For each unseen `YDeltaBatch`:

1. Validate topology/shape:
   - `deltaY.length % radialCount === 0`
   - derived `rayCount > 0`
   - finite numbers
2. Enqueue payload work item with independent cursors.

Per frame:

- Drain Y queue in bounded ray slices (same slice-size control, independent cursor from interpolation queue).
- Apply additive Y only.
- If theta 0 touched, mirror seam row.

Frame commit rules:

- Set `positionAttribute.needsUpdate = true` once if either pipeline changed data.
- Call `invalidate()` only if any queue still has pending work.

---

## Error policy (final)

As finalized by product direction:

- **Throw and propagate** for invalid input.
- No warning-based fallback for these API violations.

Must throw on:

- duplicate or invalid batch IDs,
- malformed payload lengths,
- non-finite values,
- topology mismatch,
- invalid empty payloads.

Replay/rerender idempotency rule:

- Already-applied IDs are considered consumed (ignored if explicitly supported by ingestion strategy), but any **duplicate in unseen stream contract** should throw according to the strict ID policy.
- Unseen suffix should be strictly increasing and unique.

---

## File-by-file implementation plan

## `src/components/World/Lake.types.ts`

- Remove imperative/ref types (`LakeHandle`, `PositionRef`).
- Add `YDeltaBatch` and updated `LakeProps` with `yDeltaBatches`.

## `src/components/World/Lake.queue.ts`

- Keep interpolation `RangeQueue` for XZ dirty ranges.
- Add Y payload queue type for `YDeltaBatch` processing:
  - batch cursor,
  - intra-batch ray cursor,
  - helpers to enqueue/clear/process bounded rays.

## `src/components/World/Lake.tsx`

- Remove `forwardRef`, `useImperativeHandle`, Y-copy ref logic.
- Add effect to ingest unseen `yDeltaBatches` from props.
- Validate and enqueue Y payload batches.
- Keep interpolation subscription and processing.
- Process interpolation and Y queues each frame with independent cursors.
- Keep seam mirroring and single-update batching behavior.

## `src/components/Stage.perimetry.ts`

- Keep `PerimetryStore` core contract.
- Add helper(s) for applying `RadiusDeltaBatch` via `transact` efficiently.
- Ensure writes remain batched with single invalidate + merged dirty notifications.

## `src/components/Stage.tsx`

- Add prop-driven ingestion of `radiusDeltaBatches`.
- Track applied IDs for strict ordering/uniqueness checks.
- Route valid batches to perimetry update helper.

## `src/components/World/World.tsx` and related call sites

- Remove imperative lake ref usage for Y updates.
- Pass `yDeltaBatches` and `radiusDeltaBatches` as props through the component tree.
- Maintain existing segment clamp behavior (`>= 3`).

---

## Topology-change behavior

Retain existing topology rules:

1. Recreate store/geometry when topology changes.
2. Clear interpolation and Y queues.
3. Enqueue full-theta interpolation refresh once.
4. Invalidate for visible refresh on demand loop.

Additional requirement:

- Validate incoming queued/unapplied batches against the **current** topology and throw on mismatch.

---

## Non-goals for this change

- No backward compatibility for legacy Y-copy/ref API.
- No mixed imperative+prop API.
- No conversion to sparse theta addressing.
- No alteration of orthogonal channel model.

---

## Rollout sequence

1. **Type/API cleanup**: establish new batch types and remove legacy imperative types.
2. **Stage ingestion**: implement radius-delta prop pipeline.
3. **Lake ingestion**: implement Y-delta prop pipeline.
4. **Queue infrastructure**: add payload queue for Y deltas.
5. **Callsite migration**: switch producers to prop-driven batch streams.
6. **Validation hardening**: strict throw/propagate checks for all malformed batches.

---

## Acceptance criteria

- Stage perimeter can be updated via contiguous radius-delta batches (wrapping allowed).
- Lake can apply contiguous full-ray Y-delta batches via props (no imperative control).
- XZ interpolation and Y updates remain orthogonal and chunked.
- Frame cost remains bounded by shared `raySliceSizePerFrame` with independent cursors.
- Seam remains correct in both pipelines.
- Invalid batches fail fast with thrown, propagated errors.
- Legacy Y-copy/ref API is fully removed.

---

## Notes

- This document is the agreed plan only; implementation comes in a separate step.
- Keep changes surgical and aligned with existing architecture (store + queue + demand invalidation model).
