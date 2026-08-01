import type { MutableRefObject } from "react";

export interface RadiusDeltaBatch {
    id: number;
    start: number;
    deltaRadius: Float32Array;
}

export interface YDeltaBatch {
    id: number;
    start: number;
    deltaY: Float32Array;
}

export interface LakeDeformationReadApi {
    getRadiusOffset(theta: number): number;
    getYAt(theta: number, radial: number): number;
    getYRay(theta: number): Float32Array;
}

export interface LakeProps {
    baseline: number;
    innerRadius: number;
    phiSegments: number;
    yDeltaBatches?: readonly YDeltaBatch[];
    raySliceSizePerFrame?: number;
    deformationReadApiRef?: MutableRefObject<LakeDeformationReadApi | null>;
}
