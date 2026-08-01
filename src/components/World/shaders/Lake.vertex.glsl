uniform sampler2D uRadiusDeltaTex;
uniform sampler2D uYDeltaTex;
uniform float uThetaCount;
uniform float uRadialCount;
uniform float uInnerRadius;
uniform float uBaseRadius;

attribute float aThetaIndex;
attribute float aRadialIndex;
attribute vec2 aBaseDir;
attribute float aRadialWeight;

void main() {
    float thetaUv = (aThetaIndex + 0.5) / uThetaCount;
    float radialUv = (aRadialIndex + 0.5) / uRadialCount;

    float radiusDelta = texture2D(uRadiusDeltaTex, vec2(thetaUv, 0.5)).r;
    float yDelta = texture2D(uYDeltaTex, vec2(thetaUv, radialUv)).r;

    float targetRadius = uBaseRadius + radiusDelta;
    vec2 innerXZ = aBaseDir * uInnerRadius;
    vec2 outerXZ = aBaseDir * targetRadius;
    vec2 xz = mix(innerXZ, outerXZ, aRadialWeight);

    vec3 transformed = vec3(xz.x, position.y + yDelta, xz.y);
    vec4 mvPosition = modelViewMatrix * vec4(transformed, 1.0);

    gl_Position = projectionMatrix * mvPosition;
}
