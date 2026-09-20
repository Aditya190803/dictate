import Link from "next/link";
import { GithubLogo } from "@phosphor-icons/react/dist/ssr";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";
import { Kbd } from "./components/ui/kbd";
import { Separator } from "./components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "./components/ui/table";
import CopyButton from "./components/CopyButton";
import FadeIn from "./components/FadeIn";
import InstallTabs from "./components/InstallTabs";

const REPO = "https://github.com/Aditya190803/dictate";

const AGENT_PROMPT =
  "Read https://dictate.adityamer.dev/INSTALL.md and follow it step by step to install and configure dictate on this machine. Ask me the setup questions first, then execute everything non-interactively using 'dictate config set'.";

const STEPS = [
  { n: "01", t: "Bind a key", d: "One shortcut, registered system-wide." },
  { n: "02", t: "Speak", d: "A warm daemon holds the model and socket. No cold start." },
  { n: "03", t: "Press again", d: "The same key stops the take and finalises it." },
  { n: "04", t: "Text lands", d: "Typed, pasted, copied, or written to stdout." },
];

const PROVIDERS = [
  { name: "Mistral", model: "voxtral-mini-transcribe-realtime", realtime: true, local: false },
  { name: "Deepgram", model: "nova-3", realtime: true, local: false },
  { name: "Groq", model: "whisper-large-v3-turbo", realtime: false, local: false },
  { name: "Whisper", model: "ggml, on-device", realtime: false, local: true },
];

const FEATURES = [
  { t: "Stdout first", d: "--pipe-to hands text to any command — wl-copy, ydotool, sed, your own script." },
  { t: "Two platforms, one config", d: "Same keys on both. PipeWire/ydotool on Wayland, WASAPI/Win32 on Windows." },
  { t: "Idle costs nothing", d: "The daemon sleeps until a shortcut arrives. No polling, no background work." },
  { t: "Polish is separate", d: "A text model cleans up after recognition — OpenCode Zen, Mistral, or local Ollama." },
  { t: "Your vocabulary", d: "A dictionary of names and mishearings is applied before text reaches the screen." },
  { t: "Fully offline option", d: "Local Whisper keeps audio on the machine. Nothing uploaded, no key needed." },
];

function Eyebrow({ children }: { children: React.ReactNode }) {
  return (
    <p className="font-mono text-xs font-medium tracking-[0.14em] text-go uppercase">
      {children}
    </p>
  );
}

function SectionHead({
  eyebrow,
  title,
  children,
}: {
  eyebrow: string;
  title: string;
  children?: React.ReactNode;
}) {
  return (
    <FadeIn className="max-w-2xl">
      <Eyebrow>{eyebrow}</Eyebrow>
      <h2 className="mt-3 font-serif text-3xl leading-[1.1] tracking-[-0.02em] text-balance sm:text-4xl">
        {title}
      </h2>
      {children && (
        <p className="mt-3 text-[0.98rem] leading-relaxed text-pretty text-muted-foreground">
          {children}
        </p>
      )}
    </FadeIn>
  );
}

export default function Home() {
  return (
    <>
      <a href="#main" className="skip">
        Skip to content
      </a>

      <header className="sticky top-0 z-50 border-b bg-background/85 backdrop-blur-md">
        <nav className="mx-auto flex h-13 w-full max-w-4xl items-center gap-6 px-6">
          <Link href="/" className="font-mono text-[0.95rem] font-bold tracking-[-0.02em]">
            <span className="text-go" aria-hidden="true">
              ›
            </span>{" "}
            dictate
          </Link>
          <div className="ml-auto hidden items-center gap-1 sm:flex">
            <Button variant="ghost" size="sm" asChild>
              <a href="#how">How it works</a>
            </Button>
            <Button variant="ghost" size="sm" asChild>
              <a href="#providers">Providers</a>
            </Button>
            <Button variant="ghost" size="sm" asChild>
              <a href="#install">Install</a>
            </Button>
          </div>
          <Button variant="ghost" size="icon" asChild aria-label="View dictate on GitHub">
            <a href={REPO} target="_blank" rel="noopener noreferrer">
              <GithubLogo className="size-[1.1rem]" />
            </a>
          </Button>
        </nav>
      </header>

      <main id="main" className="mx-auto w-full max-w-4xl px-6">
        {/* ── Hero ── */}
        <section className="pt-16 pb-16 sm:pt-24 sm:pb-20">
          <FadeIn>
            <Eyebrow>Speech to text for Wayland Linux and Windows</Eyebrow>
            <h1 className="mt-5 max-w-2xl font-serif text-5xl leading-[1.05] tracking-[-0.025em] text-balance sm:text-6xl">
              Press a key. Speak.{" "}
              <em className="text-go">It&rsquo;s already typed.</em>
            </h1>
            <p className="mt-5 max-w-[52ch] text-[1.05rem] leading-relaxed text-pretty text-muted-foreground">
              A daemon-friendly dictation CLI. Realtime transcription lands
              straight in the focused window — no GUI, no cloud account, no
              waiting for a model to load.
            </p>
            <div className="mt-7 flex flex-wrap items-center gap-3">
              <Button asChild>
                <a href="#install">Install dictate</a>
              </Button>
              <Button variant="outline" asChild>
                <a href={REPO} target="_blank" rel="noopener noreferrer">
                  Source on GitHub
                </a>
              </Button>
              <span className="ml-1 inline-flex items-center gap-1.5 text-xs text-muted-foreground">
                <Kbd>Super</Kbd>+<Kbd>R</Kbd>
                <span className="mx-1 opacity-50">/</span>
                <Kbd>Ctrl</Kbd>+<Kbd>Alt</Kbd>+<Kbd>R</Kbd>
              </span>
            </div>
          </FadeIn>
        </section>

        <Separator />

        {/* ── How it works ── */}
        <section id="how" className="scroll-mt-16 py-14 sm:py-16">
          <SectionHead eyebrow="How it works" title="One key, held open by a warm daemon" />

          <div className="mt-10 grid gap-x-8 gap-y-8 sm:grid-cols-2 lg:grid-cols-4">
            {STEPS.map((s, i) => (
              <FadeIn key={s.n} delay={i * 60}>
                <div className="border-t pt-4">
                  <span className="font-mono text-xs font-medium text-go">{s.n}</span>
                  <h3 className="mt-2 text-[0.95rem] font-medium">{s.t}</h3>
                  <p className="mt-1 text-sm leading-relaxed text-muted-foreground">
                    {s.d}
                  </p>
                </div>
              </FadeIn>
            ))}
          </div>
        </section>

        <Separator />

        {/* ── Providers ── */}
        <section id="providers" className="scroll-mt-16 py-14 sm:py-16">
          <SectionHead eyebrow="Providers" title="Pick your trade-off, not ours" />

          <FadeIn delay={80} className="mt-8">
            <div className="overflow-hidden rounded-xl border bg-card">
              <Table>
                <TableHeader>
                  <TableRow className="hover:bg-transparent">
                    <TableHead>Provider</TableHead>
                    <TableHead>Model</TableHead>
                    <TableHead>Realtime</TableHead>
                    <TableHead>Audio</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {PROVIDERS.map((p) => (
                    <TableRow key={p.name}>
                      <TableCell className="font-medium">{p.name}</TableCell>
                      <TableCell className="font-mono text-[0.78rem] text-muted-foreground">
                        {p.model}
                      </TableCell>
                      <TableCell>
                        <Badge variant={p.realtime ? "go" : "secondary"}>
                          {p.realtime ? "streaming" : "on pause"}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <Badge variant={p.local ? "go" : "secondary"}>
                          {p.local ? "on device" : "cloud"}
                        </Badge>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
            <p className="mt-4 text-sm leading-relaxed text-muted-foreground">
              One dictation: words appear as you speak, pauses polish the last
              phrase, and voice edits rewrite what was just typed. Clipboard
              commands run on the same shortcut when you copied text first.
            </p>
          </FadeIn>
        </section>

        <Separator />

        {/* ── Features ── */}
        <section id="features" className="scroll-mt-16 py-14 sm:py-16">
          <SectionHead eyebrow="Details" title="A UNIX citizen that happens to hear" />

          <div className="mt-10 grid gap-x-10 gap-y-7 sm:grid-cols-2 lg:grid-cols-3">
            {FEATURES.map((f, i) => (
              <FadeIn key={f.t} delay={i * 40}>
                <h3 className="text-sm font-medium">{f.t}</h3>
                <p className="mt-1 text-sm leading-relaxed text-muted-foreground">
                  {f.d}
                </p>
              </FadeIn>
            ))}
          </div>
        </section>

        <Separator />

        {/* ── Install ── */}
        <section id="install" className="scroll-mt-16 py-14 sm:py-16">
          <SectionHead eyebrow="Install" title="Running in about a minute">
            <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-[0.85em]">dictate doctor</code>{" "}
            checks the whole chain — keys, mic, permissions, daemons.
          </SectionHead>

          <FadeIn delay={70} className="mt-8">
            <InstallTabs />
          </FadeIn>

          <FadeIn delay={120} className="mt-8">
            <div className="flex items-start gap-3 rounded-lg border bg-card p-4">
              <p className="flex-1 font-mono text-[0.78rem] leading-relaxed text-muted-foreground">
                {AGENT_PROMPT}
              </p>
              <CopyButton text={AGENT_PROMPT} />
            </div>
            <p className="mt-3 text-sm text-muted-foreground">
              Or hand that prompt to Claude Code, Cursor, Copilot, Windsurf, or
              Gemini CLI.
            </p>
          </FadeIn>
        </section>
      </main>

      <footer className="border-t">
        <div className="mx-auto flex w-full max-w-4xl flex-wrap items-center justify-between gap-3 px-6 py-8">
          <p className="font-mono text-xs text-muted-foreground">
            Free, GPL-3.0 · Rust · PipeWire · WASAPI
          </p>
          <div className="flex flex-wrap gap-5 text-sm text-muted-foreground">
            <a className="transition-colors hover:text-foreground" href={REPO} target="_blank" rel="noopener noreferrer">
              GitHub
            </a>
            <a className="transition-colors hover:text-foreground" href={`${REPO}/releases`} target="_blank" rel="noopener noreferrer">
              Releases
            </a>
            <a className="transition-colors hover:text-foreground" href="/INSTALL.md">
              Docs
            </a>
            <a className="transition-colors hover:text-foreground" href={`${REPO}/blob/main/LICENSE`} target="_blank" rel="noopener noreferrer">
              License
            </a>
          </div>
        </div>
      </footer>
    </>
  );
}
