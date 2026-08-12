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
import { WorldLookAt } from "./World/World";

const StageContext = createContext<{
    lookAt: WorldLookAt;
    rBase: number;
    stageSegments: number;
    setStageSegments: Dispatch<SetStateAction<number>>;
    stagePerimetryPointsGetter: () => (i: number) => THREE.TypedArray | null;
    stagePerimetryPointsSetter: () => (
        i: number,
        s: (base: THREE.TypedArray) => THREE.Vector3,
    ) => void;
} | null>(null);

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

    const stageGeometryBase = useMemo(() => {
        const baseGeometry = new THREE.CircleGeometry(rBase, segments);
        baseGeometry.rotateX(-Math.PI / 2);
        return baseGeometry;
    }, [rBase, segments]);

    const stagePerimetryGeometry = useMemo(
        () => stageGeometryBase.clone(),
        [stageGeometryBase],
    );

    // const focusTarget = useMemo(() => {
    //     switch (lookAt) {
    //         case WorldLookAt.Shore:
    //         case WorldLookAt.LighthouseIsland:
    //         case WorldLookAt.Mountain:
    //         default:
    //             return new THREE.Vector3(0, 65, 0);
    //     }
    // }, [lookAt]);

    const stagePerimetryPosition = useCallback(
        (t: number) => {
            const count = stagePerimetryGeometry.attributes.position.count - 1;
            const lowIndex = Math.floor(t * count) + 1;
            const highIndex = ((lowIndex + 1) % count) + 1;
            const lowPosition = new THREE.Vector3(
                stagePerimetryGeometry.attributes.position.getX(lowIndex),
                stagePerimetryGeometry.attributes.position.getY(lowIndex),
                stagePerimetryGeometry.attributes.position.getZ(lowIndex),
            );
            const highPosition = new THREE.Vector3(
                stagePerimetryGeometry.attributes.position.getX(highIndex),
                stagePerimetryGeometry.attributes.position.getY(highIndex),
                stagePerimetryGeometry.attributes.position.getZ(highIndex),
            );
            return lowPosition.lerp(highPosition, t * count - lowIndex);
        },
        [stagePerimetryGeometry],
    );

    const stagePerimetryPointsGetter = useCallback(() => {
        const count = stagePerimetryGeometry.attributes.position.count - 1;
        const pos = stagePerimetryGeometry.attributes.position;

        return (i: number) => {
            if (i >= count) {
                return null;
            }
            const innerI = i + 1;

            return pos.array.subarray(innerI * 3, innerI * 3 + 3);
        };
    }, [stagePerimetryGeometry]);

    const stagePerimetryPointsSetter = useCallback(() => {
        const count = stageGeometryBase.attributes.position.count - 1;
        const basePos = stageGeometryBase.attributes.position;
        const stagePerimetryPos = stagePerimetryGeometry.attributes.position;
        return (i: number, s: (base: THREE.TypedArray) => THREE.Vector3) => {
            if (i >= count) {
                return;
            }
            const innerI = i + 1;
            const base = basePos.array.subarray(innerI * 3, innerI * 3 + 3);
            const v = s(base);

            stagePerimetryPos.setXYZ(innerI, v.x, v.y, v.z);
            stagePerimetryPos.needsUpdate = true;
            invalidate();
        };
    }, [stagePerimetryGeometry, stageGeometryBase]);

    const ref = useRef<{
        pos: number;
        fov: number;
        elevation: number;
        translationX: number;
        translationY: number;
        translationZ: number;
        focusTarget: THREE.Vector3;
    }>({
        pos: 0,
        fov: 35,
        elevation: 0,
        translationX: 0,
        translationY: 0,
        translationZ: 0,
        focusTarget: new THREE.Vector3(0, 65, 0),
    });
    const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);

    useEffect(() => {
        const gui = new GUI({ name: "Camera" });
        // game like
        gui.add(ref.current, "pos", 0, 1, 0.001)
            .name("Camera Position")
            .onChange(invalidate);
        gui.add(ref.current, "fov", 1, 1000).name("FOV").onChange(invalidate);
        gui.add(ref.current, "elevation", -1000, 1000)
            .name("Elevation")
            .onChange(invalidate);
        // editor like
        gui.add(ref.current, "translationX", -1000, 1000)
            .name("Translation X")
            .onChange(invalidate);
        gui.add(ref.current, "translationY", -1000, 1000)
            .name("Translation Y")
            .onChange(invalidate);
        gui.add(ref.current, "translationZ", -1000, 1000)
            .name("Translation Z")
            .onChange(invalidate);

        gui.add(ref.current.focusTarget, "x", -1000, 1000)
            .name("Focus Target X")
            .onChange(invalidate);
        gui.add(ref.current.focusTarget, "y", -1000, 1000)
            .name("Focus Target Y")
            .onChange(invalidate);
        gui.add(ref.current.focusTarget, "z", -1000, 1000)
            .name("Focus Target Z")
            .onChange(invalidate);

        return () => {
            gui.destroy();
        };
    }, []);

    useFrame(() => {
        if (!cameraRef.current) {
            return;
        }

        cameraRef.current.position.copy(
            stagePerimetryPosition(ref.current.pos).add(
                new THREE.Vector3(0, ref.current.elevation, 0),
            ),
        );
        cameraRef.current.lookAt(ref.current.focusTarget);
        cameraRef.current.setFocalLength(ref.current.fov);

        cameraRef.current.position.x += ref.current.translationX;
        cameraRef.current.position.y += ref.current.translationY;
        cameraRef.current.position.z += ref.current.translationZ;
    });

    return (
        <StageContext.Provider
            value={{
                lookAt,
                rBase,
                stageSegments: segments,
                setStageSegments: setSegments,
                stagePerimetryPointsGetter,
                stagePerimetryPointsSetter,
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
