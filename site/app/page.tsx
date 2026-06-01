import Link from "next/link";
import InstallSection from "./components/InstallSection";
import ScrollReveal from "./components/ScrollReveal";
import CopyButton from "./components/CopyButton";
import GSAPAnimations from "./components/GSAPAnimations";

const INSTALL_MD_URL =
  "https://dictate.adityamer.dev/INSTALL.md";

const AGENT_PROMPT = `Read ${INSTALL_MD_URL} and follow it step by step to install and configure dictate on this machine. Ask me the setup questions first, then execute everything non-interactively using 'dictate config set'.`;

export default function Home() {
  return (
    <main className="overflow-x-hidden w-full max-w-full">
      <GSAPAnimations />
      {/* ── NAV ── */}
      <nav className="nav" id="nav">
        <Link href="/" className="nav-logo">dictate</Link>
        <div className="nav-links">
          <a href="#how" className="nav-link hm">How it works</a>
          <a href="#features" className="nav-link hm">Features</a>
          <a
            href="https://github.com/Aditya190803/dictate"
            target="_blank"
            rel="noopener noreferrer"
            className="nav-link"
          >
            GitHub
          </a>
          <a href="#install" className="nav-cta" id="nav-install">Install</a>
        </div>
      </nav>

      {/* ── HERO — agent prompt as primary CTA ── */}
      <header className="hero">
        <div className="wrap">
          <div className="hero-center">
            <div className="hero-meta">
              <span className="badge">Rust</span>
              <span className="badge">PipeWire</span>
              <span className="badge">Wayland</span>
            </div>

            <h1>
              Voice to text,
              <br />
              from your <span>terminal.</span>
            </h1>

            <p className="hero-desc">
              <strong>dictate</strong> is a signal-driven CLI with Mistral
              realtime STT by default. Keyboard shortcuts start realtime unless
              you set BATCH_MODE=true for whole-clip batch. One keybind. Done.
            </p>

            <div className="hero-prompt-section">
              <p className="hero-prompt-label">
                Give this prompt to your coding agent to install and setup dictate
              </p>
              <div className="prompt-block">
                <div className="prompt-block-text">{AGENT_PROMPT}</div>
                <CopyButton text={AGENT_PROMPT} label="copy prompt" id="hero-copy" />
              </div>
              <div className="hero-prompt-agents">
                {["Claude Code", "Cursor", "Copilot", "Windsurf", "Gemini CLI"].map(
                  (a) => (
                    <span className="badge" key={a}>{a}</span>
                  )
                )}
              </div>
            </div>

            <div className="hero-or">
              <span className="hero-or-line" />
              <span className="hero-or-text">or install manually</span>
              <span className="hero-or-line" />
            </div>

            <div className="cmd-block">
              <code>curl -fsSL https://dictate.adityamer.dev/install.sh | sh</code>
              <CopyButton
                text="curl -fsSL https://dictate.adityamer.dev/install.sh | sh"
                label="copy"
                id="hero-curl-copy"
              />
            </div>
          </div>
        </div>
      </header>

      {/* ── HOW IT WORKS ── */}
      <div className="sec-line" />
      <section className="sec" id="how">
        <div className="wrap">
          <ScrollReveal>
            <div className="sec-head">
              <h2>How it works</h2>
              <p>A single UNIX signal controls the entire flow. No daemon polling, no wasted resources.</p>
            </div>
          </ScrollReveal>

          <ScrollReveal stagger>
            <div className="flow">
              {[
                { num: "01", title: "Bind a key", desc: "Assign a keybind in your Wayland compositor — Hyprland, Niri, GNOME, KDE.", code: "bind = SUPER, R, exec, ..." },
                { num: "02", title: "Speak", desc: "dictate records from PipeWire. Audio beeps confirm recording start.", code: "♫ recording started" },
                { num: "03", title: "Toggle", desc: "Press again. Mistral realtime stops on SIGUSR1; batch mode uses the same signal to finish a clip.", code: "pkill --signal SIGUSR1 dictate" },
                { num: "04", title: "Get text", desc: "Transcribed text piped to stdout — clipboard, direct typing, or any command.", code: "stdout → wl-copy | ydotool" },
              ].map((s) => (
                <div className="flow-card reveal" key={s.num}>
                  <div className="flow-num">{s.num}</div>
                  <div className="flow-title">{s.title}</div>
                  <div className="flow-desc">{s.desc}</div>
                  <div className="flow-snippet">{s.code}</div>
                </div>
              ))}
            </div>
          </ScrollReveal>
        </div>
      </section>

      {/* ── FEATURES ── */}
      <div className="sec-line" />
      <section className="sec" id="features">
        <div className="wrap">
          <ScrollReveal>
            <div className="sec-head">
              <h2>Built for the terminal</h2>
              <p>A UNIX citizen. Composable. Zero runtime overhead when idle.</p>
            </div>
          </ScrollReveal>

          <ScrollReveal stagger>
            <div className="bento">
              {[
                { title: "Signal-driven", desc: "SIGUSR1 triggers transcription. No polling, no wasted cycles. The process sleeps until you need it." },
                { title: "Pipe anywhere", desc: "--pipe-to sends output to wl-copy, ydotool, sed, or any command. Compose however you like." },
                { title: "Audio feedback", desc: "Musical beeps confirm recording start, stop, and success. Configurable volume or fully disabled." },
                { title: "Realtime by default", desc: "Mistral uses Voxtral realtime WebSocket STT from the normal keyboard shortcut. BATCH_MODE=true opts out to whole-clip transcription." },
                { title: "Multi-provider", desc: "Mistral (default), Groq, or local Whisper. Groq and local stream modes stay on VAD chunking because they do not use the Mistral realtime API." },
                { title: "Wayland native", desc: "PipeWire audio capture. Works with Hyprland, Niri, GNOME, KDE, Sway, and more." },
                { title: "Privacy option", desc: "Local Whisper mode — your audio never leaves your machine. Download GGML models and transcribe offline." },
              ].map((f) => (
                <div className="bento-card reveal" key={f.title}>
                  <div className="bento-t">{f.title}</div>
                  <div className="bento-d">{f.desc}</div>
                </div>
              ))}
            </div>
          </ScrollReveal>
        </div>
      </section>

      {/* ── INSTALL (detailed) ── */}
      <div className="sec-line" />
      <section className="sec" id="install">
        <div className="wrap">
          <ScrollReveal>
            <div className="sec-head">
              <h2>Get started</h2>
              <p>More ways to install and configure dictate.</p>
            </div>
          </ScrollReveal>

          <ScrollReveal>
            <InstallSection />
          </ScrollReveal>
        </div>
      </section>

      {/* ── REFERENCE ── */}
      <div className="sec-line" />
      <section className="sec" id="reference">
        <div className="wrap">
          <ScrollReveal>
            <div className="sec-head">
              <h2>Quick reference</h2>
              <p>The commands you&apos;ll actually use.</p>
            </div>
          </ScrollReveal>

          <ScrollReveal>
            <div className="ref-grid">
              <div className="ref-item">
                <div className="ref-label">Copy to clipboard</div>
                <div className="cmd-block">
                  <code>dictate --pipe-to wl-copy</code>
                  <CopyButton text="dictate --pipe-to wl-copy" label="copy" id="ref-1" />
                </div>
              </div>
              <div className="ref-item">
                <div className="ref-label">Type into focused window</div>
                <div className="cmd-block">
                  <code>dictate --pipe-to ydotool type --file -</code>
                  <CopyButton text="dictate --pipe-to ydotool type --file -" label="copy" id="ref-2" />
                </div>
              </div>
              <div className="ref-item">
                <div className="ref-label">Trigger transcription</div>
                <div className="cmd-block">
                  <code>pkill --signal SIGUSR1 dictate</code>
                  <CopyButton text="pkill --signal SIGUSR1 dictate" label="copy" id="ref-3" />
                </div>
              </div>
              <div className="ref-item">
                <div className="ref-label">Configure provider</div>
                <div className="cmd-block">
                  <code>dictate config set provider mistral</code>
                  <CopyButton text="dictate config set provider mistral" label="copy" id="ref-4" />
                </div>
              </div>
              <div className="ref-item">
                <div className="ref-label">Force batch mode</div>
                <div className="cmd-block">
                  <code>dictate config set batch-mode true</code>
                  <CopyButton text="dictate config set batch-mode true" label="copy" id="ref-batch" />
                </div>
              </div>
              <div className="ref-item">
                <div className="ref-label">Set API key</div>
                <div className="cmd-block">
                  <code>dictate config set mistral-api-key YOUR_KEY</code>
                  <CopyButton text="dictate config set mistral-api-key YOUR_KEY" label="copy" id="ref-5" />
                </div>
              </div>
              <div className="ref-item">
                <div className="ref-label">Generate keybind</div>
                <div className="cmd-block">
                  <code>dictate shortcuts hyprland --mode type --key SUPER,R</code>
                  <CopyButton text="dictate shortcuts hyprland --mode type --key SUPER,R" label="copy" id="ref-6" />
                </div>
              </div>
            </div>
          </ScrollReveal>
        </div>
      </section>

      {/* ── FOOTER ── */}
      <footer className="foot">
        <span>Rust · PipeWire · GPL-3.0</span>
        <div className="foot-links">
          <a href="https://github.com/Aditya190803/dictate" target="_blank" rel="noopener noreferrer">GitHub</a>
          <a href="https://github.com/Aditya190803/dictate/releases" target="_blank" rel="noopener noreferrer">Releases</a>
        </div>
      </footer>
    </main>
  );
}
