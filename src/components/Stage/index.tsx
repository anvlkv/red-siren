import { PerspectiveCamera } from "@react-three/drei";
import { useFrame, useThree } from "@react-three/fiber";
import { GUI } from "dat.gui";
import {
    createContext,
    Dispatch,
    PropsWithChildren,
    SetStateAction,
    useCallback,
    useContext,
    useEffect,
    useMemo,
    useRef,
    useState,
} from "react";
import * as THREE from "three";
import {
    applyRadiusDeltaBatch,
    createPerimetryStore,
    type PerimetryStore,
    type PerimetryTx,
    type ThetaRange,
} from "./perimetry";
import type { RadiusDeltaBatch } from "../World/Lake/types";
import { WorldLookAt } from "../World";
import { createNoise2D } from "simplex-noise";

export type { PerimetryStore, PerimetryTx, ThetaRange };

interface StageContextValue {
    lookAt: WorldLookAt;
    rBase: number;
    stageSegments: number;
    setStageSegments: Dispatch<SetStateAction<number>>;
    perimetryStore: PerimetryStore;
}

interface StageProps {
    lookAt: WorldLookAt;
    rBase: number;
    radiusDeltaBatches?: readonly RadiusDeltaBatch[];
}

const EMPTY_RADIUS_DELTA_BATCHES: readonly RadiusDeltaBatch[] = [];

function createRandomRadiusDeltas(thetaCount: number) {
    const noiseA = createNoise2D();
    const noiseB = createNoise2D();
    const deltas = new Float32Array(thetaCount);

    for (let theta = 0; theta < thetaCount; theta += 1) {
        const lowFreq = noiseA(theta * 0.018, 0.12) * 16;
        const highFreq = noiseB(theta * 0.073, 1.37) * 4;
        deltas[theta] = lowFreq + highFreq;
    }

    return deltas;
}

const StageContext = createContext<StageContextValue | null>(null);

function Stage({
    lookAt,
    rBase,
    radiusDeltaBatches = EMPTY_RADIUS_DELTA_BATCHES,
    children,
}: PropsWithChildren<StageProps>) {
    const { invalidate } = useThree();
    const [segments, setSegments] = useState(360);

    const setStageSegments = useCallback<Dispatch<SetStateAction<number>>>(
        (next) => {
            setSegments((prev) => {
                const resolved = typeof next === "function" ? next(prev) : next;
                return Math.max(3, Math.floor(resolved));
            });
        },
        [],
    );

    const stageGeometryBase = useMemo(() => {
        const baseGeometry = new THREE.CircleGeometry(rBase, segments);
        baseGeometry.rotateX(-Math.PI / 2);
        return baseGeometry;
    }, [rBase, segments]);

    const stagePerimetryGeometry = useMemo(
        () => stageGeometryBase.clone(),
        [stageGeometryBase],
    );

    const perimetryStore = useMemo(
        () =>
            createPerimetryStore({
                thetaCount: segments,
                basePositionAttribute: stageGeometryBase.attributes
                    .position as THREE.BufferAttribute,
                perimetryPositionAttribute: stagePerimetryGeometry.attributes
                    .position as THREE.BufferAttribute,
                invalidate,
            }),
        [invalidate, segments, stageGeometryBase, stagePerimetryGeometry],
    );

    const appliedRadiusBatchIdsRef = useRef(new Set<number>());
    const lastRadiusBatchIdRef = useRef(Number.NEGATIVE_INFINITY);

    useEffect(() => {
        appliedRadiusBatchIdsRef.current.clear();
    }, [perimetryStore.thetaCount]);

    useEffect(() => {
        if (radiusDeltaBatches.length > 0) {
            return;
        }

        applyRadiusDeltaBatch(perimetryStore, {
            id: 0,
            start: 0,
            deltaRadius: createRandomRadiusDeltas(perimetryStore.thetaCount),
        });
    }, [perimetryStore, radiusDeltaBatches]);

    useEffect(() => {
        if (radiusDeltaBatches.length === 0) {
            return;
        }

        const seenInProp = new Set<number>();
        let previousId = Number.NEGATIVE_INFINITY;

        for (let i = 0; i < radiusDeltaBatches.length; i += 1) {
            const batch = radiusDeltaBatches[i];

            if (!Number.isInteger(batch.id) || batch.id < 0) {
                throw new Error(
                    `RadiusDeltaBatch.id at index ${i} must be a non-negative integer`,
                );
            }

            if (seenInProp.has(batch.id)) {
                throw new Error(
                    `Duplicate RadiusDeltaBatch id ${batch.id} in prop payload`,
                );
            }

            if (batch.id <= previousId) {
                throw new Error(
                    `RadiusDeltaBatch ids must be strictly increasing: got ${batch.id} after ${previousId}`,
                );
            }

            seenInProp.add(batch.id);
            previousId = batch.id;
        }

        const consumedIds = appliedRadiusBatchIdsRef.current;

        for (let i = 0; i < radiusDeltaBatches.length; i += 1) {
            const batch = radiusDeltaBatches[i];

            if (consumedIds.has(batch.id)) {
                continue;
            }

            if (batch.id <= lastRadiusBatchIdRef.current) {
                throw new Error(
                    `Stale or non-monotonic RadiusDeltaBatch id ${batch.id}; last applied id is ${lastRadiusBatchIdRef.current}`,
                );
            }

            applyRadiusDeltaBatch(perimetryStore, batch);
            consumedIds.add(batch.id);
            lastRadiusBatchIdRef.current = batch.id;
        }
    }, [perimetryStore, radiusDeltaBatches]);

    const focusTarget = useMemo(() => {
        switch (lookAt) {
            case WorldLookAt.Shore:
            case WorldLookAt.LighthouseIsland:
            case WorldLookAt.Mountain:
            default:
                return new THREE.Vector3(0, 65, 0);
        }
    }, [lookAt]);

    const stagePerimetryPosition = useCallback(
        (t: number) => {
            const count = perimetryStore.thetaCount;
            const normalizedT = ((t % 1) + 1) % 1;
            const scaled = normalizedT * count;
            const lowIndex = Math.floor(scaled) % count;
            const highIndex = (lowIndex + 1) % count;
            const alpha = scaled - Math.floor(scaled);

            const low = perimetryStore.getPoint(lowIndex);
            const high = perimetryStore.getPoint(highIndex);

            return new THREE.Vector3(
                THREE.MathUtils.lerp(low[0], high[0], alpha),
                THREE.MathUtils.lerp(low[1], high[1], alpha),
                THREE.MathUtils.lerp(low[2], high[2], alpha),
            );
        },
        [perimetryStore],
    );

    const ref = useRef<{ pos: number; fov: number; elevation: number }>({
        pos: 0,
        fov: 35,
        elevation: 0,
    });
    const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);

    useEffect(() => {
        const gui = new GUI({ name: "Camera" });
        gui.add(ref.current, "pos", 0, 1, 0.001)
            .name("Camera Position")
            .onChange(invalidate);
        gui.add(ref.current, "fov", 1, 1000).name("FOV").onChange(invalidate);
        gui.add(ref.current, "elevation", -1000, 1000)
            .name("Elevation")
            .onChange(invalidate);
        return () => {
            gui.destroy();
        };
    }, [invalidate]);

    useFrame(() => {
        if (!cameraRef.current) {
            return;
        }

        cameraRef.current.position.copy(
            stagePerimetryPosition(ref.current.pos).add(
                new THREE.Vector3(0, ref.current.elevation, 0),
            ),
        );
        cameraRef.current.lookAt(focusTarget);
        cameraRef.current.setFocalLength(ref.current.fov);
    });

    return (
        <StageContext.Provider
            value={{
                lookAt,
                rBase,
                stageSegments: segments,
                setStageSegments,
                perimetryStore,
            }}
        >
            <ambientLight />
            <PerspectiveCamera
                ref={cameraRef}
                makeDefault
                position={stagePerimetryPosition(0)}
            />

            {children}
        </StageContext.Provider>
    );
}

export default Stage;

export function useStage() {
    const context = useContext(StageContext);
    if (!context) {
        throw new Error("useStage must be used within a Stage");
    }
    return context;
}
