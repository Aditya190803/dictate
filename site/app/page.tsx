"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import CopyButton from "./components/CopyButton";
import HeroDemo from "./components/HeroDemo";
import InstallTabs from "./components/InstallTabs";
import Reveal from "./components/Reveal";
import ThemeToggle from "./components/ThemeToggle";

const REPO = "https://github.com/Aditya190803/dictate";

const AGENT_PROMPT =
  "Read https://dictate.adityamer.dev/INSTALL.md and follow it step by step to install and configure dictate on this machine. Ask me the setup questions first, then execute everything non-interactively using 'dictate config set'.";

const STEPS = [
  {
    n: "01",
    t: "Bind a key",
    d: "One shortcut per mode, registered by your compositor on Linux or by the resident agent on Windows.",
  },
  {
    n: "02",
    t: "Speak",
    d: "A warm daemon already holds the model and the socket, so there is no cold start. A beep confirms capture.",
  },
  {
    n: "03",
    t: "Press again",
    d: "SIGUSR1 on Unix, a named pipe on Windows. The same key stops the take and finalises the transcript.",
  },
  {
    n: "04",
    t: "Text lands",
    d: "Typed into the focused window, pasted, copied, or written to stdout for anything else to consume.",
  },
];

const PROVIDERS = [
  { name: "Mistral", model: "voxtral-mini-transcribe-realtime", realtime: true, local: false },
  { name: "Deepgram", model: "nova-3", realtime: true, local: false },
  { name: "Groq", model: "whisper-large-v3-turbo", realtime: false, local: false },
  { name: "Whisper", model: "ggml · on-device", realtime: false, local: true },
];

const PROFILES = [
  {
    t: "Live typing",
    d: "Words appear in the focused window as you speak, over a realtime WebSocket.",
  },
  {
    t: "Smart paste",
    d: "Speak, stop, and a single polished block is pasted once — better for long-form.",
  },
  {
    t: "Segmented",
    d: "Pause-bound chunks with session context and per-segment polish. The default.",
  },
  {
    t: "Batch clip",
    d: "Record, transcribe once, apply local cleanup only. No LLM in the loop.",
  },
];

const FEATURES = [
  {
    t: "Stdout first",
    d: "--pipe-to hands text to any command. Compose it with wl-copy, ydotool, sed, or your own script.",
  },
  {
    t: "Two platforms, one config",
    d: "The same keys work on both. Only the platform glue differs: PipeWire and ydotool on Wayland, WASAPI and Win32 on Windows.",
  },
  {
    t: "Idle costs nothing",
    d: "The daemon sleeps until a shortcut arrives. No polling loop, no background transcription.",
  },
  {
    t: "Polish is separate",
    d: "A text model cleans up transcripts independently of speech recognition — OpenCode Zen, Mistral, or a local Ollama.",
  },
  {
    t: "Your vocabulary",
    d: "A dictionary of names, tools, and common mishearings is applied before the text ever reaches the screen.",
  },
  {
    t: "Fully offline option",
    d: "Local Whisper keeps audio on the machine. Nothing is uploaded, and no key is needed.",
  },
];

export default function Home() {
  const [stuck, setStuck] = useState(false);

  useEffect(() => {
    const onScroll = () => setStuck(window.scrollY > 8);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <>
      <a href="#main" className="skip">Skip to content</a>

      <nav className="nav" data-stuck={stuck}>
        <div className="nav-inner">
          <Link href="/" className="brand">
            <span className="brand-caret" aria-hidden="true">›</span>
            dictate
          </Link>
          <div className="nav-right">
            <a href="#install" className="nav-link opt">Install</a>
            <a href="#how" className="nav-link opt">How it works</a>
            <a href="#providers" className="nav-link opt">Providers</a>
            <ThemeToggle />
            <a
              href={REPO}
              target="_blank"
              rel="noopener noreferrer"
              className="icon-btn"
              aria-label="View dictate on GitHub"
            >
              <svg width="18" height="18" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
                <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
              </svg>
            </a>
          </div>
        </div>
      </nav>

      <main id="main">
        {/* ── Hero ── */}
        <header className="hero">
          <div className="wrap hero-grid">
            <div>
              <Reveal as="div">
                <span className="eyebrow">Speech to text, for people who live in a terminal</span>
              </Reveal>

              <Reveal as="h1" delay={70}>
                Press a key.<br />
                Talk. The words are <em>already there.</em>
              </Reveal>

              <Reveal as="p" className="hero-sub" delay={140}>
                A daemon-friendly dictation CLI for Wayland Linux and Windows.
                Realtime transcription lands straight in the focused window —
                no GUI, no cloud account required, no waiting for a model to load.
              </Reveal>

              <Reveal as="div" className="hero-meta" delay={210}>
                <span className="chip"><strong>Rust</strong></span>
                <span className="chip">Wayland <strong>·</strong> Windows</span>
                <span className="chip">4 STT providers</span>
                <span className="chip">GPL-3.0</span>
              </Reveal>
            </div>

            <Reveal delay={260}>
              <HeroDemo />
            </Reveal>
          </div>
        </header>

        {/* ── Install ── */}
        <section className="sec" id="install">
          <div className="wrap">
            <Reveal className="sec-head">
              <span className="eyebrow">Install</span>
              <h2>Running in about a minute</h2>
              <p>
                One command, then <code className="mono">dictate setup</code> walks
                through providers and shortcuts. <code className="mono">dictate doctor</code>{" "}
                checks the whole chain — keys, mic, permissions, daemons — and tells
                you precisely what is missing.
              </p>
            </Reveal>

            <Reveal delay={80}>
              <InstallTabs />
            </Reveal>

            <Reveal delay={160} style={{ marginTop: 40 }}>
              <span className="eyebrow">Or hand it to your agent</span>
              <div className="cmd" style={{ marginTop: 14, alignItems: "flex-start" }}>
                <code style={{ whiteSpace: "pre-wrap", lineHeight: 1.6 }}>
                  {AGENT_PROMPT}
                </code>
                <CopyButton text={AGENT_PROMPT} id="copy-agent" />
              </div>
              <p className="note">
                Works with Claude Code, Cursor, Copilot, Windsurf, and Gemini CLI.
              </p>
            </Reveal>
          </div>
        </section>

        {/* ── How ── */}
        <section className="sec" id="how">
          <div className="wrap">
            <Reveal className="sec-head">
              <span className="eyebrow">How it works</span>
              <h2>One key, held open by a warm daemon</h2>
              <p>
                The model and the socket stay resident between takes, so the first
                word is transcribed as fast as the hundredth.
              </p>
            </Reveal>

            <div className="steps">
              {STEPS.map((s, i) => (
                <Reveal key={s.n} className="step" delay={i * 70}>
                  <div className="step-n">{s.n}</div>
                  <div className="step-t">{s.t}</div>
                  <div className="step-d">{s.d}</div>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* ── Providers ── */}
        <section className="sec" id="providers">
          <div className="wrap">
            <Reveal className="sec-head">
              <span className="eyebrow">Providers</span>
              <h2>Pick your trade-off, not ours</h2>
              <p>
                Realtime providers stream words as you speak. Batch providers wait
                for a pause and return the whole take. Local Whisper does neither
                over the network — audio never leaves the machine.
              </p>
            </Reveal>

            <Reveal className="matrix-scroll">
              <table className="matrix">
                <thead>
                  <tr>
                    <th scope="col">Provider</th>
                    <th scope="col">Model</th>
                    <th scope="col">Realtime</th>
                    <th scope="col">On device</th>
                  </tr>
                </thead>
                <tbody>
                  {PROVIDERS.map((p) => (
                    <tr key={p.name}>
                      <td className="name">{p.name}</td>
                      <td className="model">{p.model}</td>
                      <td>
                        <span className={`tick ${p.realtime ? "yes" : "no"}`}>
                          {p.realtime ? "streaming" : "on pause"}
                        </span>
                      </td>
                      <td>
                        <span className={`tick ${p.local ? "yes" : "no"}`}>
                          {p.local ? "yes" : "cloud"}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </Reveal>

            <Reveal className="sec-head" delay={80} style={{ marginTop: 68, marginBottom: 30 }}>
              <span className="eyebrow">Profiles</span>
              <h2>Four ways to put words on screen</h2>
            </Reveal>

            <div className="feats">
              {PROFILES.map((p, i) => (
                <Reveal key={p.t} className="feat" delay={i * 60}>
                  <div className="feat-t">{p.t}</div>
                  <div className="feat-d">{p.d}</div>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* ── Features ── */}
        <section className="sec" id="features">
          <div className="wrap">
            <Reveal className="sec-head">
              <span className="eyebrow">Design</span>
              <h2>A UNIX citizen that happens to hear</h2>
              <p>
                Text goes to stdout unless you ask otherwise. Everything else is
                composition.
              </p>
            </Reveal>

            <div className="feats">
              {FEATURES.map((f, i) => (
                <Reveal key={f.t} className="feat" delay={i * 55}>
                  <div className="feat-t">{f.t}</div>
                  <div className="feat-d">{f.d}</div>
                </Reveal>
              ))}
            </div>
          </div>
        </section>

        {/* ── CTA ── */}
        <section className="sec">
          <div className="wrap cta">
            <Reveal as="h2">Stop typing what you could say.</Reveal>
            <Reveal as="p" delay={70}>
              Free and GPL-3.0. Bring your own key, or run Whisper locally and
              bring nothing at all.
            </Reveal>
            <Reveal className="cta-cmd" delay={140}>
              <InstallTabs />
            </Reveal>
            <Reveal className="btn-row" delay={200}>
              <a href={REPO} target="_blank" rel="noopener noreferrer" className="btn primary">
                Source on GitHub
              </a>
              <a href={`${REPO}/releases`} target="_blank" rel="noopener noreferrer" className="btn">
                Releases
              </a>
              <a href="/INSTALL.md" className="btn">Install guide</a>
            </Reveal>
          </div>
        </section>
      </main>

      <footer className="foot">
        <div className="wrap foot-inner">
          <div className="foot-meta">
            <span>Rust</span><span aria-hidden="true">·</span>
            <span>PipeWire</span><span aria-hidden="true">·</span>
            <span>WASAPI</span><span aria-hidden="true">·</span>
            <span>GPL-3.0</span>
          </div>
          <div className="foot-links">
            <a href={REPO} target="_blank" rel="noopener noreferrer">GitHub</a>
            <a href={`${REPO}/releases`} target="_blank" rel="noopener noreferrer">Releases</a>
            <a href="/INSTALL.md">Docs</a>
          </div>
        </div>
      </footer>
    </>
  );
}
