"use client";

import CopyButton from "./CopyButton";

const INSTALL_MD_URL =
  "https://dictate.adityamer.dev/INSTALL.md";

const AGENT_PROMPT = `Read ${INSTALL_MD_URL} and follow it step by step to install and configure dictate on this machine. Ask me the setup questions first, then execute everything non-interactively using 'dictate config set'.`;

const CURL_CMD = `curl -fsSL https://dictate.adityamer.dev/install.sh | sh`;

export default function InstallSection() {
  return (
    <div className="install-grid">
      {/* Primary: INSTALL.md */}
      <div className="install-card">
        <span className="install-badge rec">recommended</span>
        <div className="install-title">Give INSTALL.md to your AI agent</div>
        <p className="install-desc">
          Paste this prompt into your coding agent. It reads INSTALL.md, asks
          you setup questions (provider, API key, compositor, output mode),
          then installs and configures everything non-interactively.
        </p>

        <div className="t" style={{ marginBottom: 14 }}>
          <div className="t-bar">
            <span className="t-title">agent prompt</span>
            <CopyButton text={AGENT_PROMPT} label="copy" id="copy-agent" />
          </div>
          <div className="t-body">
            <div style={{ color: "#c8c4bc", fontSize: "0.76rem", lineHeight: 1.7 }}>
              {AGENT_PROMPT}
            </div>
          </div>
        </div>

        <p
          style={{
            fontSize: "0.73rem",
            color: "var(--text3)",
            lineHeight: 1.6,
            marginBottom: 12,
          }}
        >
          The agent uses{" "}
          <code
            style={{
              fontFamily: "var(--mono)",
              background: "var(--bg2)",
              padding: "1px 5px",
              borderRadius: "3px",
              fontSize: "0.68rem",
            }}
          >
            dictate config set
          </code>{" "}
          for each setting. Zero interactive prompts needed.
        </p>

        <div className="badges">
          {["Claude Code", "Cursor", "Copilot", "Windsurf", "Gemini CLI"].map(
            (a) => (
              <span className="badge" key={a}>{a}</span>
            )
          )}
        </div>
      </div>

      {/* Fallback: curl */}
      <div className="install-card">
        <span className="install-badge alt">fallback</span>
        <div className="install-title">Install script</div>
        <p className="install-desc">
          One command — detects your distro, installs dependencies, downloads
          the binary, and walks through interactive setup.
        </p>

        <div className="t" style={{ marginBottom: 14 }}>
          <div className="t-bar">
            <span className="t-title">bash</span>
            <CopyButton text={CURL_CMD} label="copy" id="copy-curl" />
          </div>
          <div className="t-body">
            <div className="t-line">
              <span className="t-ps">$</span>
              <span className="t-cmd">{CURL_CMD}</span>
            </div>
          </div>
        </div>

        <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 16 }}>
          {["Arch", "Ubuntu", "Fedora", "Debian", "openSUSE", "Alpine", "Void"].map(
            (d) => (
              <span className="kbd" key={d}>{d}</span>
            )
          )}
        </div>

        <details
          style={{
            border: "1px solid var(--border)",
            borderRadius: "8px",
            background: "var(--bg)",
          }}
        >
          <summary
            style={{
              padding: "8px 14px",
              cursor: "pointer",
              fontSize: "0.72rem",
              color: "var(--text3)",
              fontFamily: "var(--mono)",
              listStyle: "none",
              fontWeight: 500,
            }}
          >
            ▸ env overrides
          </summary>
          <div
            style={{
              padding: "0 14px 10px",
              fontFamily: "var(--mono)",
              fontSize: "0.68rem",
              color: "var(--text3)",
              lineHeight: 2,
            }}
          >
            <div><span style={{ color: "var(--accent)" }}>DICTATE_BUILD_FROM_SOURCE</span>=yes</div>
            <div><span style={{ color: "var(--accent)" }}>DICTATE_BUILD_FEATURES</span>=local</div>
            <div><span style={{ color: "var(--accent)" }}>DICTATE_INSTALL_DIR</span>=/usr/local/bin</div>
          </div>
        </details>
      </div>
    </div>
  );
}
