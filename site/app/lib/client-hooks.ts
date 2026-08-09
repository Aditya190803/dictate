import { useSyncExternalStore } from "react";

/**
 * Browser values that live outside React — a media query, localStorage — are
 * external stores, so they are read with useSyncExternalStore rather than by
 * setting state inside an effect. That avoids the cascading render React 19
 * warns about, and gives a defined server snapshot so hydration matches.
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
const darkScheme = mediaQuery("(prefers-color-scheme: dark)");

/** False on the server, so the animated path is what gets hydrated. */
export function usePrefersReducedMotion(): boolean {
  return useSyncExternalStore(
    reducedMotion.subscribe,
    reducedMotion.get,
    () => false,
  );
}

export function usePrefersDark(): boolean {
  return useSyncExternalStore(darkScheme.subscribe, darkScheme.get, () => false);
}

/**
 * navigator.platform is deprecated but remains the most reliable signal
 * without UA-CH; the user agent is the fallback. Server snapshot is false so
 * the markup always hydrates as the Linux default.
 */
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

/** The explicit theme override, or null when following the system. */
export function useStoredTheme(): "light" | "dark" | null {
  return useSyncExternalStore(
    (onChange) => {
      if (typeof window === "undefined") return () => {};
      window.addEventListener("storage", onChange);
      window.addEventListener("themechange", onChange);
      return () => {
        window.removeEventListener("storage", onChange);
        window.removeEventListener("themechange", onChange);
      };
    },
    () => {
      try {
        const v = localStorage.getItem("theme");
        return v === "light" || v === "dark" ? v : null;
      } catch {
        return null;
      }
    },
    () => null,
  );
}
