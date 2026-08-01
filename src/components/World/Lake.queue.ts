import type { ThetaRange } from "../Stage";

export interface RangeQueue {
    ranges: ThetaRange[];
    rangeCursor: number;
    thetaCursor: number;
}

function mod(value: number, n: number) {
    const r = value % n;
    return r < 0 ? r + n : r;
}

function normalizeRange(
    start: number,
    end: number,
    thetaCount: number,
): ThetaRange[] {
    if (thetaCount <= 0) {
        return [];
    }

    const normalizedStart = mod(start, thetaCount);
    const normalizedEnd = mod(end, thetaCount);

    if (normalizedStart === normalizedEnd) {
        if (start === end) {
            return [];
        }
        return [{ start: 0, end: thetaCount }];
    }

    if (normalizedStart < normalizedEnd) {
        return [{ start: normalizedStart, end: normalizedEnd }];
    }

    return [
        { start: normalizedStart, end: thetaCount },
        { start: 0, end: normalizedEnd },
    ];
}

export function clearQueue(queue: RangeQueue) {
    queue.ranges.length = 0;
    queue.rangeCursor = 0;
    queue.thetaCursor = 0;
}

export function queueHasWork(queue: RangeQueue) {
    return queue.rangeCursor < queue.ranges.length;
}

export function enqueueRanges(
    queue: RangeQueue,
    ranges: ThetaRange[],
    thetaCount: number,
) {
    for (let r = 0; r < ranges.length; r += 1) {
        const normalized = normalizeRange(
            ranges[r].start,
            ranges[r].end,
            thetaCount,
        );
        for (let i = 0; i < normalized.length; i += 1) {
            queue.ranges.push(normalized[i]);
        }
    }
}

export function processQueue(
    queue: RangeQueue,
    maxThetas: number,
    apply: (theta: number) => void,
) {
    let remaining = maxThetas;

    while (remaining > 0) {
        if (queue.rangeCursor >= queue.ranges.length) {
            clearQueue(queue);
            break;
        }

        const range = queue.ranges[queue.rangeCursor];

        if (queue.thetaCursor < range.start || queue.thetaCursor >= range.end) {
            queue.thetaCursor = range.start;
        }

        const available = range.end - queue.thetaCursor;
        if (available <= 0) {
            queue.rangeCursor += 1;
            continue;
        }

        const take = available < remaining ? available : remaining;
        const startTheta = queue.thetaCursor;
        const endTheta = startTheta + take;

        for (let theta = startTheta; theta < endTheta; theta += 1) {
            apply(theta);
        }

        queue.thetaCursor = endTheta;
        remaining -= take;

        if (queue.thetaCursor >= range.end) {
            queue.rangeCursor += 1;
            if (queue.rangeCursor < queue.ranges.length) {
                queue.thetaCursor = queue.ranges[queue.rangeCursor].start;
            }
        }
    }
}
