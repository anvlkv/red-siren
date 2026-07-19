import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { PlaybackState } from "../../types/PlaybackState";

function Play() {
    const [playbackState, setPlaybackState] =
        useState<PlaybackState>("Stopped");
    useEffect(() => {
        (async () => {
            try {
                await invoke("create_synth");
            } catch (error) {
                console.error("Error creating synth:", error);
            }

            try {
                const next = await invoke<PlaybackState>("start_playback");
                setPlaybackState(next);
            } catch (error) {
                console.error("Error starting playback:", error);
            }
        })();
    }, []);
    return (
        <div>
            <h1>Play: {playbackState}</h1>
            {Array.from({ length: 7 }, (_, i) => (
                <WheelControls key={i} index={i} />
            ))}
            <div style={{ position: "absolute", top: "0px", right: "0px" }}>
                {/* <Visualizer /> */}
            </div>
        </div>
    );
}

function WheelControls({ index }: { index: number }) {
    const [speed, setSpeed] = useState(0);
    const [window, setWindow] = useState(0);

    return (
        <div>
            <h2>Wheel {index + 1}</h2>
            <div>
                <input
                    type="range"
                    min="0"
                    max="1024"
                    value={speed}
                    onChange={(e) => {
                        const newValue = parseInt(e.target.value);
                        setSpeed(newValue);
                        invoke("set_speed", {
                            chamberIndex: index,
                            speed: newValue,
                        }).catch((error) => {
                            console.error(
                                `Error setting wheel ${index + 1} value:`,
                                error,
                            );
                        });
                    }}
                />
                <span>speed: {speed}</span>
            </div>
            <div>
                <input
                    type="range"
                    min="0"
                    max="1024"
                    value={window}
                    onChange={(e) => {
                        const newValue = parseInt(e.target.value);
                        setWindow(newValue);
                        invoke("set_window", {
                            chamberIndex: index,
                            window: newValue,
                        }).catch((error) => {
                            console.error(
                                `Error setting wheel ${index + 1} window:`,
                                error,
                            );
                        });
                    }}
                />
                <span>window: {window}</span>
            </div>
            <button
                onClick={() => {
                    invoke("set_shape", { chamberIndex: index }).catch(
                        (error) => {
                            console.error(
                                `Error setting shape wheel ${index + 1}:`,
                                error,
                            );
                        },
                    );
                }}
            >
                Test call shape
            </button>
        </div>
    );
}

export default Play;
