# dictate on Windows

Same binary, same config keys, same profiles as Linux. What changes is the platform glue: output sinks are in-process Win32 calls instead of `ydotool` / `wl-copy`, and daemon control is a named pipe instead of `SIGUSR1`.

**No ydotool, no wl-clipboard, no PipeWire, no helper binary to install.**

## Build

Pick **one** toolchain. MSVC is the Rust default on Windows; the GNU one avoids Visual Studio.

```powershell
# Option A — MSVC (needs Visual Studio Build Tools, "Desktop development with C++")
winget install Microsoft.VisualStudio.2022.BuildTools
rustup default stable-x86_64-pc-windows-msvc

# Option B — GNU (no Visual Studio; needs MinGW-w64 binutils on PATH)
winget install BrechtSanders.WinLibs.POSIX.UCRT
rustup default stable-x86_64-pc-windows-gnu
$env:PATH = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:PATH"
```

```powershell
cargo build --release
copy target\release\dictate.exe %USERPROFILE%\bin\dictate.exe
```

| Item | Status |
|------|--------|
| **C compiler** | **Not needed.** Windows TLS is SChannel via `native-tls`; Unix uses rustls + `ring`, and `ring` is the part that would need one |
| `x86_64-pc-windows-msvc` | Works with the VS Build Tools linker |
| `x86_64-pc-windows-gnu` | Works with MinGW-w64 binutils on `PATH`. rustup's bundled `dlltool` is **not** enough — it invokes `as`, which rustup does not ship, and you get `dlltool ... CreateProcess` errors |
| Audio capture | cpal → WASAPI, no extra setup |
| `--features words-ui` | **Not supported** on Windows (GTK4) |
| `--features local` | **Not supported/tested** on Windows (whisper-rs needs cmake + clang) |

## Config

| Path | Contents |
|------|----------|
| `%APPDATA%\dictate\.env` | Same keys as Linux |
| `%APPDATA%\dictate\text.toml` | Dictionary, snippets, cleanup, `[polish]` |
| `%APPDATA%\dictate\` | Models, history, scratchpad |

```powershell
dictate setup      # guided, reports Windows-specific checks
dictate doctor
```

Minimal `%APPDATA%\dictate\.env`:

```bash
TRANSCRIPTION_PROVIDER=mistral
MISTRAL_API_KEY=...
SHORTCUT_OUTPUT=type
SHORTCUT_KEY_LIVE=CTRL,ALT,R
```

## Output sinks

| `SHORTCUT_OUTPUT` | Windows implementation |
|-------------------|------------------------|
| `type` | Win32 `SendInput` (Unicode) into the focused window, in-process |
| `paste` | Sets the Win32 clipboard, then sends Ctrl+V |
| `clipboard` | Sets the Win32 clipboard |
| `stdout` | Prints |

Clipboard command mode (`dictate --command`) reads the Win32 clipboard directly — no `wl-paste`.

## Shortcuts

> **Win+R is reserved by Windows** (Run dialog). The Linux default `SUPER,R` **will not register**. Set `SHORTCUT_KEY_LIVE=CTRL,ALT,R` (or any free combo).

Global shortcuts come from a background hotkey agent. `dictate setup` installs it at login and registers `SHORTCUT_KEY_LIVE` (Ctrl+Alt+R by default). The key runs `dictate`.

## Known limits

| Limit | Detail |
|-------|--------|
| Elevated windows | `SendInput` cannot type into a window running as Administrator unless dictate is elevated too. Run dictate as Administrator, or use `clipboard` output. |
| `dictate words` GUI | Needs the `words-ui` GTK feature, which is not built on Windows. `dictate words` falls back to opening `text.toml` in an editor (`notepad` by default, or `$EDITOR`). |
| Local Whisper | `--features local` is not supported on Windows; use Mistral or Groq. |

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Hotkey never fires | The combo is taken by Windows or another app (classically `Win+R`). Change `SHORTCUT_KEY_LIVE` and run `dictate setup`. |
| Text goes nowhere in an admin app | See elevation limit above. |
| First press is slow | Run `dictate setup` and accept login autostart. |
| Shortcut does nothing | Run `dictate doctor`, then `dictate setup`. |
| "No default input device" | Check the mic in Windows Settings → Sound → Input, and app microphone permissions. |
| Anything else | `dictate doctor` |
