import { useSyncExternalStore } from "react";

/*
 * Browser values that live outside React — media queries, UA — are external
 * stores, so they are read with useSyncExternalStore rather than by setting
 * state inside an effect. Server snapshots keep hydration matching.
 */

const noopSubscribe = () => () => {};

function mediaQuery(query: string) {
  return {
    subscribe(onChange: () => void) {
      if (typeof window === "undefined") return () => {};
      const mql = window.matchMedia(query);
      mql.addEventListener("change", onChange);
      return () => mql.removeEventListener("change", onChange);
    },
    get: () =>
      typeof window !== "undefined" && window.matchMedia(query).matches,
  };
}

const reducedMotion = mediaQuery("(prefers-reduced-motion: reduce)");

/** False on the server, so the animated path is what gets hydrated. */
export function usePrefersReducedMotion(): boolean {
  return useSyncExternalStore(
    reducedMotion.subscribe,
    reducedMotion.get,
    () => false,
  );
}

/** Server snapshot is false, so markup hydrates as the Linux default. */
export function useIsWindows(): boolean {
  return useSyncExternalStore(
    noopSubscribe,
    () => {
      if (typeof navigator === "undefined") return false;
      const ua = `${navigator.userAgent} ${navigator.platform ?? ""}`;
      return ua.toLowerCase().includes("win");
    },
    () => false,
  );
}
