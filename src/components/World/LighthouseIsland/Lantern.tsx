import { darken, mix, lighten, transparentize } from "color2k";
import { useRef, useMemo, useEffect } from "react";
import { createNoise2D } from "simplex-noise";
import * as THREE from "three";
import { useAppTheme } from "../../../App/Theme";
import { useThree } from "@react-three/fiber";

function Lantern({
    r,
    height,
    tall,
}: {
    r: number;
    height: number;
    tall: number;
}) {
    const { invalidate } = useThree();
    const {
        primaryColor,
        backgroundColor,
        secondaryColor,
        tertiaryColor,
        pixelRatio,
    } = useAppTheme();

    const spotRef = useRef<THREE.SpotLight>(null);
    const targetRef = useRef<THREE.Object3D>(null);

    const supportCount = 8;
    const railingCount = 14;
    const step = (Math.PI * 2) / supportCount;
    const ringRotation = step / 2;
    const ringRadialSegments = Math.max(8, supportCount);
    const ringTubularSegments = supportCount * 4;

    const frameRadius = 0.07;

    const baseRadius = r * 1.35;
    const baseHeight = Math.max(0.35, height * 0.25);
    const deckThickness = Math.max(0.06, height * 0.05);

    const lanternRadius = r * 0.92;
    const lanternBaseY = baseHeight + deckThickness;
    const lanternHeight = Math.max(0.62, height * 0.58);
    const roofBaseY = lanternBaseY + lanternHeight;

    const supportRadius = lanternRadius * 1.02;
    const bottomRingY = lanternBaseY + frameRadius;
    const topRingY = roofBaseY - frameRadius;
    const supportHeight = topRingY - bottomRingY + frameRadius * 0.8;
    const supportY = (bottomRingY + topRingY) / 2;

    const sideApothem = supportRadius * Math.cos(Math.PI / supportCount);
    const panelRadius = sideApothem - frameRadius * 0.9;
    const panelWidth = 2 * panelRadius * Math.tan(Math.PI / supportCount);
    const panelHeight = topRingY - bottomRingY + frameRadius * 0.4;
    const panelY = (topRingY + bottomRingY) / 2;

    const roofRadius = lanternRadius * 1.03;
    const roofBrimHeight = Math.max(0.08, height * 0.08);
    const spireHeight = Math.max(0.2, height * 0.23);

    const railRadius = baseRadius * 1.06;
    const railHeight = Math.max(0.12, height * 0.11);
    const railBaseY = lanternBaseY + 0.02;

    const lampY = lanternBaseY + lanternHeight * 0.5;
    const beamAngle = Math.PI * 0.62;

    const baseColor = darken(tertiaryColor, 0.16);
    const deckColor = mix(secondaryColor, backgroundColor, 0.75);
    const frameColor = darken(primaryColor, 0.24);
    const frameAccentColor = lighten(frameColor, 0.16);
    const railColor = mix(frameAccentColor, backgroundColor, 0.45);
    const roofBrimColor = darken(primaryColor, 0.18);
    const roofColor = darken(primaryColor, 0.06);
    const spireColor = lighten(secondaryColor, 0.2);
    const finialColor = lighten(secondaryColor, 0.34);
    const lampCoreColor = lighten(secondaryColor, 0.34);
    const lampEmissiveColor = lighten(secondaryColor, 0.2);
    const lightColor = lighten(secondaryColor, 0.38);
    const glassColor = mix(backgroundColor, secondaryColor, 0.38);

    const glassTexture = useMemo(() => {
        const size = Math.max(128, Math.floor(220 * Math.min(pixelRatio, 2)));
        const canvas = document.createElement("canvas");
        canvas.width = size;
        canvas.height = size;

        const ctx = canvas.getContext("2d")!;

        const gradient = ctx.createLinearGradient(0, 0, size, size);
        gradient.addColorStop(
            0,
            transparentize(lighten(backgroundColor, 0.32), 0.2),
        );
        gradient.addColorStop(
            1,
            transparentize(mix(backgroundColor, secondaryColor, 0.45), 0.38),
        );
        ctx.fillStyle = gradient;
        ctx.fillRect(0, 0, size, size);

        const frostNoise = createNoise2D();
        const frostDotColor = lighten(backgroundColor, 0.48);
        const frostLineColor = lighten(secondaryColor, 0.45);

        for (let y = 0; y < size; y += 3) {
            for (let x = 0; x < size; x += 3) {
                const n = frostNoise(x / size, y / size) * 0.5 + 0.5;
                const alpha = 0.04 + n * 0.11;
                ctx.fillStyle = transparentize(frostDotColor, 1 - alpha);
                ctx.fillRect(x, y, 2, 2);
            }
        }

        for (let x = 0; x < size; x += 11) {
            const wobble = (frostNoise(x / 23, 0.27) * 0.5 + 0.5) * 8;
            ctx.strokeStyle = transparentize(frostLineColor, 0.9);
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(x + wobble, 0);
            ctx.lineTo(x - wobble * 0.35, size);
            ctx.stroke();
        }

        const texture = new THREE.CanvasTexture(canvas);
        texture.wrapS = THREE.RepeatWrapping;
        texture.wrapT = THREE.RepeatWrapping;
        texture.repeat.set(2, 1);
        texture.needsUpdate = true;

        invalidate();

        return texture;
    }, [backgroundColor, secondaryColor, pixelRatio]);

    useEffect(() => {
        if (!spotRef.current || !targetRef.current) return;

        spotRef.current.target = targetRef.current;
        spotRef.current.target.updateMatrixWorld();
    }, []);

    useEffect(
        () => () => {
            glassTexture.dispose();
        },
        [glassTexture],
    );

    return (
        <group position={[0, tall, 0]}>
            {/* base */}
            <mesh position={[0, baseHeight / 2, 0]} castShadow receiveShadow>
                <cylinderGeometry
                    args={[baseRadius * 0.95, baseRadius, baseHeight, 36]}
                />
                <meshStandardMaterial color={baseColor} roughness={0.85} />
            </mesh>
            <mesh
                position={[0, lanternBaseY - deckThickness / 2, 0]}
                castShadow
                receiveShadow
            >
                <cylinderGeometry
                    args={[
                        baseRadius * 1.075,
                        baseRadius * 1.02,
                        deckThickness,
                        36,
                    ]}
                />
                <meshStandardMaterial color={deckColor} roughness={0.7} />
            </mesh>

            {/* railing */}
            {Array.from({ length: railingCount }).map((_, i) => {
                const angle = (i / railingCount) * Math.PI * 2;
                const x = Math.cos(angle) * railRadius;
                const z = Math.sin(angle) * railRadius;
                return (
                    <mesh
                        key={`rail-post-${i}`}
                        position={[x, railBaseY + railHeight / 2, z]}
                        castShadow
                    >
                        <cylinderGeometry args={[0.02, 0.02, railHeight, 8]} />
                        <meshStandardMaterial
                            color={railColor}
                            metalness={0.45}
                            roughness={0.4}
                        />
                    </mesh>
                );
            })}
            <mesh
                position={[0, railBaseY + railHeight, 0]}
                rotation={[Math.PI / 2, 0, 0]}
                castShadow
            >
                <torusGeometry args={[railRadius, 0.017, 10, 64]} />
                <meshStandardMaterial
                    color={frameAccentColor}
                    metalness={0.5}
                    roughness={0.36}
                />
            </mesh>

            {/* supports, from outer points of the circle to the roof */}
            {Array.from({ length: supportCount }).map((_, i) => {
                const angle = i * step;
                const x = Math.cos(angle) * supportRadius;
                const z = Math.sin(angle) * supportRadius;
                return (
                    <mesh
                        key={`support-${i}`}
                        position={[x, supportY, z]}
                        castShadow
                    >
                        <cylinderGeometry
                            args={[frameRadius, frameRadius, supportHeight, 10]}
                        />
                        <meshStandardMaterial
                            color={frameColor}
                            metalness={0.2}
                            roughness={0.6}
                        />
                    </mesh>
                );
            })}

            {/* lantern rings */}
            <mesh
                position={[0, bottomRingY, 0]}
                rotation={[Math.PI / 2, 0, ringRotation]}
                castShadow
            >
                <torusGeometry
                    args={[
                        supportRadius,
                        frameRadius,
                        ringRadialSegments,
                        ringTubularSegments,
                    ]}
                />
                <meshStandardMaterial
                    color={frameColor}
                    roughness={0.5}
                    metalness={0.25}
                />
            </mesh>
            <mesh
                position={[0, topRingY, 0]}
                rotation={[Math.PI / 2, 0, ringRotation]}
                castShadow
            >
                <torusGeometry
                    args={[
                        supportRadius,
                        frameRadius,
                        ringRadialSegments,
                        ringTubularSegments,
                    ]}
                />
                <meshStandardMaterial
                    color={frameColor}
                    roughness={0.5}
                    metalness={0.25}
                />
            </mesh>

            {/* glass panels with diffusing texture */}
            {Array.from({ length: supportCount }).map((_, i) => {
                const mid = i * step + step / 2;
                const x = Math.cos(mid) * panelRadius;
                const z = Math.sin(mid) * panelRadius;
                return (
                    <mesh
                        key={`glass-${i}`}
                        position={[x, panelY, z]}
                        rotation={[0, Math.PI / 2 - mid, 0]}
                        castShadow
                        receiveShadow
                    >
                        <planeGeometry args={[panelWidth, panelHeight]} />
                        <meshPhysicalMaterial
                            map={glassTexture}
                            bumpMap={glassTexture}
                            bumpScale={0.025}
                            color={glassColor}
                            roughness={0.8}
                            metalness={0.02}
                            transmission={0.58}
                            thickness={0.12}
                            opacity={0.72}
                            transparent
                            depthWrite={false}
                            side={THREE.DoubleSide}
                        />
                    </mesh>
                );
            })}

            {/* lamp: bright directional light */}
            <mesh position={[0, lampY, 0]}>
                <sphereGeometry args={[0.09, 16, 16]} />
                <meshStandardMaterial
                    color={lampCoreColor}
                    emissive={lampEmissiveColor}
                    emissiveIntensity={2.6}
                    toneMapped={false}
                />
            </mesh>
            <pointLight
                position={[0, lampY, 0]}
                color={lightColor}
                intensity={6}
                distance={300}
                decay={1.6}
            />
            <spotLight
                ref={spotRef}
                position={[0, lampY, 0]}
                color={lightColor}
                intensity={48}
                angle={0.34}
                penumbra={0.35}
                distance={300}
                decay={1.15}
                castShadow
            />
            <object3D
                ref={targetRef}
                position={[
                    Math.cos(beamAngle) * 24,
                    lampY - 0.65,
                    Math.sin(beamAngle) * 24,
                ]}
            />

            {/* roof: brim + hemisphere + spire */}
            <mesh position={[0, roofBaseY - roofBrimHeight / 2, 0]} castShadow>
                <cylinderGeometry
                    args={[
                        roofRadius * 1.22,
                        roofRadius * 1.08,
                        roofBrimHeight,
                        36,
                    ]}
                />
                <meshStandardMaterial
                    color={roofBrimColor}
                    roughness={0.55}
                    metalness={0.15}
                />
            </mesh>
            <mesh position={[0, roofBaseY, 0]} castShadow>
                <sphereGeometry
                    args={[roofRadius, 32, 20, 0, Math.PI * 2, 0, Math.PI / 2]}
                />
                <meshStandardMaterial
                    color={roofColor}
                    roughness={0.44}
                    metalness={0.18}
                />
            </mesh>
            <mesh
                position={[0, roofBaseY + roofRadius + spireHeight * 0.48, 0]}
                castShadow
            >
                <coneGeometry args={[roofRadius * 0.14, spireHeight, 16]} />
                <meshStandardMaterial
                    color={spireColor}
                    metalness={0.68}
                    roughness={0.28}
                />
            </mesh>
            <mesh
                position={[0, roofBaseY + roofRadius + spireHeight + 0.03, 0]}
                castShadow
            >
                <sphereGeometry args={[roofRadius * 0.05, 12, 12]} />
                <meshStandardMaterial
                    color={finialColor}
                    metalness={0.74}
                    roughness={0.25}
                />
            </mesh>
        </group>
    );
}

export default Lantern;
