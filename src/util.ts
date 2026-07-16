import { useRef, useCallback, useEffect, useLayoutEffect } from "react";

export function useAnimationFrameFps(
    fps: number,
    callback: (deltaTime: number) => void,
    deps: any[] = [],
) {
    const requestRef = useRef<number>(0);
    const previousTimeRef = useRef<number>(0);
    const fpsInterval = 1000 / fps;

    const animate = useCallback(
        (time: number) => {
            if (previousTimeRef.current === 0) {
                previousTimeRef.current = time;
            }
            const deltaTime = time - previousTimeRef.current;

            if (deltaTime > fpsInterval) {
                previousTimeRef.current = time - (deltaTime % fpsInterval);
                callback(deltaTime);
            }

            requestRef.current = requestAnimationFrame(animate);
        },
        [callback, fpsInterval],
    );

    useEffect(() => {
        requestRef.current = requestAnimationFrame(animate);
        return () => cancelAnimationFrame(requestRef.current!);
    }, [animate, ...deps]);
}

export function useElement<E extends HTMLElement>(el: string) {
    const ref = useRef<E | null>(null);
    useLayoutEffect(() => {
        const element = document.createElement(el);
        ref.current = element as E;
        return () => {
            if (ref.current) {
                ref.current.remove();
            }
        };
    }, []);

    return ref;
}
