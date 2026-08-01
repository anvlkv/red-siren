import { useFrame, useThree } from "@react-three/fiber";
import {
    forwardRef,
    useCallback,
    useEffect,
    useImperativeHandle,
    useMemo,
    useRef,
} from "react";
import type { ThetaRange } from "../Stage";
import { useStage } from "../Stage";
import { createThetaMajorRingGeometry } from "./Lake.geometry";
import {
    clearQueue,
    enqueueRanges,
    processQueue,
    queueHasWork,
    type RangeQueue,
} from "./Lake.queue";
import type { LakeHandle, LakeProps } from "./Lake.types";

export type { LakeHandle, LakeProps, PositionRef } from "./Lake.types";

const Lake = forwardRef<LakeHandle, LakeProps>(function Lake(
    {
        baseline,
        innerRadius,
        phiSegments,
        yCopySourceRef,
        raySliceSizePerFrame = 128,
    },
    ref,
) {
    const { invalidate } = useThree();
    const { rBase, stageSegments, perimetryStore } = useStage();

    const safePhiSegments = Math.max(1, Math.floor(phiSegments));
    const safeSliceSize = Math.max(1, Math.floor(raySliceSizePerFrame));

    const geometryData = useMemo(
        () =>
            createThetaMajorRingGeometry(
                innerRadius,
                rBase,
                stageSegments,
                safePhiSegments,
            ),
        [innerRadius, rBase, safePhiSegments, stageSegments],
    );

    const interpolationQueueRef = useRef<RangeQueue>({
        ranges: [],
        rangeCursor: 0,
        thetaCursor: 0,
    });
    const yQueueRef = useRef<RangeQueue>({
        ranges: [],
        rangeCursor: 0,
        thetaCursor: 0,
    });

    const enqueueInterpolationRanges = useCallback(
        (ranges: ThetaRange[]) => {
            enqueueRanges(interpolationQueueRef.current, ranges, stageSegments);
        },
        [stageSegments],
    );

    const enqueueYCopyRange = useCallback(
        (start: number, end: number) => {
            enqueueRanges(
                yQueueRef.current,
                [{ start, end }],
                perimetryStore.thetaCount,
            );
            invalidate();
        },
        [invalidate, perimetryStore.thetaCount],
    );

    const enqueueYCopyRanges = useCallback(
        (ranges: ThetaRange[]) => {
            enqueueRanges(yQueueRef.current, ranges, perimetryStore.thetaCount);
            invalidate();
        },
        [invalidate, perimetryStore.thetaCount],
    );

    useImperativeHandle(
        ref,
        () => ({
            enqueueYCopyRange,
            enqueueYCopyRanges,
        }),
        [enqueueYCopyRange, enqueueYCopyRanges],
    );

    useEffect(() => {
        clearQueue(interpolationQueueRef.current);
        clearQueue(yQueueRef.current);
        enqueueInterpolationRanges([
            { start: 0, end: perimetryStore.thetaCount },
        ]);
        invalidate();
    }, [
        enqueueInterpolationRanges,
        geometryData.geometry,
        invalidate,
        perimetryStore,
    ]);

    useEffect(() => {
        const unsubscribe = perimetryStore.subscribe((dirtyRanges) => {
            enqueueInterpolationRanges(dirtyRanges);
        });

        return () => {
            unsubscribe();
        };
    }, [enqueueInterpolationRanges, perimetryStore]);

    useEffect(() => {
        return () => {
            geometryData.geometry.dispose();
        };
    }, [geometryData.geometry]);

    useFrame(() => {
        const {
            positionArray,
            innerAnchor,
            radialWeights,
            rayStride,
            radialCount,
        } = geometryData;

        let changed = false;
        let seamNeedsMirror = false;

        processQueue(interpolationQueueRef.current, safeSliceSize, (theta) => {
            const outer = perimetryStore.getPoint(theta);
            const innerOffset = theta * 3;
            const rayStart = theta * rayStride;

            const innerX = innerAnchor[innerOffset];
            const innerZ = innerAnchor[innerOffset + 2];
            const deltaX = outer[0] - innerX;
            const deltaZ = outer[2] - innerZ;

            let posOffset = rayStart;
            for (let j = 0; j < radialCount; j += 1) {
                const w = radialWeights[j];
                positionArray[posOffset] = innerX + deltaX * w;
                positionArray[posOffset + 2] = innerZ + deltaZ * w;
                posOffset += 3;
            }

            changed = true;
            if (theta === 0) {
                seamNeedsMirror = true;
            }
        });

        const ySource = yCopySourceRef?.current ?? null;
        const hasYQueueWork = queueHasWork(yQueueRef.current);
        const canCopyY =
            ySource !== null &&
            ySource.length === geometryData.expectedSourceLength;

        if (
            hasYQueueWork &&
            ySource !== null &&
            ySource.length !== geometryData.expectedSourceLength
        ) {
            console.warn(
                `Lake Y-copy source topology mismatch: expected ${geometryData.expectedSourceLength} values, got ${ySource.length}`,
            );
        }

        processQueue(yQueueRef.current, safeSliceSize, (theta) => {
            if (!canCopyY || ySource === null) {
                return;
            }

            const rayStart = theta * rayStride;

            for (let k = 0; k < rayStride; k += 3) {
                positionArray[rayStart + k + 1] = ySource[rayStart + k + 1];
            }

            changed = true;
            if (theta === 0) {
                seamNeedsMirror = true;
            }
        });

        if (seamNeedsMirror) {
            const seamRayStart = perimetryStore.thetaCount * rayStride;
            for (let k = 0; k < rayStride; k += 1) {
                positionArray[seamRayStart + k] = positionArray[k];
            }
            changed = true;
        }

        if (changed) {
            geometryData.positionAttribute.needsUpdate = true;
        }

        if (
            queueHasWork(interpolationQueueRef.current) ||
            queueHasWork(yQueueRef.current)
        ) {
            invalidate();
        }
    });

    return (
        <mesh
            position={[0, baseline, 0]}
            geometry={geometryData.geometry}
            receiveShadow
        >
            <meshStandardMaterial
                color="white"
                roughness={0.25}
                metalness={0.15}
            />
        </mesh>
    );
});

export default Lake;
