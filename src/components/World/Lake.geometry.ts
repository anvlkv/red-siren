import * as THREE from "three";

export interface LakeGeometryData {
    geometry: THREE.BufferGeometry;
    positionAttribute: THREE.BufferAttribute;
    positionArray: Float32Array;
    innerAnchor: Float32Array;
    radialWeights: Float32Array;
    radialCount: number;
    rayStride: number;
}

function smoothstep(t: number) {
    return t * t * (3 - 2 * t);
}

export function createThetaMajorRingGeometry(
    innerRadius: number,
    outerRadius: number,
    thetaCount: number,
    phiSegments: number,
): LakeGeometryData {
    const radialCount = phiSegments + 1;
    const seamThetaCount = thetaCount + 1;
    const vertexCount = seamThetaCount * radialCount;
    const rayStride = radialCount * 3;

    const positionArray = new Float32Array(vertexCount * 3);
    const normalArray = new Float32Array(vertexCount * 3);
    const uvArray = new Float32Array(vertexCount * 2);

    const thetaIndexArray = new Float32Array(vertexCount);
    const radialIndexArray = new Float32Array(vertexCount);
    const baseDirectionArray = new Float32Array(vertexCount * 2);
    const radialWeightArray = new Float32Array(vertexCount);

    const innerAnchor = new Float32Array(thetaCount * 3);
    const radialWeights = new Float32Array(radialCount);

    for (let j = 0; j < radialCount; j += 1) {
        const t = j / phiSegments;
        radialWeights[j] = smoothstep(t);
    }

    for (let i = 0; i <= thetaCount; i += 1) {
        const theta = i === thetaCount ? 0 : i;
        const angle = (theta / thetaCount) * Math.PI * 2;
        const cos = Math.cos(angle);
        const sin = Math.sin(angle);

        for (let j = 0; j < radialCount; j += 1) {
            const t = j / phiSegments;
            const radius = innerRadius + (outerRadius - innerRadius) * t;

            const vertexIndex = i * radialCount + j;
            const posOffset = vertexIndex * 3;
            const uvOffset = vertexIndex * 2;
            const dirOffset = vertexIndex * 2;

            positionArray[posOffset] = cos * radius;
            positionArray[posOffset + 1] = 0;
            positionArray[posOffset + 2] = sin * radius;

            normalArray[posOffset] = 0;
            normalArray[posOffset + 1] = 1;
            normalArray[posOffset + 2] = 0;

            uvArray[uvOffset] = i / thetaCount;
            uvArray[uvOffset + 1] = t;

            thetaIndexArray[vertexIndex] = theta;
            radialIndexArray[vertexIndex] = j;
            baseDirectionArray[dirOffset] = cos;
            baseDirectionArray[dirOffset + 1] = sin;
            radialWeightArray[vertexIndex] = radialWeights[j];

            if (i < thetaCount && j === 0) {
                const anchorOffset = i * 3;
                innerAnchor[anchorOffset] = positionArray[posOffset];
                innerAnchor[anchorOffset + 1] = positionArray[posOffset + 1];
                innerAnchor[anchorOffset + 2] = positionArray[posOffset + 2];
            }
        }
    }

    const indexArray = new Uint32Array(thetaCount * phiSegments * 6);
    let indexOffset = 0;

    for (let i = 0; i < thetaCount; i += 1) {
        for (let j = 0; j < phiSegments; j += 1) {
            const a = i * radialCount + j;
            const b = (i + 1) * radialCount + j;
            const c = (i + 1) * radialCount + (j + 1);
            const d = i * radialCount + (j + 1);

            indexArray[indexOffset] = a;
            indexArray[indexOffset + 1] = d;
            indexArray[indexOffset + 2] = b;
            indexArray[indexOffset + 3] = b;
            indexArray[indexOffset + 4] = d;
            indexArray[indexOffset + 5] = c;
            indexOffset += 6;
        }
    }

    const geometry = new THREE.BufferGeometry();
    const positionAttribute = new THREE.BufferAttribute(positionArray, 3);
    positionAttribute.setUsage(THREE.DynamicDrawUsage);

    geometry.setAttribute("position", positionAttribute);
    geometry.setAttribute("normal", new THREE.BufferAttribute(normalArray, 3));
    geometry.setAttribute("uv", new THREE.BufferAttribute(uvArray, 2));
    geometry.setAttribute(
        "aThetaIndex",
        new THREE.BufferAttribute(thetaIndexArray, 1),
    );
    geometry.setAttribute(
        "aRadialIndex",
        new THREE.BufferAttribute(radialIndexArray, 1),
    );
    geometry.setAttribute(
        "aBaseDir",
        new THREE.BufferAttribute(baseDirectionArray, 2),
    );
    geometry.setAttribute(
        "aRadialWeight",
        new THREE.BufferAttribute(radialWeightArray, 1),
    );
    geometry.setIndex(new THREE.BufferAttribute(indexArray, 1));
    geometry.computeBoundingSphere();

    return {
        geometry,
        positionAttribute,
        positionArray,
        innerAnchor,
        radialWeights,
        radialCount,
        rayStride,
    };
}
