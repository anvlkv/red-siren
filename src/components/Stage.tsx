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
    createPerimetryStore,
    type PerimetryStore,
    type PerimetryTx,
    type ThetaRange,
} from "./Stage.perimetry";
import { WorldLookAt } from "./World/World";

export type { PerimetryStore, PerimetryTx, ThetaRange };

interface StageContextValue {
    lookAt: WorldLookAt;
    rBase: number;
    stageSegments: number;
    setStageSegments: Dispatch<SetStateAction<number>>;
    perimetryStore: PerimetryStore;
}

const StageContext = createContext<StageContextValue | null>(null);

function Stage({
    lookAt,
    rBase,
    children,
}: PropsWithChildren<{
    lookAt: WorldLookAt;
    rBase: number;
}>) {
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
