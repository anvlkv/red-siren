import type { RefObject } from "react";
import type { ThetaRange } from "../Stage";

export type PositionRef = RefObject<Float32Array | null>;

export interface LakeProps {
    baseline: number;
    innerRadius: number;
    phiSegments: number;
    yCopySourceRef?: PositionRef;
    raySliceSizePerFrame?: number;
}

export interface LakeHandle {
    enqueueYCopyRange(start: number, end: number): void;
    enqueueYCopyRanges(ranges: ThetaRange[]): void;
}
