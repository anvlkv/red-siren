import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState, useMemo, useEffect } from "react";
import * as THREE from "three";
import * as log from "@tauri-apps/plugin-log";
import { useAppTheme } from "../../../App/Theme";
import { ChamberInfo } from "../../../types/ChamberInfo";
import { useElement } from "../../../util";
import { useThree } from "@react-three/fiber";

function Level({
    index,
    r1,
    r2,
    levelHeight,
}: {
    index: number;
    r1: number;
    r2: number;
    levelHeight: number;
}) {
    const { invalidate } = useThree();
    const {
        primaryColor,
        backgroundColor,
        secondaryColor,
        tertiaryColor,
        pixelRatio,
    } = useAppTheme();
    const [chamberInfo, setChamberInfo] = useState(null as ChamberInfo | null);
    const isEven = index % 2 === 0;

    const textureCanvas = useElement<HTMLCanvasElement>("canvas");

    const texture = useMemo(() => {
        const canvas = textureCanvas.current;

        if (chamberInfo && canvas) {
            const TEXELS_PER_WORLD_UNIT = 96;
            const dpr = Math.min(pixelRatio, 2);
            const avgRadius = (r1 + r2) * 0.5;
            const circumference = 2 * Math.PI * avgRadius;

            const targetWidth = Math.max(
                64,
                Math.round(circumference * TEXELS_PER_WORLD_UNIT * dpr),
            );
            const targetHeight = Math.max(
                32,
                Math.round(levelHeight * TEXELS_PER_WORLD_UNIT * dpr),
            );

            const resolution = Math.max(1, chamberInfo.resolution);
            const columnWidth = Math.max(
                1,
                Math.round(targetWidth / resolution),
            );

            canvas.width = columnWidth * resolution;
            canvas.height = targetHeight;

            const ctx = canvas.getContext("2d")!;
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            ctx.fillStyle = isEven ? primaryColor : backgroundColor;
            ctx.fillRect(0, 0, canvas.width, canvas.height);

            ctx.fillStyle = isEven ? tertiaryColor : secondaryColor;
            chamberInfo.wheel.forEach((val, i) => {
                const valHeight = Math.round(
                    canvas.height * Math.abs(val) * 0.75,
                );
                const x = Math.round(i * columnWidth);
                const y = Math.round((canvas.height - valHeight) / 2);

                ctx.fillRect(x, y, columnWidth, valHeight);
            });
        }

        const tex = new THREE.CanvasTexture(canvas);
        tex.magFilter = THREE.NearestFilter;
        tex.minFilter = THREE.LinearFilter;
        tex.generateMipmaps = false;

        invalidate();

        return tex;
    }, [
        chamberInfo,
        pixelRatio,
        r1,
        r2,
        levelHeight,
        primaryColor,
        backgroundColor,
        secondaryColor,
        tertiaryColor,
        isEven,
    ]);

    useEffect(
        () => () => {
            texture.dispose();
        },
        [texture],
    );

    useEffect(() => {
        let unlisten: () => void;
        (async () => {
            unlisten = await listen<ChamberInfo>(
                `chamber-${index}-info`,
                (shape) => {
                    setChamberInfo(shape.payload);
                    log.debug(
                        `Received chamber-${index}-info event: ${JSON.stringify(shape.payload)}`,
                    );
                },
            );
            log.debug(`Listening for chamber-${index}-info events`);
            await invoke("request_chamber_info", { chamberIndex: index });
        })();

        return () => {
            unlisten && unlisten();
        };
    }, [index]);

    return (
        <mesh
            position={[0, index * levelHeight + levelHeight * 0.5, 0]}
            castShadow
        >
            <cylinderGeometry args={[r2, r1, levelHeight, 28]} />
            <meshStandardMaterial map={texture} />
        </mesh>
    );
}

export default Level;
