import { useMemo } from "react"
import { createNoise2D } from "simplex-noise"
import * as THREE from "three"

function Mountain({lookedAt = false}: {lookedAt?: boolean}) {
    const geometry = useMemo(() => {
        const geo = new THREE.PlaneGeometry(56, 56, 128, 128)
        const pos = geo.attributes.position

        const noise2D = createNoise2D()

        for (let i = 0; i < pos.count; i++) {
            const x = pos.getX(i)
            const y = pos.getY(i)

            const d = Math.sqrt(x * x + y * y)

            const mountain =
                Math.exp(-(d * d) / 420)

            const height =
                mountain * 22 +
                noise2D(x * 0.07, y * 0.07) * 2.8

            pos.setZ(i, height)
        }

        geo.computeVertexNormals()

        return geo
    }, [])

    return (
        <mesh geometry={geometry} rotation-x={-Math.PI / 2} position-y={-1} castShadow receiveShadow>
            <meshStandardMaterial color={lookedAt ? "#6e7d57" : "#5f6a53"} roughness={0.9} />
        </mesh>
    )
}

export default Mountain;