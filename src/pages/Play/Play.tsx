import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { PlaybackState } from "../../types/PlaybackState";

function Play() {
    const [playbackState, setPlaybackState] = useState<PlaybackState>("Stopped");
    useEffect(() => {
        (async () => {
            try {
                await invoke("create_synth");
            }
            catch (error) {
                console.error("Error creating synth:", error);
            }
            
            try {
                const next = await invoke<PlaybackState>("start_playback");
                setPlaybackState(next);
            }
            catch (error) {
                console.error("Error starting playback:", error);
            }
        })();
    }, []);
    return <div>
        <h1>Play: {playbackState}</h1>
    </div>;
}

export default Play;