"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import AudioVisualizer from "./AudioVisualizer";

type DemoPhase = "idle" | "listening" | "processing" | "result";

const DEMO_TEXT =
  "Schedule a meeting with the design team for next Tuesday at 3 PM.";

export default function DemoTerminal() {
  const [phase, setPhase] = useState<DemoPhase>("idle");
  const [typedText, setTypedText] = useState("");
  const [isTypingDone, setIsTypingDone] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval>>(null);

  const cleanup = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    if (intervalRef.current) clearInterval(intervalRef.current);
  }, []);

  const startDemo = useCallback(() => {
    cleanup();
    setTypedText("");
    setIsTypingDone(false);
    setPhase("listening");

    timerRef.current = setTimeout(() => {
      setPhase("processing");
      timerRef.current = setTimeout(() => {
        setPhase("result");
        let i = 0;
        intervalRef.current = setInterval(() => {
          i++;
          if (i <= DEMO_TEXT.length) {
            setTypedText(DEMO_TEXT.slice(0, i));
          } else {
            if (intervalRef.current) clearInterval(intervalRef.current);
            setIsTypingDone(true);
          }
        }, 28);
      }, 1400);
    }, 2800);
  }, [cleanup]);

  useEffect(() => cleanup, [cleanup]);

  const resetDemo = useCallback(() => {
    cleanup();
    setPhase("idle");
    setTypedText("");
    setIsTypingDone(false);
  }, [cleanup]);

  return (
    <div className="t" style={{ width: "100%", maxWidth: 600 }}>
      <div className="t-bar">
        <div className="t-dots">
          <span className="t-dot t-dot-r" />
          <span className="t-dot t-dot-y" />
          <span className="t-dot t-dot-g" />
        </div>
        <span className="t-title">dictate — demo</span>
        {phase !== "idle" ? (
          <span
            className={`demo-st ${
              phase === "listening" ? "rec" : phase === "processing" ? "proc" : "ok"
            }`}
          >
            {phase === "listening" ? "● rec" : phase === "processing" ? "transcribing..." : "✓ done"}
          </span>
        ) : <span />}
      </div>

      <div className="t-body" style={{ minHeight: 180 }}>
        {phase === "idle" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div className="t-line">
              <span className="t-ps">$</span>
              <span className="t-cmd">dictate --pipe-to wl-copy</span>
            </div>
            <div className="t-out" style={{ marginBottom: 6 }}>waiting for SIGUSR1...</div>
            <button onClick={startDemo} className="demo-btn" id="demo-start-btn">
              Send SIGUSR1
            </button>
          </div>
        )}

        {phase === "listening" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div className="t-line">
              <span className="t-ps">$</span>
              <span className="t-cmd">dictate --pipe-to wl-copy</span>
            </div>
            <div className="t-cmt"># recording from pipewire...</div>
            <AudioVisualizer active barCount={56} />
          </div>
        )}

        {phase === "processing" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div className="t-line">
              <span className="t-ps">$</span>
              <span className="t-cmd">dictate --pipe-to wl-copy</span>
            </div>
            <div className="t-cmt"># sending to mistral...</div>
            <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "8px 0" }}>
              <div className="spinner" />
              <span style={{ color: "#9e9a90", fontSize: "0.8rem" }}>
                transcribing with voxtral-mini-latest
              </span>
            </div>
          </div>
        )}

        {phase === "result" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <div className="t-line">
              <span className="t-ps">$</span>
              <span className="t-cmd">dictate --pipe-to wl-copy</span>
            </div>
            <div className="demo-result">
              <div className="demo-result-label">stdout → wl-copy</div>
              <span style={{ color: "#e8e4dc" }}>
                {typedText}
                {!isTypingDone && <span className="cursor" />}
              </span>
            </div>
            {isTypingDone && (
              <button onClick={resetDemo} className="demo-btn-ghost" id="demo-reset-btn">
                ↺ run again
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
