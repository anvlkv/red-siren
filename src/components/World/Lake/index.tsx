import { useFrame, useThree } from "@react-three/fiber";
import { useCallback, useEffect, useMemo, useRef } from "react";
import { createNoise2D } from "simplex-noise";
import * as THREE from "three";
import { MeshBasicNodeMaterial } from "three/webgpu";
import {
    attribute,
    color as tslColor,
    float,
    positionLocal,
    texture,
    vec2,
    wgslFn,
} from "three/tsl";
import type { ThetaRange } from "../../Stage";
import { useStage } from "../../Stage";
import { createThetaMajorRingGeometry } from "./geometry";
import LAKE_VERTEX_SHADER from "./shaders/Lake.vertex.glsl?raw";
import LAKE_FRAGMENT_SHADER from "./shaders/Lake.fragment.glsl?raw";
import LAKE_WEBGPU_VERTEX_SHADER from "./shaders/Lake.vertex.wgsl?raw";
import LAKE_WEBGPU_FRAGMENT_SHADER from "./shaders/Lake.fragment.wgsl?raw";
import {
    clearQueue,
    enqueueRanges,
    enqueueUploadSpan,
    processQueue,
    queueHasWork,
    type RangeQueue,
} from "./queue";
import type { LakeDeformationReadApi, LakeProps, YDeltaBatch } from "./types";

export type {
    LakeDeformationReadApi,
    LakeProps,
    RadiusDeltaBatch,
    YDeltaBatch,
} from "./types";

const EMPTY_Y_DELTA_BATCHES: readonly YDeltaBatch[] = [];

const lakeWebGpuDeformPositionFn = wgslFn(LAKE_WEBGPU_VERTEX_SHADER);
const lakeWebGpuColorFn = wgslFn(LAKE_WEBGPU_FRAGMENT_SHADER);

interface LakeTextureState {
    radiusTexture: THREE.DataTexture;
    radiusTexelData: Float32Array;
    yTexture: THREE.DataTexture;
    yTexelData: Float32Array;
}

function mod(value: number, n: number) {
    const r = value % n;
    return r < 0 ? r + n : r;
}

function normalizeThetaIndex(theta: number, thetaCount: number) {
    if (!Number.isFinite(theta)) {
        throw new Error(`theta must be a finite number`);
    }
    return mod(Math.floor(theta), thetaCount);
}

function normalizeRadialIndex(radial: number, radialCount: number) {
    if (!Number.isInteger(radial) || radial < 0 || radial >= radialCount) {
        throw new Error(
            `radial must be an integer in [0, ${radialCount - 1}], got ${radial}`,
        );
    }
    return radial;
}

function createRangeQueue(): RangeQueue {
    return {
        ranges: [],
        rangeCursor: 0,
        thetaCursor: 0,
    };
}

function createFloatTexture(width: number, height: number, data: Float32Array) {
    const texture = new THREE.DataTexture(
        data,
        width,
        height,
        THREE.RGBAFormat,
        THREE.FloatType,
    );
    texture.generateMipmaps = false;
    texture.magFilter = THREE.NearestFilter;
    texture.minFilter = THREE.NearestFilter;
    texture.wrapS = THREE.ClampToEdgeWrapping;
    texture.wrapT = THREE.ClampToEdgeWrapping;
    texture.needsUpdate = true;
    return texture;
}

function fillRandomYNoise(
    target: Float32Array,
    thetaCount: number,
    radialCount: number,
) {
    const noiseA = createNoise2D();
    const noiseB = createNoise2D();

    for (let theta = 0; theta < thetaCount; theta += 1) {
        for (let radial = 0; radial < radialCount; radial += 1) {
            const t = radial / Math.max(1, radialCount - 1);
            const outerWeight = t * t;
            const idx = theta * radialCount + radial;

            const lowFreq = noiseA(theta * 0.018, radial * 0.03) * 2.2;
            const ripple =
                noiseB(theta * 0.09 + 7.1, radial * 0.12 - 3.8) * 0.55;
            target[idx] = (lowFreq + ripple) * outerWeight;
        }
    }
}

interface WebGpuRendererLike {
    isWebGPURenderer: true;
}

function isWebGpuRenderer(renderer: unknown): renderer is WebGpuRendererLike {
    return (
        typeof renderer === "object" &&
        renderer !== null &&
        "isWebGPURenderer" in renderer &&
        (renderer as { isWebGPURenderer?: boolean }).isWebGPURenderer === true
    );
}

function supportsWebGlGpuDeformation(renderer: unknown) {
    if (!(renderer instanceof THREE.WebGLRenderer)) {
        return false;
    }

    const capabilities = renderer.capabilities;
    const hasFloatTextureSupport =
        capabilities.isWebGL2 || renderer.extensions.has("OES_texture_float");

    return hasFloatTextureSupport && capabilities.maxVertexTextures > 0;
}

function Lake({
    baseline,
    innerRadius,
    phiSegments,
    yDeltaBatches = EMPTY_Y_DELTA_BATCHES,
    raySliceSizePerFrame = 128,
    deformationReadApiRef,
}: LakeProps) {
    const { gl, invalidate } = useThree();
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

    const { radialCount } = geometryData;

    const isWebGpuDeformationEnabled = useMemo(
        () => isWebGpuRenderer(gl),
        [gl],
    );
    const isWebGlGpuDeformationEnabled = useMemo(
        () => supportsWebGlGpuDeformation(gl),
        [gl],
    );
    const isAnyGpuDeformationEnabled =
        isWebGpuDeformationEnabled || isWebGlGpuDeformationEnabled;

    const textureState = useMemo<LakeTextureState | null>(() => {
        if (!isAnyGpuDeformationEnabled) {
            return null;
        }

        const radiusTexelData = new Float32Array(stageSegments * 4);
        const yTexelData = new Float32Array(stageSegments * radialCount * 4);

        const radiusTexture = createFloatTexture(
            stageSegments,
            1,
            radiusTexelData,
        );
        const yTexture = createFloatTexture(
            stageSegments,
            radialCount,
            yTexelData,
        );

        return {
            radiusTexture,
            radiusTexelData,
            yTexture,
            yTexelData,
        };
    }, [isAnyGpuDeformationEnabled, radialCount, stageSegments]);

    const webGlGpuMaterial = useMemo<THREE.ShaderMaterial | null>(() => {
        if (!isWebGlGpuDeformationEnabled || textureState === null) {
            return null;
        }

        return new THREE.ShaderMaterial({
            uniforms: {
                uRadiusDeltaTex: { value: textureState.radiusTexture },
                uYDeltaTex: { value: textureState.yTexture },
                uThetaCount: { value: stageSegments },
                uRadialCount: { value: radialCount },
                uInnerRadius: { value: innerRadius },
                uBaseRadius: { value: rBase },
                uColor: { value: new THREE.Color("white") },
            },
            vertexShader: LAKE_VERTEX_SHADER,
            fragmentShader: LAKE_FRAGMENT_SHADER,
        });
    }, [
        innerRadius,
        isWebGlGpuDeformationEnabled,
        radialCount,
        rBase,
        stageSegments,
        textureState,
    ]);

    const webGpuMaterial = useMemo<MeshBasicNodeMaterial | null>(() => {
        if (!isWebGpuDeformationEnabled || textureState === null) {
            return null;
        }

        const thetaIndex = attribute("aThetaIndex", "float") as any;
        const radialIndex = attribute("aRadialIndex", "float") as any;
        const baseDir = attribute("aBaseDir", "vec2") as any;
        const radialWeight = attribute("aRadialWeight", "float") as any;

        const thetaUv = thetaIndex.add(float(0.5)).div(float(stageSegments));
        const radialUv = radialIndex.add(float(0.5)).div(float(radialCount));

        const radiusDelta = texture(
            textureState.radiusTexture,
            vec2(thetaUv, float(0.5)),
        ).x;
        const yDelta = texture(
            textureState.yTexture,
            vec2(thetaUv, radialUv),
        ).x;

        const material = new MeshBasicNodeMaterial();
        material.positionNode = lakeWebGpuDeformPositionFn({
            radiusDelta,
            yDelta,
            baseDir,
            radialWeight,
            baseY: positionLocal.y,
            innerRadius: float(innerRadius),
            baseRadius: float(rBase),
        });
        material.colorNode = lakeWebGpuColorFn({
            baseColor: tslColor(new THREE.Color("white")),
        }) as never;

        return material;
    }, [
        innerRadius,
        isWebGpuDeformationEnabled,
        radialCount,
        rBase,
        stageSegments,
        textureState,
    ]);

    const interpolationQueueRef = useRef<RangeQueue>(createRangeQueue());
    const radiusUploadQueueRef = useRef<RangeQueue>(createRangeQueue());
    const yUploadQueueRef = useRef<RangeQueue>(createRangeQueue());

    const radiusDeltaByThetaRef = useRef<Float32Array>(
        new Float32Array(stageSegments),
    );
    const yDeltaByThetaRadialRef = useRef<Float32Array>(
        new Float32Array(stageSegments * radialCount),
    );

    const appliedYBatchIdsRef = useRef(new Set<number>());
    const lastYBatchIdRef = useRef(Number.NEGATIVE_INFINITY);

    const enqueueInterpolationRanges = useCallback(
        (ranges: ThetaRange[]) => {
            enqueueRanges(interpolationQueueRef.current, ranges, stageSegments);
        },
        [stageSegments],
    );

    const syncRadiusOffsetAtTheta = useCallback(
        (theta: number) => {
            const base = perimetryStore.getBasePoint(theta);
            const current = perimetryStore.getPoint(theta);

            const baseRadius = Math.hypot(base[0], base[2]);
            if (!Number.isFinite(baseRadius) || baseRadius <= 0) {
                throw new Error(
                    `Invalid base radius at theta ${theta}: ${baseRadius}`,
                );
            }

            const dirX = base[0] / baseRadius;
            const dirZ = base[2] / baseRadius;
            const projectedOuterRadius = current[0] * dirX + current[2] * dirZ;
            const delta = projectedOuterRadius - baseRadius;

            if (!Number.isFinite(delta)) {
                throw new Error(
                    `Computed non-finite radius delta at theta ${theta}: ${delta}`,
                );
            }

            radiusDeltaByThetaRef.current[theta] = delta;
        },
        [perimetryStore],
    );

    const syncRadiusOffsetsForRanges = useCallback(
        (ranges: ThetaRange[]) => {
            for (let r = 0; r < ranges.length; r += 1) {
                const range = ranges[r];
                for (let theta = range.start; theta < range.end; theta += 1) {
                    syncRadiusOffsetAtTheta(theta);
                }
            }
        },
        [syncRadiusOffsetAtTheta],
    );

    const deformationReadApi = useMemo<LakeDeformationReadApi>(
        () => ({
            getRadiusOffset: (theta: number) => {
                const normalizedTheta = normalizeThetaIndex(
                    theta,
                    stageSegments,
                );
                return radiusDeltaByThetaRef.current[normalizedTheta];
            },
            getYAt: (theta: number, radial: number) => {
                const normalizedTheta = normalizeThetaIndex(
                    theta,
                    stageSegments,
                );
                const normalizedRadial = normalizeRadialIndex(
                    radial,
                    radialCount,
                );
                return (
                    yDeltaByThetaRadialRef.current[
                        normalizedTheta * radialCount + normalizedRadial
                    ] ?? 0
                );
            },
            getYRay: (theta: number) => {
                const normalizedTheta = normalizeThetaIndex(
                    theta,
                    stageSegments,
                );
                const start = normalizedTheta * radialCount;
                return yDeltaByThetaRadialRef.current.subarray(
                    start,
                    start + radialCount,
                );
            },
        }),
        [radialCount, stageSegments],
    );

    useEffect(() => {
        if (!deformationReadApiRef) {
            return;
        }

        deformationReadApiRef.current = deformationReadApi;

        return () => {
            if (deformationReadApiRef.current === deformationReadApi) {
                deformationReadApiRef.current = null;
            }
        };
    }, [deformationReadApi, deformationReadApiRef]);

    useEffect(() => {
        radiusDeltaByThetaRef.current = new Float32Array(stageSegments);

        const nextYMirror = new Float32Array(stageSegments * radialCount);
        if (yDeltaBatches.length === 0) {
            fillRandomYNoise(nextYMirror, stageSegments, radialCount);
        }
        yDeltaByThetaRadialRef.current = nextYMirror;

        appliedYBatchIdsRef.current.clear();

        clearQueue(interpolationQueueRef.current);
        clearQueue(radiusUploadQueueRef.current);
        clearQueue(yUploadQueueRef.current);

        const fullRange = [{ start: 0, end: stageSegments }];
        syncRadiusOffsetsForRanges(fullRange);
        enqueueInterpolationRanges(fullRange);
        enqueueRanges(radiusUploadQueueRef.current, fullRange, stageSegments);
        enqueueRanges(yUploadQueueRef.current, fullRange, stageSegments);

        invalidate();
    }, [
        enqueueInterpolationRanges,
        invalidate,
        radialCount,
        stageSegments,
        syncRadiusOffsetsForRanges,
    ]);

    useEffect(() => {
        const unsubscribe = perimetryStore.subscribe((dirtyRanges) => {
            enqueueInterpolationRanges(dirtyRanges);
            syncRadiusOffsetsForRanges(dirtyRanges);
            enqueueRanges(
                radiusUploadQueueRef.current,
                dirtyRanges,
                stageSegments,
            );
            invalidate();
        });

        return () => {
            unsubscribe();
        };
    }, [
        enqueueInterpolationRanges,
        invalidate,
        perimetryStore,
        stageSegments,
        syncRadiusOffsetsForRanges,
    ]);

    useEffect(() => {
        if (yDeltaBatches.length === 0) {
            return;
        }

        const seenInProp = new Set<number>();
        let previousId = Number.NEGATIVE_INFINITY;

        for (let i = 0; i < yDeltaBatches.length; i += 1) {
            const batch = yDeltaBatches[i];

            if (!Number.isInteger(batch.id) || batch.id < 0) {
                throw new Error(
                    `YDeltaBatch.id at index ${i} must be a non-negative integer`,
                );
            }

            if (!Number.isInteger(batch.start)) {
                throw new Error(
                    `YDeltaBatch.start at index ${i} must be an integer`,
                );
            }

            if (!(batch.deltaY instanceof Float32Array)) {
                throw new Error(
                    `YDeltaBatch.deltaY at index ${i} must be a Float32Array`,
                );
            }

            if (seenInProp.has(batch.id)) {
                throw new Error(
                    `Duplicate YDeltaBatch id ${batch.id} in prop payload`,
                );
            }

            if (batch.id <= previousId) {
                throw new Error(
                    `YDeltaBatch ids must be strictly increasing: got ${batch.id} after ${previousId}`,
                );
            }

            seenInProp.add(batch.id);
            previousId = batch.id;
        }

        const consumedIds = appliedYBatchIdsRef.current;
        let ingestedAny = false;

        for (let i = 0; i < yDeltaBatches.length; i += 1) {
            const batch = yDeltaBatches[i];

            if (consumedIds.has(batch.id)) {
                continue;
            }

            if (batch.id <= lastYBatchIdRef.current) {
                throw new Error(
                    `Stale or non-monotonic YDeltaBatch id ${batch.id}; last applied id is ${lastYBatchIdRef.current}`,
                );
            }

            if (batch.deltaY.length === 0) {
                throw new Error(`YDeltaBatch.deltaY must not be empty`);
            }

            if (batch.deltaY.length % radialCount !== 0) {
                throw new Error(
                    `YDeltaBatch.deltaY length ${batch.deltaY.length} is not divisible by radialCount ${radialCount}`,
                );
            }

            const rayCount = batch.deltaY.length / radialCount;
            if (rayCount <= 0) {
                throw new Error(`YDeltaBatch has invalid rayCount ${rayCount}`);
            }

            if (rayCount > stageSegments) {
                throw new Error(
                    `YDeltaBatch rayCount ${rayCount} exceeds thetaCount ${stageSegments}`,
                );
            }

            for (let k = 0; k < batch.deltaY.length; k += 1) {
                if (!Number.isFinite(batch.deltaY[k])) {
                    throw new Error(
                        `YDeltaBatch.deltaY[${k}] for id ${batch.id} must be finite`,
                    );
                }
            }

            const yMirror = yDeltaByThetaRadialRef.current;

            for (let rayOffset = 0; rayOffset < rayCount; rayOffset += 1) {
                const theta = mod(batch.start + rayOffset, stageSegments);
                const srcBase = rayOffset * radialCount;
                const dstBase = theta * radialCount;

                for (let radial = 0; radial < radialCount; radial += 1) {
                    yMirror[dstBase + radial] += batch.deltaY[srcBase + radial];
                }
            }

            enqueueUploadSpan(
                yUploadQueueRef.current,
                batch.start,
                rayCount,
                stageSegments,
            );

            consumedIds.add(batch.id);
            lastYBatchIdRef.current = batch.id;
            ingestedAny = true;
        }

        if (ingestedAny) {
            invalidate();
        }
    }, [invalidate, radialCount, stageSegments, yDeltaBatches]);

    useEffect(() => {
        return () => {
            geometryData.geometry.dispose();
        };
    }, [geometryData.geometry]);

    useEffect(() => {
        return () => {
            if (textureState !== null) {
                textureState.radiusTexture.dispose();
                textureState.yTexture.dispose();
            }
        };
    }, [textureState]);

    useEffect(() => {
        return () => {
            webGlGpuMaterial?.dispose();
            webGpuMaterial?.dispose();
        };
    }, [webGlGpuMaterial, webGpuMaterial]);

    useFrame(() => {
        const {
            positionArray,
            innerAnchor,
            radialWeights,
            rayStride,
            radialCount: frameRadialCount,
            positionAttribute,
        } = geometryData;

        const useGpu = isAnyGpuDeformationEnabled && textureState !== null;

        let changedPosition = false;
        let seamNeedsMirror = false;
        let changedRadiusTexture = false;
        let changedYTexture = false;

        processQueue(interpolationQueueRef.current, safeSliceSize, (theta) => {
            if (useGpu) {
                return;
            }

            const outer = perimetryStore.getPoint(theta);
            const innerOffset = theta * 3;
            const rayStart = theta * rayStride;

            const innerX = innerAnchor[innerOffset];
            const innerZ = innerAnchor[innerOffset + 2];
            const deltaX = outer[0] - innerX;
            const deltaZ = outer[2] - innerZ;

            let posOffset = rayStart;
            for (let j = 0; j < frameRadialCount; j += 1) {
                const w = radialWeights[j];
                positionArray[posOffset] = innerX + deltaX * w;
                positionArray[posOffset + 2] = innerZ + deltaZ * w;
                posOffset += 3;
            }

            changedPosition = true;
            if (theta === 0) {
                seamNeedsMirror = true;
            }
        });

        processQueue(radiusUploadQueueRef.current, safeSliceSize, (theta) => {
            if (!useGpu || textureState === null) {
                return;
            }

            const value = radiusDeltaByThetaRef.current[theta];
            const texelOffset = theta * 4;

            textureState.radiusTexelData[texelOffset] = value;
            textureState.radiusTexelData[texelOffset + 1] = 0;
            textureState.radiusTexelData[texelOffset + 2] = 0;
            textureState.radiusTexelData[texelOffset + 3] = 1;

            changedRadiusTexture = true;
        });

        processQueue(yUploadQueueRef.current, safeSliceSize, (theta) => {
            const yMirror = yDeltaByThetaRadialRef.current;

            if (useGpu && textureState !== null) {
                let texelOffset = theta * frameRadialCount * 4;
                let mirrorOffset = theta * frameRadialCount;

                for (let radial = 0; radial < frameRadialCount; radial += 1) {
                    const value = yMirror[mirrorOffset + radial];
                    textureState.yTexelData[texelOffset] = value;
                    textureState.yTexelData[texelOffset + 1] = 0;
                    textureState.yTexelData[texelOffset + 2] = 0;
                    textureState.yTexelData[texelOffset + 3] = 1;
                    texelOffset += 4;
                }

                changedYTexture = true;
                return;
            }

            const rayStart = theta * rayStride;
            let posOffset = rayStart + 1;
            let mirrorOffset = theta * frameRadialCount;

            for (let radial = 0; radial < frameRadialCount; radial += 1) {
                positionArray[posOffset] = yMirror[mirrorOffset + radial];
                posOffset += 3;
            }

            changedPosition = true;
            if (theta === 0) {
                seamNeedsMirror = true;
            }
        });

        if (!useGpu && seamNeedsMirror) {
            const seamRayStart = perimetryStore.thetaCount * rayStride;
            for (let k = 0; k < rayStride; k += 1) {
                positionArray[seamRayStart + k] = positionArray[k];
            }
            changedPosition = true;
        }

        if (!useGpu && changedPosition) {
            positionAttribute.needsUpdate = true;
        }

        if (useGpu && textureState !== null) {
            if (changedRadiusTexture) {
                textureState.radiusTexture.needsUpdate = true;
            }
            if (changedYTexture) {
                textureState.yTexture.needsUpdate = true;
            }
        }

        if (
            queueHasWork(interpolationQueueRef.current) ||
            queueHasWork(radiusUploadQueueRef.current) ||
            queueHasWork(yUploadQueueRef.current)
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
            {webGpuMaterial ? (
                <primitive object={webGpuMaterial} attach="material" />
            ) : webGlGpuMaterial ? (
                <primitive object={webGlGpuMaterial} attach="material" />
            ) : (
                <meshStandardMaterial
                    color="white"
                    roughness={0.25}
                    metalness={0.15}
                />
            )}
        </mesh>
    );
}

export default Lake;
