"use client";

import { useState } from "react";
import CopyButton from "./CopyButton";
import { useIsWindows } from "../lib/client-hooks";

/**
 * Linux and Windows are both first-class now, and their install paths differ
 * enough that a single command would be wrong for half of visitors. Defaults
 * to whichever platform the visitor is actually on.
 */

type Platform = "linux" | "windows";

const INSTALL: Record<
  Platform,
  { label: string; cmd: string; note: React.ReactNode }
> = {
  linux: {
    label: "Linux",
    cmd: "curl -fsSL https://dictate.adityamer.dev/install.sh | sh",
    note: (
      <>
        Detects your distro, installs PipeWire deps, and pulls the latest
        release binary. Then run <code>dictate setup</code>.
      </>
    ),
  },
  windows: {
    label: "Windows",
    cmd: "cargo build --release && dictate setup",
    note: (
      <>
        No C compiler needed — TLS uses SChannel. Requires the MSVC or MinGW-w64
        linker. Typing, clipboard, and global shortcuts are in-process Win32
        calls.
      </>
    ),
  },
};

export default function InstallTabs() {
  // Detected platform is the default; an explicit click overrides it.
  const detected: Platform = useIsWindows() ? "windows" : "linux";
  const [chosen, setChosen] = useState<Platform | null>(null);
  const platform = chosen ?? detected;

  const active = INSTALL[platform];

  return (
    <div>
      <div className="tabs" role="tablist" aria-label="Install platform">
        {(Object.keys(INSTALL) as Platform[]).map((p) => (
          <button
            key={p}
            role="tab"
            id={`tab-${p}`}
            aria-selected={platform === p}
            aria-controls={`panel-${p}`}
            className="tab"
            onClick={() => setChosen(p)}
          >
            {INSTALL[p].label}
          </button>
        ))}
      </div>

      <div
        role="tabpanel"
        id={`panel-${platform}`}
        aria-labelledby={`tab-${platform}`}
        style={{ marginTop: 14 }}
      >
        <div className="cmd">
          <code>
            <span className="sigil" aria-hidden="true">
              $
            </span>
            {active.cmd}
          </code>
          <CopyButton text={active.cmd} id={`copy-${platform}`} />
        </div>
        <p className="note">{active.note}</p>
      </div>
    </div>
  );
}
