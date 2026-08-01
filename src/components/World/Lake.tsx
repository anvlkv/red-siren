import * as THREE from "three";
import { useStage } from "../Stage";
import { useMemo } from "react";

function Lake({
    baseline,
    innerRadius,
    phiSegments,
}: {
    baseline: number;
    innerRadius: number;
    phiSegments: number;
}) {
    const { rBase, stagePerimetryPointsGetter, stageSegments } = useStage();

    const geometry = useMemo(() => {
        const baseGeometry = new THREE.RingGeometry(
            innerRadius,
            rBase,
            stageSegments,
            phiSegments,
        );
        baseGeometry.rotateX(-Math.PI / 2);
        return baseGeometry;
    }, [
        innerRadius,
        rBase,
        stageSegments,
        phiSegments,
        stagePerimetryPointsGetter,
    ]);

    return (
        <mesh position={[0, baseline, 0]} geometry={geometry} receiveShadow>
            <meshStandardMaterial
                color="white"
                roughness={0.25}
                metalness={0.15}
            />
        </mesh>
    );
}

export default Lake;
