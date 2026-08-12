import * as THREE from "three";
import { useStage } from "../Stage";
import {
    forwardRef,
    useEffect,
    useImperativeHandle,
    useMemo,
    useState,
} from "react";
import { createNoise2D } from "simplex-noise";
import { Sphere } from "@react-three/drei";
import { GUI } from "dat.gui";

export type RangeUpdateFn = (
    index: number,
    base: THREE.TypedArray[],
    current: THREE.TypedArray[],
) => THREE.Vector3[];

class RingGeometryStore {
    base: THREE.RingGeometry;
    deformed: THREE.RingGeometry;

    constructor(
        innerRadius?: number | undefined,
        outerRadius?: number | undefined,
        thetaSegments?: number | undefined,
        phiSegments?: number | undefined,
    ) {
        const base = new THREE.RingGeometry(
            innerRadius,
            outerRadius,
            thetaSegments,
            phiSegments,
        );
        base.rotateX(-Math.PI / 2);
        this.base = base;

        const noise = createNoise2D();
        const radius = this.base.parameters.outerRadius;
        const pos = this.base.attributes.position;
        for (let t = 0; t < base.parameters.thetaSegments; t++) {
            const addresses = this.t_addresses(t);
            const noiseValues = Array.from(
                { length: this.base.parameters.phiSegments + 1 },
                (_, p) => noise(t, p),
            );
            const noizeSum = noiseValues.reduce((acc, val) => acc + val, 0);
            const radiusIncrement = radius * (noizeSum / noiseValues.length);
            let radiusAcc = 0;
            for (let i = 0; i < addresses.length; i++) {
                const address = addresses[i];
                const x = pos.getX(address);
                const z = pos.getZ(address);
                const nz = noiseValues[i];
                const nzFactor = nz / noizeSum;
                const newRadius =
                    radius + radiusIncrement * nzFactor + radiusAcc;
                radiusAcc += radiusIncrement * nzFactor;

                const newX = (x / radius) * newRadius;
                const newZ = (z / radius) * newRadius;

                pos.setX(address, newX);
                pos.setZ(address, newZ);
                pos.setY(address, nz);
            }
        }
        pos.needsUpdate = true;
        this.base.computeVertexNormals();
        this.base.computeTangents();
        this.deformed = this.base.clone();
    }

    address(p: number, t: number): number {
        return p * (this.base.parameters.thetaSegments + 1) + t;
    }

    p_addresses(p: number): number[] {
        return Array.from(
            { length: this.base.parameters.thetaSegments + 1 },
            (_, t) => this.address(p, t),
        );
    }

    t_addresses(t: number): number[] {
        return Array.from(
            { length: this.base.parameters.phiSegments + 1 },
            (_, p) => this.address(p, t),
        );
    }

    update_t_range(start: number, end: number, f: RangeUpdateFn) {
        for (let t = start; t < end; t++) {
            const addresses = this.t_addresses(t);
            const pos = addresses.map((address) =>
                this.deformed.attributes.position.array.subarray(
                    address * 3,
                    address * 3 + 3,
                ),
            );
            const basePos = addresses.map((address) =>
                this.base.attributes.position.array.subarray(
                    address * 3,
                    address * 3 + 3,
                ),
            );
            const newPositions = f(t - start, basePos, pos);
            addresses.forEach((address, i) => {
                const newPos = newPositions[i];
                this.deformed.attributes.position.setXYZ(
                    address,
                    newPos.x,
                    newPos.y,
                    newPos.z,
                );
            });
        }
        this.deformed.attributes.position.needsUpdate = true;
        this.deformed.computeVertexNormals();
        this.deformed.computeTangents();
    }

    update_p_range(start: number, end: number, f: RangeUpdateFn) {
        for (let p = start; p < end; p++) {
            const addresses = this.p_addresses(p);
            const pos = addresses.map((address) =>
                this.deformed.attributes.position.array.subarray(
                    address * 3,
                    address * 3 + 3,
                ),
            );
            const basePos = addresses.map((address) =>
                this.base.attributes.position.array.subarray(
                    address * 3,
                    address * 3 + 3,
                ),
            );
            const newPositions = f(p - start, basePos, pos);
            addresses.forEach((address, i) => {
                const newPos = newPositions[i];
                this.deformed.attributes.position.setXYZ(
                    address,
                    newPos.x,
                    newPos.y,
                    newPos.z,
                );
            });
        }
        this.deformed.attributes.position.needsUpdate = true;
        this.deformed.computeVertexNormals();
        this.deformed.computeTangents();
    }
}

export type LakeRef = { geometry: RingGeometryStore };

function Lake(
    {
        baseline,
        innerRadius,
        phiSegments,
    }: {
        baseline: number;
        innerRadius: number;
        phiSegments: number;
    },
    ref: React.Ref<LakeRef>,
) {
    const { rBase, stagePerimetryPointsGetter, stageSegments } = useStage();

    const geometry = useMemo(() => {
        const baseGeometry = new RingGeometryStore(
            innerRadius,
            rBase,
            stageSegments,
            phiSegments,
        );
        return baseGeometry;
    }, [
        innerRadius,
        rBase,
        stageSegments,
        phiSegments,
        stagePerimetryPointsGetter,
    ]);

    const [showPAt, setShowPAt] = useState<number>(0);
    const [showTAt, setShowPTAt] = useState<number>(0);

    useEffect(() => {
        const gui = new GUI();
        gui.add({ showPAt }, "showPAt", 0, phiSegments)
            .step(1)
            .onChange((v) => {
                setShowPAt(v);
            });
        gui.add({ showTAt }, "showTAt", 0, stageSegments)
            .step(1)
            .onChange((v) => {
                setShowPTAt(v);
            });
        return () => {
            gui.destroy();
        };
    }, [phiSegments, stageSegments]);

    useImperativeHandle(
        ref,
        () => ({
            geometry,
        }),
        [geometry],
    );

    return (
        <group>
            <mesh
                position={[0, baseline, 0]}
                geometry={geometry.deformed}
                receiveShadow
            >
                <meshStandardMaterial
                    color="blue"
                    wireframe
                    roughness={0.25}
                    metalness={0.15}
                />
            </mesh>
            <group>
                {geometry.t_addresses(showTAt).map((address) => {
                    const pos = new THREE.Vector3(
                        geometry.deformed.attributes.position.getX(address),
                        geometry.deformed.attributes.position.getY(address),
                        geometry.deformed.attributes.position.getZ(address),
                    );
                    return <Sphere key={`t_at_${address}`} position={pos} />;
                })}
            </group>
            <group>
                {geometry.p_addresses(showPAt).map((address) => {
                    const pos = new THREE.Vector3(
                        geometry.deformed.attributes.position.getX(address),
                        geometry.deformed.attributes.position.getY(address),
                        geometry.deformed.attributes.position.getZ(address),
                    );
                    return <Sphere key={`p_at_${address}`} position={pos} />;
                })}
            </group>
        </group>
    );
}

export default forwardRef(Lake);
