import { useThree } from "@react-three/fiber";
import { useMemo } from "react";
import { createNoise2D } from "simplex-noise";
import * as THREE from "three";
import { useAppTheme } from "../../../App/Theme";

function RockyIsland({ r, height }: { r: number; height: number }) {
    const { invalidate } = useThree();
    const geometry = useMemo(() => {
        const radialSegments = 32;
        const heightSegments = 4;

        const ringOffsets = [] as number[][];
        const radialNoise = createNoise2D();
        const spikeNoise = createNoise2D();
        const safeHeight = Number.isFinite(height) && height > 0 ? height : r;

        for (let ring = 0; ring <= heightSegments; ring++) {
            ringOffsets[ring] = [];

            for (let i = 0; i <= radialSegments; i++) {
                ringOffsets[ring][i] =
                    radialNoise(ring / heightSegments, i / radialSegments) *
                    0.85;
            }
        }

        const geometry = new THREE.CylinderGeometry(
            r * 0.75,
            r,
            safeHeight,
            radialSegments,
            heightSegments,
        );

        const pos = geometry.attributes.position;
        const v = new THREE.Vector3();
        const spikeCount = 11;

        for (let i = 0; i < pos.count; i++) {
            v.fromBufferAttribute(pos, i);

            const yNorm = (v.y + safeHeight / 2) / safeHeight;
            const topBlend = THREE.MathUtils.clamp((yNorm - 0.55) / 0.45, 0, 1);

            // ignore center vertices of caps, but keep top center slightly lifted for a rounded hill
            const radius = Math.sqrt(v.x * v.x + v.z * v.z);
            if (radius < 1e-5) {
                if (topBlend > 0.95) {
                    v.y += safeHeight * 0.08;
                    pos.setXYZ(i, v.x, v.y, v.z);
                }
                continue;
            }

            // determine which ring this belongs to
            const t = (safeHeight / 2 - v.y) / safeHeight;
            const ring = Math.round(t * heightSegments);

            // angle around cylinder
            let theta = Math.atan2(v.z, v.x);
            if (theta < 0) theta += Math.PI * 2;

            const slice = Math.round((theta / (Math.PI * 2)) * radialSegments);
            const offsetVal = ringOffsets[ring][slice];

            // rounded upper hill profile
            const hillShrink = 1 - topBlend * topBlend * 0.28;
            const hillLift = topBlend * topBlend * safeHeight * 0.08;

            // sharp spikes plus per-spike vertical offset
            const spikeWave = Math.max(0, Math.cos(theta * spikeCount));
            const spikeSharpness = Math.pow(spikeWave, 9);
            const spikeRandom =
                spikeNoise(
                    Math.cos(theta) + yNorm * 2.1,
                    Math.sin(theta) - yNorm * 1.7,
                ) *
                    0.5 +
                0.5;
            const spikeStrength = spikeSharpness * (0.5 + spikeRandom * 0.5);

            const radialSpikeOffset =
                spikeStrength * r * (0.1 + topBlend * 0.22);
            const verticalSpikeOffset =
                spikeStrength * safeHeight * (0.03 + topBlend * 0.12);

            const newRadius =
                (radius + offsetVal + radialSpikeOffset) * hillShrink;

            v.x *= newRadius / radius;
            v.z *= newRadius / radius;
            v.y += hillLift + verticalSpikeOffset;

            pos.setXYZ(i, v.x, v.y, v.z);
        }

        pos.needsUpdate = true;
        geometry.computeVertexNormals();

        invalidate();

        return geometry;
    }, [r, height]);

    const { tertiaryColor } = useAppTheme();

    return (
        <mesh
            position={[0, -height / 3, 0]}
            geometry={geometry}
            castShadow
            receiveShadow
        >
            <meshStandardMaterial color={tertiaryColor} roughness={0.95} />
        </mesh>
    );
}

export default RockyIsland;
