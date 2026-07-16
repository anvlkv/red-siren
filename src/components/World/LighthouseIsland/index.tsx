import { useThree } from "@react-three/fiber";
import { useMemo } from "react";
import Lantern from "./Lantern";
import Level from "./Level";
import RockyIsland from "./RockyIsland";

function LighthouseIsland({
    rBase,
    height,
    nChambers,
}: {
    rBase: number;
    height: number;
    nChambers: number;
}) {
    const { invalidate } = useThree();
    const levels = useMemo(() => {
        invalidate();
        return Array.from({ length: nChambers }).reduce(
            (acc: { r1: number; r2: number }[], _, at) => {
                if (at === 0) {
                    acc.push({ r1: rBase, r2: rBase * 0.9 });
                } else {
                    const prev = acc[at - 1];
                    acc.push({ r1: prev.r2, r2: prev.r2 * 0.9 });
                }
                return acc;
            },
            [] as { r1: number; r2: number }[],
        );
    }, [nChambers, rBase]);

    const hSegment = height / (nChambers + 2);

    return (
        <group>
            <RockyIsland r={rBase * 2.5} height={hSegment * 3} />
            <group position={[0, hSegment / 3, 0]}>
                {levels.map(({ r1, r2 }, i) => (
                    <Level
                        key={i}
                        index={i}
                        r1={r1}
                        r2={r2}
                        levelHeight={hSegment}
                    />
                ))}
                <Lantern
                    r={levels.length ? levels[levels.length - 1].r2 : 1}
                    height={hSegment * 2}
                    tall={hSegment * nChambers}
                />
            </group>
        </group>
    );
}

export default LighthouseIsland;
