import LighthouseIsland from "./LighthouseIsland";
import { useStage } from "../Stage";
import Lake, { type YDeltaBatch } from "./Lake";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import * as log from "@tauri-apps/plugin-log";
import * as THREE from "three";
import * as dat from "dat.gui";
import { SirenConfig } from "../../types/SirenConfig";
import { useAnimationFrameFps } from "../../util";
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

const EMPTY_Y_DELTA_BATCHES: readonly YDeltaBatch[] = [];

function World() {
    const { setStageSegments, perimetryStore } = useStage();
    const [nChambers, setNChambers] = useState(3);

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

    useEffect(() => {
        const nextSegments = Math.max(3, 360 * nChambers);
        setStageSegments(nextSegments);
    }, [setStageSegments, nChambers]);

    const [yDeltaBatches, setYDeltaBatches] = useState<readonly YDeltaBatch[]>(
        EMPTY_Y_DELTA_BATCHES,
    );

    const updateRange = useRef({ start: 0, end: 32 });

    useEffect(() => {
        const gui = new dat.GUI();
        gui.add(updateRange.current, "start", 0, 2048).onChange((value) => {
            updateRange.current.start = value;
            if (updateRange.current.start >= updateRange.current.end) {
                updateRange.current.end = updateRange.current.start + 1;
            }
        });
        gui.add(updateRange.current, "end", 1, 2048).onChange((value) => {
            updateRange.current.end = value;
            if (updateRange.current.end <= updateRange.current.start) {
                updateRange.current.start = updateRange.current.end - 1;
            }
        });

        return () => {
            gui.destroy();
        };
    }, []);

    const batchId = useRef(0);
    useAnimationFrameFps(30, () => {
        const id = batchId.current++;
        (async () => {
            const data = await invoke<number[][]>("get_snapshot");
            setYDeltaBatches(() => {
                const deltaY = new Float32Array(data.flat());
                const newBatch: YDeltaBatch = {
                    start: updateRange.current.start,
                    end: updateRange.current.end,
                    id,
                    deltaY,
                };
                return [newBatch];
            });
        })();
    });

    const spotlightTargetRef = useRef<THREE.Object3D>(null);

    // perimetryStore.getPoint(i)
    return (
        <group>
            <LighthouseIsland rBase={10} height={75} nChambers={nChambers} />
            <Lake
                baseline={-10}
                phiSegments={1023}
                innerRadius={10}
                yDeltaBatches={yDeltaBatches}
            />
            <object3D
                ref={spotlightTargetRef}
                // position={[
                //     Math.cos(beamAngle) * 24,
                //     lampY - 0.65,
                //     Math.sin(beamAngle) * 24,
                // ]}
            />
        </group>
    );
}

export default World;
