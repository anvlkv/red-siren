import * as THREE from "three";
import type { RadiusDeltaBatch } from "../World/Lake/types";

export type ThetaRange = { start: number; end: number }; // [start, end), modulo thetaCount

export interface PerimetryTx {
    setPoint(i: number, x: number, y: number, z: number): void;
    setRange(
        start: number,
        end: number,
        f: (i: number, base: Float32Array) => [number, number, number],
    ): void;
}

export interface PerimetryStore {
    thetaCount: number;
    getPoint(i: number): Float32Array;
    getBasePoint(i: number): Float32Array;
    transact(fn: (tx: PerimetryTx) => void): void;
    subscribe(fn: (dirty: ThetaRange[], version: number) => void): () => void;
}

interface CreatePerimetryStoreOptions {
    thetaCount: number;
    basePositionAttribute: THREE.BufferAttribute;
    perimetryPositionAttribute: THREE.BufferAttribute;
    invalidate: () => void;
}

function mod(value: number, n: number) {
    const r = value % n;
    return r < 0 ? r + n : r;
}

function normalizeRange(
    start: number,
    end: number,
    thetaCount: number,
): ThetaRange[] {
    if (thetaCount <= 0) {
        return [];
    }

    const normalizedStart = mod(start, thetaCount);
    const normalizedEnd = mod(end, thetaCount);

    if (normalizedStart === normalizedEnd) {
        if (start === end) {
            return [];
        }
        return [{ start: 0, end: thetaCount }];
    }

    if (normalizedStart < normalizedEnd) {
        return [{ start: normalizedStart, end: normalizedEnd }];
    }

    return [
        { start: normalizedStart, end: thetaCount },
        { start: 0, end: normalizedEnd },
    ];
}

function mergeDirtyRanges(
    dirtyFlags: Uint8Array,
    thetaCount: number,
): ThetaRange[] {
    const ranges: ThetaRange[] = [];

    let i = 0;
    while (i < thetaCount) {
        if (dirtyFlags[i] === 0) {
            i += 1;
            continue;
        }

        const start = i;
        i += 1;
        while (i < thetaCount && dirtyFlags[i] === 1) {
            i += 1;
        }

        ranges.push({ start, end: i });
    }

    if (
        ranges.length > 1 &&
        ranges[0].start === 0 &&
        ranges[ranges.length - 1].end === thetaCount
    ) {
        const wrapped: ThetaRange = {
            start: ranges[ranges.length - 1].start,
            end: ranges[0].end,
        };
        const middle = ranges.slice(1, ranges.length - 1);
        middle.push(wrapped);
        return middle;
    }

    return ranges;
}

export function createPerimetryStore({
    thetaCount,
    basePositionAttribute,
    perimetryPositionAttribute,
    invalidate,
}: CreatePerimetryStoreOptions): PerimetryStore {
    const base = new Float32Array(thetaCount * 3);
    const current = new Float32Array(thetaCount * 3);

    const basePos = basePositionAttribute.array as Float32Array;
    const stagePos = perimetryPositionAttribute.array as Float32Array;

    for (let i = 0; i < thetaCount; i += 1) {
        const logicalOffset = i * 3;
        const stageOffset = (i + 1) * 3;

        base[logicalOffset] = basePos[stageOffset];
        base[logicalOffset + 1] = basePos[stageOffset + 1];
        base[logicalOffset + 2] = basePos[stageOffset + 2];

        current[logicalOffset] = base[logicalOffset];
        current[logicalOffset + 1] = base[logicalOffset + 1];
        current[logicalOffset + 2] = base[logicalOffset + 2];

        stagePos[stageOffset] = current[logicalOffset];
        stagePos[stageOffset + 1] = current[logicalOffset + 1];
        stagePos[stageOffset + 2] = current[logicalOffset + 2];
    }

    const seamOffset = (thetaCount + 1) * 3;
    stagePos[seamOffset] = current[0];
    stagePos[seamOffset + 1] = current[1];
    stagePos[seamOffset + 2] = current[2];
    perimetryPositionAttribute.needsUpdate = true;

    let version = 0;
    const listeners = new Set<
        (dirty: ThetaRange[], nextVersion: number) => void
    >();

    const markSeamFromZero = () => {
        stagePos[seamOffset] = current[0];
        stagePos[seamOffset + 1] = current[1];
        stagePos[seamOffset + 2] = current[2];
    };

    const setCurrentPoint = (
        dirtyFlags: Uint8Array,
        i: number,
        x: number,
        y: number,
        z: number,
    ) => {
        const normalizedI = mod(i, thetaCount);
        const logicalOffset = normalizedI * 3;
        current[logicalOffset] = x;
        current[logicalOffset + 1] = y;
        current[logicalOffset + 2] = z;
        dirtyFlags[normalizedI] = 1;
    };

    return {
        thetaCount,
        getPoint: (i: number) => {
            const normalizedI = mod(i, thetaCount);
            const logicalOffset = normalizedI * 3;
            return current.subarray(logicalOffset, logicalOffset + 3);
        },
        getBasePoint: (i: number) => {
            const normalizedI = mod(i, thetaCount);
            const logicalOffset = normalizedI * 3;
            return base.subarray(logicalOffset, logicalOffset + 3);
        },
        transact: (fn: (tx: PerimetryTx) => void) => {
            const dirtyFlags = new Uint8Array(thetaCount);

            fn({
                setPoint: (i: number, x: number, y: number, z: number) => {
                    setCurrentPoint(dirtyFlags, i, x, y, z);
                },
                setRange: (
                    start: number,
                    end: number,
                    f: (
                        i: number,
                        basePoint: Float32Array,
                    ) => [number, number, number],
                ) => {
                    const ranges = normalizeRange(start, end, thetaCount);
                    for (
                        let rangeIndex = 0;
                        rangeIndex < ranges.length;
                        rangeIndex += 1
                    ) {
                        const range = ranges[rangeIndex];
                        for (let i = range.start; i < range.end; i += 1) {
                            const logicalOffset = i * 3;
                            const basePoint = base.subarray(
                                logicalOffset,
                                logicalOffset + 3,
                            );
                            const [x, y, z] = f(i, basePoint);
                            current[logicalOffset] = x;
                            current[logicalOffset + 1] = y;
                            current[logicalOffset + 2] = z;
                            dirtyFlags[i] = 1;
                        }
                    }
                },
            });

            let hasDirty = false;
            let touchesThetaZero = false;

            for (let i = 0; i < thetaCount; i += 1) {
                if (dirtyFlags[i] === 0) {
                    continue;
                }

                hasDirty = true;
                if (i === 0) {
                    touchesThetaZero = true;
                }

                const logicalOffset = i * 3;
                const stageOffset = (i + 1) * 3;
                stagePos[stageOffset] = current[logicalOffset];
                stagePos[stageOffset + 1] = current[logicalOffset + 1];
                stagePos[stageOffset + 2] = current[logicalOffset + 2];
            }

            if (!hasDirty) {
                return;
            }

            if (touchesThetaZero) {
                markSeamFromZero();
            }

            perimetryPositionAttribute.needsUpdate = true;
            invalidate();

            version += 1;
            const dirtyRanges = mergeDirtyRanges(dirtyFlags, thetaCount);
            listeners.forEach((listener) => {
                listener(dirtyRanges, version);
            });
        },
        subscribe: (fn: (dirty: ThetaRange[], nextVersion: number) => void) => {
            listeners.add(fn);
            return () => {
                listeners.delete(fn);
            };
        },
    };
}

function assertFiniteNumber(value: number, label: string) {
    if (!Number.isFinite(value)) {
        throw new Error(`${label} must be a finite number`);
    }
}

export function applyRadiusDeltaBatch(
    store: PerimetryStore,
    batch: RadiusDeltaBatch,
) {
    if (!Number.isInteger(batch.id) || batch.id < 0) {
        throw new Error(`RadiusDeltaBatch.id must be a non-negative integer`);
    }

    if (!Number.isInteger(batch.start)) {
        throw new Error(`RadiusDeltaBatch.start must be an integer`);
    }

    if (!(batch.deltaRadius instanceof Float32Array)) {
        throw new Error(`RadiusDeltaBatch.deltaRadius must be a Float32Array`);
    }

    const thetaCount = store.thetaCount;
    const rayCount = batch.deltaRadius.length;

    if (rayCount === 0) {
        throw new Error(`RadiusDeltaBatch.deltaRadius must not be empty`);
    }

    if (rayCount > thetaCount) {
        throw new Error(
            `RadiusDeltaBatch.deltaRadius length ${rayCount} exceeds thetaCount ${thetaCount}`,
        );
    }

    for (let k = 0; k < rayCount; k += 1) {
        assertFiniteNumber(
            batch.deltaRadius[k],
            `RadiusDeltaBatch.deltaRadius[${k}]`,
        );
    }

    store.transact((tx) => {
        for (let k = 0; k < rayCount; k += 1) {
            const theta = mod(batch.start + k, thetaCount);
            const basePoint = store.getBasePoint(theta);

            const baseX = basePoint[0];
            const baseY = basePoint[1];
            const baseZ = basePoint[2];
            const baseRadius = Math.hypot(baseX, baseZ);

            if (!Number.isFinite(baseRadius) || baseRadius <= 0) {
                throw new Error(
                    `Invalid base radius at theta ${theta}: ${baseRadius}`,
                );
            }

            const dirX = baseX / baseRadius;
            const dirZ = baseZ / baseRadius;
            const targetRadius = baseRadius + batch.deltaRadius[k];

            assertFiniteNumber(
                targetRadius,
                `RadiusDeltaBatch target radius at theta ${theta}`,
            );

            tx.setPoint(theta, dirX * targetRadius, baseY, dirZ * targetRadius);
        }
    });
}
