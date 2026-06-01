"use client";

import { useRef, useEffect, useState } from "react";
import Link from "next/link";
import { gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import {
  Lightning,
  Plugs,
  Broadcast,
  Stack,
  Monitor,
  ShieldCheck,
} from "@phosphor-icons/react";
import CopyButton from "./components/CopyButton";

gsap.registerPlugin(ScrollTrigger);

const CURL_CMD = "curl -fsSL https://dictate.adityamer.dev/install.sh | sh";

const AGENT_PROMPT = `Read https://dictate.adityamer.dev/INSTALL.md and follow it step by step to install and configure dictate on this machine. Ask me the setup questions first, then execute everything non-interactively using 'dictate config set'.`;

const FLOW_STEPS = [
  { num: "01", title: "Bind a key", desc: "Assign a keybind in your Wayland compositor. Hyprland, Niri, GNOME, KDE, Sway.", code: "bind = SUPER, R, exec, ..." },
  { num: "02", title: "Speak", desc: "dictate records from PipeWire. A beep confirms recording has started.", code: "♫ recording started" },
  { num: "03", title: "Signal", desc: "Press the keybind again. SIGUSR1 stops realtime or finishes a batch clip.", code: "pkill --signal SIGUSR1 dictate" },
  { num: "04", title: "Get text", desc: "Transcribed text is piped to stdout. Send it to clipboard, type it, or pipe it anywhere.", code: "stdout → wl-copy | ydotool" },
];

const FEATURES = [
  { icon: Lightning, title: "Signal-driven", desc: "SIGUSR1 triggers transcription. No polling, no wasted cycles. The process sleeps until you need it." },
  { icon: Plugs, title: "Pipe anywhere", desc: "--pipe-to sends output to wl-copy, ydotool, sed, or any command. Compose however you like." },
  { icon: Broadcast, title: "Realtime by default", desc: "Mistral Voxtral realtime WebSocket STT from the keyboard shortcut. BATCH_MODE=true opts out." },
  { icon: Stack, title: "Multi-provider", desc: "Mistral (default), Groq, or local Whisper. Choose what works for your setup and privacy needs." },
  { icon: Monitor, title: "Wayland native", desc: "PipeWire audio capture. Works with Hyprland, Niri, GNOME, KDE, Sway, and more." },
  { icon: ShieldCheck, title: "Privacy option", desc: "Local Whisper mode. Your audio never leaves your machine. Download GGML models and transcribe offline." },
];

export default function Home() {
  const [mounted, setMounted] = useState(false);
  const containerRef = useRef<HTMLElement>(null);

  const heroH1Ref = useRef<HTMLHeadingElement>(null);
  const heroDescRef = useRef<HTMLParagraphElement>(null);
  const heroWaveformRef = useRef<HTMLDivElement>(null);
  const heroInstallRef = useRef<HTMLDivElement>(null);
  const heroAgentRef = useRef<HTMLDivElement>(null);
  const heroScriptRef = useRef<HTMLDivElement>(null);
  const howHeadRef = useRef<HTMLDivElement>(null);
  const flowRef = useRef<HTMLDivElement>(null);
  const featHeadRef = useRef<HTMLDivElement>(null);
  const bentoRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setMounted(true);
  }, []);

  useEffect(() => {
    if (!mounted || typeof window === "undefined") return;
    const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (prefersReducedMotion) return;

    const ctx = gsap.context(() => {
      // Left column: h1 → desc → waveform
      const heroTextEls = [heroH1Ref.current, heroDescRef.current, heroWaveformRef.current].filter(Boolean);
      if (heroTextEls.length > 0) {
        gsap.fromTo(heroTextEls,
          { opacity: 0, y: 32 },
          { opacity: 1, y: 0, duration: 0.8, stagger: 0.12, ease: "power3.out", clearProps: "all" }
        );
      }

      // Right column: install panel slides up with scale
      if (heroInstallRef.current) {
        gsap.fromTo(heroInstallRef.current,
          { opacity: 0, y: 40, scale: 0.97 },
          { opacity: 1, y: 0, scale: 1, duration: 0.9, delay: 0.3, ease: "power3.out", clearProps: "all" }
        );
      }

      const heads = [howHeadRef.current, featHeadRef.current].filter(Boolean);
      heads.forEach((el) => {
        gsap.fromTo(el,
          { opacity: 0, y: 20 },
          { opacity: 1, y: 0, duration: 0.6, ease: "power2.out",
            scrollTrigger: { trigger: el, start: "top 85%", toggleActions: "play none none none" },
            clearProps: "all"
          }
        );
      });

      if (flowRef.current) {
        gsap.fromTo(flowRef.current.children,
          { opacity: 0, y: 28 },
          { opacity: 1, y: 0, duration: 0.55, stagger: 0.12, ease: "power2.out",
            scrollTrigger: { trigger: flowRef.current, start: "top 80%", toggleActions: "play none none none" },
            clearProps: "all"
          }
        );
      }

      if (bentoRef.current) {
        gsap.fromTo(bentoRef.current.children,
          { opacity: 0, y: 20, scale: 0.97 },
          { opacity: 1, y: 0, scale: 1, duration: 0.5, stagger: 0.08, ease: "power2.out",
            scrollTrigger: { trigger: bentoRef.current, start: "top 80%", toggleActions: "play none none none" },
            clearProps: "all"
          }
        );
      }
    }, containerRef);

    return () => ctx.revert();
  }, [mounted]);

  return (
    <main ref={containerRef} className="overflow-x-hidden w-full max-w-full">
      {/* ── NAV ── */}
      <nav className="nav" id="nav">
        <Link href="/" className="nav-logo">dictate</Link>
        <div className="nav-links">
          <a href="#how" className="nav-link hm">How it works</a>
          <a href="#features" className="nav-link hm">Features</a>
          <a href="https://github.com/Aditya190803/dictate" target="_blank" rel="noopener noreferrer" className="nav-github">
            <svg width="18" height="18" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
            </svg>
            <span>GitHub</span>
          </a>
        </div>
      </nav>

      {/* ── HERO ── */}
      <header className="hero">
        {/* Decorative floating orbs */}
        <div className="hero-orb" aria-hidden="true" />
        <div className="hero-orb-sm" aria-hidden="true" />

        <div className="wrap hero-wrap">
          <div className="hero-text">
            <h1 ref={heroH1Ref}>
              Voice to text,<br />
              from your <span>terminal.</span>
            </h1>

            <p ref={heroDescRef} className="hero-desc">
              A signal-driven CLI for Wayland Linux. Mistral realtime STT by default.
              One keybind. Speak. Get text. No GUI, no daemon.
            </p>

            {/* Waveform animation */}
            <div ref={heroWaveformRef} className="hero-waveform">
              <div className="waveform-bars">
                {Array.from({ length: 8 }).map((_, i) => (
                  <div key={i} className="waveform-bar" />
                ))}
              </div>
              <span className="waveform-label">voice → text</span>
            </div>
          </div>

          <div ref={heroInstallRef} className="hero-install">
            <div className="install-panel">
              <div ref={heroAgentRef} className="install-section">
                <div className="install-section-header">
                  <span className="install-badge rec">recommended</span>
                  <span className="install-section-title">AI agent</span>
                </div>
                <div className="install-prompt-block">
                  <CopyButton text={AGENT_PROMPT} label="copy" id="hero-agent-copy" />
                  <pre>{AGENT_PROMPT}</pre>
                </div>
                <div className="install-tags">
                  {["Claude Code", "Cursor", "Copilot", "Windsurf", "Gemini CLI"].map((a) => (
                    <span className="install-tag" key={a}>{a}</span>
                  ))}
                </div>
              </div>

              <div className="install-panel-divider" />

              <div ref={heroScriptRef} className="install-section">
                <div className="install-section-header">
                  <span className="install-badge alt">manual</span>
                  <span className="install-section-title">Install script</span>
                </div>
                <div className="install-cmd-bar">
                  <code>{CURL_CMD}</code>
                  <CopyButton text={CURL_CMD} label="copy" id="hero-curl-copy" />
                </div>
                <div className="install-tags">
                  {["Arch", "Ubuntu", "Fedora", "Debian", "openSUSE", "Alpine", "Void"].map((d) => (
                    <span className="install-tag" key={d}>{d}</span>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      </header>

      {/* ── HOW IT WORKS ── */}
      <div className="sec-line" />
      <section className="sec" id="how">
        <div className="wrap">
          <div ref={howHeadRef} className="sec-head">
            <span className="sec-head-label">// flow</span>
            <h2>How it works</h2>
            <p>A single UNIX signal controls the entire flow. No daemon polling, no wasted resources.</p>
          </div>

          <div ref={flowRef} className="flow">
            {FLOW_STEPS.map((s) => (
              <div className="flow-card" key={s.num}>
                <div className="flow-num">{s.num}</div>
                <div className="flow-title">{s.title}</div>
                <div className="flow-desc">{s.desc}</div>
                <div className="flow-snippet">{s.code}</div>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* ── FEATURES ── */}
      <div className="sec-line" />
      <section className="sec" id="features">
        <div className="wrap">
          <div ref={featHeadRef} className="sec-head">
            <span className="sec-head-label">// features</span>
            <h2>Built for the terminal</h2>
            <p>A UNIX citizen. Composable. Zero runtime overhead when idle.</p>
          </div>

          <div ref={bentoRef} className="bento">
            {FEATURES.map((f) => {
              const Icon = f.icon;
              return (
                <div className="bento-card" key={f.title}>
                  <div className="bento-icon">
                    <Icon weight="duotone" size={28} color="var(--accent)" />
                  </div>
                  <div className="bento-t">{f.title}</div>
                  <div className="bento-d">{f.desc}</div>
                </div>
              );
            })}
          </div>
        </div>
      </section>

      {/* ── FOOTER ── */}
      <footer className="foot">
        <div className="foot-inner">
          <div className="foot-brand">
            <div className="foot-logo">
              dictate
            </div>
            <div className="foot-tech">
              Rust<span>·</span>PipeWire<span>·</span>Wayland<span>·</span>GPL-3.0
            </div>
          </div>
          <div className="foot-links">
            <a href="https://github.com/Aditya190803/dictate" target="_blank" rel="noopener noreferrer">GitHub</a>
            <a href="https://github.com/Aditya190803/dictate/releases" target="_blank" rel="noopener noreferrer">Releases</a>
          </div>
        </div>
      </footer>
    </main>
  );
}
