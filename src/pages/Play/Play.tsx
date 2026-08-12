import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { PlaybackState } from "../../types/PlaybackState";
import { useAnimationFrameFps } from "../../util";

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

function Visualizer() {
    const [snapshot, setSnapshot] = useState<[number, number][]>([]);
    const canvasRef = useRef<HTMLCanvasElement>(null);
    useAnimationFrameFps(
        30,
        async () => {
            if (canvasRef.current) {
                const ctx = canvasRef.current.getContext("2d");
                if (ctx) {
                    ctx.clearRect(
                        0,
                        0,
                        canvasRef.current.width,
                        canvasRef.current.height,
                    );

                    ctx.strokeStyle = "red";

                    ctx.beginPath();
                    snapshot.forEach(([left, _], index) => {
                        const x =
                            (index / snapshot.length) *
                            canvasRef.current!.width;
                        const y = ((left + 1) / 2) * canvasRef.current!.height;
                        if (index === 0) {
                            ctx.moveTo(x, y);
                        } else {
                            ctx.lineTo(x, y);
                        }
                    });
                    ctx.stroke();
                    ctx.closePath();

                    ctx.beginPath();
                    snapshot.forEach(([_, right], index) => {
                        const x =
                            (index / snapshot.length) *
                            canvasRef.current!.width;
                        const y = ((right + 1) / 2) * canvasRef.current!.height;
                        if (index === 0) {
                            ctx.moveTo(x, y);
                        } else {
                            ctx.lineTo(x, y);
                        }
                    });
                    ctx.stroke();
                    ctx.closePath();

                    ctx.strokeText(
                        `max: ${Math.max(...snapshot.map(([left, right]) => Math.max(left, right)))}, min: ${Math.min(...snapshot.map(([left, right]) => Math.min(left, right)))}`,
                        10,
                        20,
                    );
                }
            }
            try {
                const newSnapshot =
                    await invoke<[number, number][]>("get_snapshot");
                setSnapshot(newSnapshot);
            } catch (error) {
                console.error("Error getting snapshot:", error);
            }
        },
        [],
    );

    return (
        <canvas
            ref={canvasRef}
            width={800}
            height={400}
            style={{ border: "1px solid black" }}
        ></canvas>
    );
}

export default Play;
