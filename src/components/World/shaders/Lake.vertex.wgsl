fn lakeDeformPosition(
    radiusDelta: f32,
    yDelta: f32,
    baseDir: vec2<f32>,
    radialWeight: f32,
    baseY: f32,
    innerRadius: f32,
    baseRadius: f32
) -> vec3<f32> {
    let targetRadius = baseRadius + radiusDelta;
    let innerXZ = baseDir * innerRadius;
    let outerXZ = baseDir * targetRadius;
    let xz = mix(innerXZ, outerXZ, radialWeight);

    return vec3<f32>(xz.x, baseY + yDelta, xz.y);
}
