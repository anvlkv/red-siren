import { Canvas } from "@react-three/fiber";
import {
    useCssVar,
    useDarkMode,
    useDevicePixelRatio,
    useWindowSize,
} from "@reactuses/core";
import { invoke } from "@tauri-apps/api/core";
import * as log from "@tauri-apps/plugin-log";
import classnames from "classnames";
import { useEffect, useRef, useState } from "react";
import { Outlet } from "react-router";
import * as THREE from "three";
import WebGPU from "three/addons/capabilities/WebGPU.js";
import { WebGPURenderer, WebGPURendererParameters } from "three/webgpu";
import Menu from "../components/Menu";
import World, { WorldLookAt } from "../components/World/World";
import ThemeProvider from "./Theme";
import "./App.css";
import Stage from "../components/Stage";

function App() {
    const [isDark, setIsDark] = useDarkMode({
        classNameDark: "dark",
        classNameLight: "light",
        defaultValue: false,
    });
    const { width, height } = useWindowSize();
    const { pixelRatio } = useDevicePixelRatio();
    const className = classnames("App", {
        dark: isDark,
        light: !isDark,
    });
    const mainRef = useRef<HTMLElement>(null);
    const [backgroundColor] = useCssVar("--color-background", mainRef);
    const [primaryColor] = useCssVar("--color-primary", mainRef);
    const [secondaryColor] = useCssVar("--color-secondary", mainRef);
    const [tertiaryColor] = useCssVar("--color-tertiary", mainRef);

    const [lookAt] = useState<WorldLookAt>(WorldLookAt.Shore);
    const isWebGPU = WebGPU.isAvailable();

    useEffect(() => {
        (async () => {
            await invoke("update_window_appearance", { width, height, isDark });
        })();
    }, [width, height, isDark]);

    return (
        <main className={className} ref={mainRef}>
            <ThemeProvider
                backgroundColor={backgroundColor}
                primaryColor={primaryColor}
                secondaryColor={secondaryColor}
                tertiaryColor={tertiaryColor}
                isDark={!!isDark}
                pixelRatio={pixelRatio}
                width={width}
                height={height}
            >
                <Canvas
                    gl={async (props) => {
                        if (isWebGPU) {
                            log.debug("WebGPU support claimed");
                            const renderer = new WebGPURenderer(
                                props as WebGPURendererParameters,
                            );
                            await renderer.init();
                            log.info("Using WebGPU renderer");
                            return renderer;
                        }
                        log.info("Using WebGL renderer");
                        return new THREE.WebGLRenderer(props);
                    }}
                    style={{
                        pointerEvents: "none",
                        position: "absolute",
                        top: 0,
                        left: 0,
                    }}
                    frameloop="demand"
                    role="img"
                >
                    <Stage lookAt={lookAt} rBase={300}>
                        <World />
                    </Stage>
                </Canvas>
                <Outlet />
                <aside>
                    <Menu setIsDark={setIsDark} />
                </aside>
            </ThemeProvider>
        </main>
    );
}

export default App;
