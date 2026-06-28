import { useRef, useCallback, useEffect } from "react";


export function useAnimationFrameFps(fps: number, callback: (deltaTime: number) => void, deps: any[] = []) {
    const requestRef = useRef<number>(0);
    const previousTimeRef = useRef<number>(0);
    const fpsInterval = 1000 / fps;

    const animate = useCallback((time: number) => {
        if (previousTimeRef.current === 0) {
            previousTimeRef.current = time;
        }
        const deltaTime = time - previousTimeRef.current;

        if (deltaTime > fpsInterval) {
            previousTimeRef.current = time - (deltaTime % fpsInterval);
            callback(deltaTime);
        }

        requestRef.current = requestAnimationFrame(animate);
    }, [callback, fpsInterval]);

    useEffect(() => {
        requestRef.current = requestAnimationFrame(animate);
        return () => cancelAnimationFrame(requestRef.current!);
    }, [animate, ...deps]);
}