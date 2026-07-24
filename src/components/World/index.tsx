import LighthouseIsland from "./LighthouseIsland";
import { useStage } from "../Stage";
import Lake, { type YDeltaBatch } from "./Lake";
import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import * as log from "@tauri-apps/plugin-log";
import { SirenConfig } from "../../types/SirenConfig";
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
    const { setStageSegments } = useStage();
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
        const nextSegments = Math.max(3, 360 * nChambers);
        setStageSegments(nextSegments);
    }, [setStageSegments, nChambers]);

    const yDeltaBatches = useMemo(() => EMPTY_Y_DELTA_BATCHES, []);

    return (
        <group>
            <LighthouseIsland rBase={10} height={75} nChambers={nChambers} />
            <Lake
                baseline={-10}
                phiSegments={256}
                innerRadius={10}
                yDeltaBatches={yDeltaBatches}
            />
        </group>
    );
}

export default World;
