"use client";

import { useEffect, useRef, useState } from "react";
import { useIsWindows, usePrefersReducedMotion } from "../lib/client-hooks";

/**
 * The hero demo runs the product's actual loop: press a key, speak, watch
 * text land in the focused app. It is the whole interface, so it is worth
 * showing literally rather than decorating around.
 *
 * Everything is driven by one timeline of phases. Nothing here fakes a
 * capability the tool does not have — the transcript below is what
 * `live_typing` genuinely does: words arrive in chunks as you speak.
 */

const BARS = 22;

const SENTENCE =
  "Ship the release notes, then reply to the thread about the audio pipeline.";

/**
 * The shipped defaults, per platform. Windows reserves Win+R for the Run
 * dialog — RegisterHotKey refuses it — so the Windows default is Ctrl+Alt+R,
 * matching `default_shortcut_keys()` in the CLI.
 */
const KEYS = {
  linux: ["Super", "R"],
  windows: ["Ctrl", "Alt", "R"],
} as const;

type Phase = "idle" | "armed" | "listening" | "settling" | "done";

export default function HeroDemo() {
  const reduced = usePrefersReducedMotion();
  const keys = useIsWindows() ? KEYS.windows : KEYS.linux;

  const [phase, setPhase] = useState<Phase>("idle");
  const [typed, setTyped] = useState("");
  const [amps, setAmps] = useState<number[]>(() => new Array(BARS).fill(0));
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const raf = useRef<number | undefined>(undefined);

  useEffect(() => {
    if (reduced) return;

    const after = (ms: number, fn: () => void) => {
      timers.current.push(setTimeout(fn, ms));
    };

    const run = () => {
      setTyped("");
      setPhase("idle");

      after(900, () => setPhase("armed"));
      after(1500, () => setPhase("listening"));

      // Words arrive in groups, the way a realtime transcript actually lands.
      const words = SENTENCE.split(" ");
      let at = 2100;
      let i = 0;
      while (i < words.length) {
        const take = 2 + Math.floor(Math.random() * 3);
        const slice = words.slice(0, i + take).join(" ");
        after(at, () => setTyped(slice));
        at += 300 + Math.random() * 260;
        i += take;
      }

      after(at + 260, () => setPhase("settling"));
      after(at + 900, () => setPhase("done"));
      after(at + 4200, () => {
        timers.current.forEach(clearTimeout);
        timers.current = [];
        run();
      });
    };

    run();
    const pending = timers.current;
    return () => {
      pending.forEach(clearTimeout);
      timers.current = [];
    };
  }, [reduced]);

  // Waveform. Amplitude follows the phase so it reads as a response to
  // speech rather than an idle decoration.
  useEffect(() => {
    if (reduced) return;

    let last = 0;
    const tick = (t: number) => {
      raf.current = requestAnimationFrame(tick);
      if (t - last < 68) return;
      last = t;

      setAmps((prev) =>
        prev.map((_, i) => {
          if (phase !== "listening") return 0;
          // Louder toward the centre, like a real level meter.
          const centre = 1 - Math.abs(i - BARS / 2) / (BARS / 2);
          return Math.max(0.06, Math.random() * (0.35 + centre * 0.65));
        }),
      );
    };
    raf.current = requestAnimationFrame(tick);
    return () => {
      if (raf.current !== undefined) cancelAnimationFrame(raf.current);
    };
  }, [phase, reduced]);

  // With motion suppressed the demo shows its finished state instead of
  // animating toward it, so the panel still communicates the same thing.
  const shownText = reduced ? SENTENCE : typed;
  const shownAmps = reduced ? new Array(BARS).fill(0) : amps;
  const shownPhase: Phase = reduced ? "done" : phase;

  const status =
    shownPhase === "listening"
      ? "listening · realtime stream"
      : shownPhase === "settling"
        ? "polishing"
        : shownPhase === "done"
          ? "typed into focused window"
          : "warm daemon · idle";

  return (
    <div
      className="demo"
      role="img"
      aria-label={`Demonstration: pressing ${keys.join(" plus ")} dictates the sentence "${SENTENCE}" into the focused window.`}
    >
      <div className="demo-bar">
        <div className="demo-dots" aria-hidden="true">
          <span className="demo-dot" />
          <span className="demo-dot" />
          <span className="demo-dot" />
        </div>
        <span className="demo-title">live typing</span>
      </div>

      <div className="demo-body">
        <div className="demo-row" aria-hidden="true">
          {keys.map((k, i) => (
            <span key={k} style={{ display: "contents" }}>
              {i > 0 && <span className="demo-hint">+</span>}
              <kbd className="keycap" data-pressed={shownPhase === "armed"}>
                {k}
              </kbd>
            </span>
          ))}
          <span className="demo-hint">
            {shownPhase === "idle" ? "press to start" : ""}
          </span>
        </div>

        <div
          className="wave"
          data-live={shownPhase === "listening"}
          aria-hidden="true"
        >
          {shownAmps.map((a, i) => (
            <span
              key={i}
              className="wave-bar"
              style={{ "--amp": a } as React.CSSProperties}
            />
          ))}
        </div>

        <div className="demo-out">
          <span className="demo-out-label">focused window</span>
          {shownText}
          {(shownPhase === "listening" || shownPhase === "settling") && (
            <span className="caret" aria-hidden="true" />
          )}
        </div>

        <div className="demo-status">
          {shownPhase === "listening" && (
            <span className="dot-live" aria-hidden="true" />
          )}
          {status}
        </div>
      </div>
    </div>
  );
}
