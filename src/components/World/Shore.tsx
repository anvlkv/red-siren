function Shore({lookedAt = false}: {lookedAt?: boolean}) {
    return (
        <group>
            <mesh rotation={[-Math.PI / 2, 0, 0]} receiveShadow>
                <circleGeometry args={[10, 40]} />
                <meshStandardMaterial color={lookedAt ? "#f2cb7f" : "#deb56b"} />
            </mesh>
            <mesh position={[0, 0.8, 0]} castShadow>
                <sphereGeometry args={[3.8, 20, 20]} />
                <meshStandardMaterial color="#cda866" />
            </mesh>
            <mesh position={[4.8, 0.5, -2.8]} castShadow>
                <sphereGeometry args={[2.2, 16, 16]} />
                <meshStandardMaterial color="#d8b373" />
            </mesh>
        </group>
    )
}

export default Shore;