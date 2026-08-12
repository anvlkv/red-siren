import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import * as log from "@tauri-apps/plugin-log";
import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { SirenConfig } from "../../types/SirenConfig";
import { useAnimationFrameFps } from "../../util";
import { useStage } from "../Stage";
import Lake, { LakeRef } from "./Lake";
import LighthouseIsland from "./LighthouseIsland";
import dat from "dat.gui";
// import LighthouseIsland from "./LighthouseIsland";
// import Mountain from "./Mountain";
// import Shore from "./Shore";

export enum WorldLookAt {
    Shore,
    LighthouseIsland,
    Mountain,
}

// const LIGHTHOUSE_POSITION: [number, number, number] = [0, 0, 0];
// const SHORE_POSITION: [number, number, number] = [0, 0, 26];
// const MOUNTAIN_POSITION: [number, number, number] = [-28, 0, -14];

// const CAMERA_PRESETS: Record<WorldLookAt, { position: [number, number, number]; target: [number, number, number] }> = {
//     [WorldLookAt.Shore]: {
//         position: [2, 9, -3],
//         target: SHORE_POSITION,
//     },
//     [WorldLookAt.LighthouseIsland]: {
//         position: [2, 7, 34],
//         target: LIGHTHOUSE_POSITION,
//     },
//     [WorldLookAt.Mountain]: {
//         position: [4, 9, -6],
//         target: MOUNTAIN_POSITION,
//     },
// };

function World() {
    const { setStageSegments, stageSegments } = useStage();
    const [nChambers, setNChambers] = useState(0);

    useEffect(() => {
        let unlisten: () => void;
        (async () => {
            unlisten = await listen<SirenConfig>(`siren-info`, (config) => {
                setNChambers(config.payload.n_chambers);
                log.debug(
                    `Received siren-info event: ${JSON.stringify(config.payload)}`,
                );
            });
            log.debug(`Listening for siren-info events`);
            await invoke("request_siren_info");
        })();

        return () => {
            unlisten && unlisten();
        };
    }, []);

    useEffect(
        () => setStageSegments(16 * nChambers),
        [setStageSegments, nChambers],
    );

    const lakeRef = useRef<LakeRef>(null);
    const snapshotRef = useRef<number[][]>([]);
    const updateRef = useRef<number>(0);

    useAnimationFrameFps(30, async () => {
        if (!lakeRef.current) return;
        const snap = snapshotRef.current.splice(0, nChambers);
        if (snap.length)
            lakeRef.current.geometry.update_t_range(
                updateRef.current,
                updateRef.current + snap.length,
                (i, b, c) =>
                    b.map((base, j) => {
                        const s = snap[i][j];
                        // const v = c[i][1] + s[0];
                        const v = s + base[1];
                        console.log(s, base, v);
                        // const v = base[1];

                        return new THREE.Vector3(base[0], v, base[2]);
                    }),
            );

        const nextSnapshot = await invoke<number[][]>("get_snapshot");

        snapshotRef.current.push(...nextSnapshot);
    });

    useEffect(() => {
        const gui = new dat.GUI();
        gui.add(updateRef, "current", 0, stageSegments, 1).name("update index");
        return () => {
            gui.destroy();
        };
    }, [stageSegments]);

    return (
        <group>
            <LighthouseIsland rBase={10} height={75} nChambers={nChambers} />
            <Lake
                baseline={-10}
                phiSegments={64}
                innerRadius={10}
                ref={lakeRef}
            />
        </group>
    );
}

export default World;
