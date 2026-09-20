"use strict";

const path = require("path");
const { app, BrowserWindow, Tray, Menu, dialog, ipcMain, nativeImage, shell } = require("electron");
const lib = require("./lib");

let win = null;
let tray = null;
let listening = false;
let binary = null;
let lastError = null;
let quitting = false;

function ok(data) {
  return { ok: true, ...data };
}

function fail(error) {
  return { ok: false, error: error instanceof Error ? error.message : String(error) };
}

function shortcutLabel(config) {
  const raw = config.SHORTCUT_KEY_LIVE || config.SHORTCUT_KEY || "";
  if (raw) return raw.replace(/,/g, "+");
  return process.platform === "win32" ? "Ctrl+Alt+R" : "Super+R";
}

function snapshot() {
  const config = lib.loadEnvFile();
  const book = lib.loadWordBook();
  return {
    binary,
    listening,
    error: lastError,
    platform: process.platform,
    configDir: lib.configDir(),
    shortcut: shortcutLabel(config),
    config: {
      TRANSCRIPTION_PROVIDER: config.TRANSCRIPTION_PROVIDER || "mistral",
      MISTRAL_API_KEY: config.MISTRAL_API_KEY || "",
      GROQ_API_KEY: config.GROQ_API_KEY || "",
      DEEPGRAM_API_KEY: config.DEEPGRAM_API_KEY || "",
      OPENCODE_API_KEY: config.OPENCODE_API_KEY || "",
      POLISH_PROVIDER: config.POLISH_PROVIDER || "auto",
      SHORTCUT_OUTPUT: config.SHORTCUT_OUTPUT || "type",
      SHORTCUT_KEY_LIVE: config.SHORTCUT_KEY_LIVE || config.SHORTCUT_KEY || "",
      DICTATE_PROFILE: config.DICTATE_PROFILE || "segmented",
      ENABLE_AUDIO_FEEDBACK: config.ENABLE_AUDIO_FEEDBACK || "true",
      BEEP_VOLUME: config.BEEP_VOLUME || "0.1",
      TRANSCRIPTION_LANGUAGE: config.TRANSCRIPTION_LANGUAGE || "auto",
    },
    polish: lib.loadPolish(),
    words: lib.rowsFromBook(book),
    history: lib.loadHistory(40).reverse(),
  };
}

function broadcast() {
  if (win && !win.isDestroyed()) {
    win.webContents.send("changed", snapshot());
  }
  updateTray();
}

function updateTray() {
  if (!tray) return;
  const label = listening ? "Stop listening" : "Listen";
  tray.setToolTip(listening ? "dictate · listening" : "dictate");
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label, click: () => toggleListen().catch(() => {}) },
      { type: "separator" },
      { label: "Open", click: showWindow },
      { label: "Quit", click: () => app.quit() },
    ]),
  );
}

function showWindow() {
  if (!win) return;
  win.show();
  win.focus();
}

function resolveBinary() {
  binary = lib.findBinary(app.getPath("userData"));
  return binary;
}

async function toggleListen() {
  lastError = null;
  const bin = resolveBinary();
  if (!bin) {
    lastError = "dictate binary not found. Install it, or pick the executable in Setup.";
    broadcast();
    throw new Error(lastError);
  }

  const result = await lib.runDictate(bin, [], { timeoutMs: 8000 });
  if (result.error) {
    lastError = result.error.message;
    broadcast();
    throw result.error;
  }
  if (result.code !== 0) {
    lastError = (result.stderr || result.stdout || `dictate exited ${result.code}`).trim();
    broadcast();
    throw new Error(lastError);
  }
  listening = !listening;
  broadcast();
}

function createWindow() {
  win = new BrowserWindow({
    width: 460,
    height: 720,
    minWidth: 400,
    minHeight: 560,
    backgroundColor: "#fcfcfa",
    autoHideMenuBar: true,
    title: "dictate",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });
  win.loadFile(path.join(__dirname, "renderer", "index.html"));
  if (process.env.DICTATE_DESKTOP_SMOKE) {
    win.webContents.on("did-finish-load", async () => {
      const fs = require("fs");
      const snap = async (name) => {
        await new Promise((resolve) => setTimeout(resolve, 600));
        const image = await win.capturePage();
        fs.writeFileSync(path.join(__dirname, `smoke-${name}.png`), image.toPNG());
      };
      await snap("listen");
      await win.webContents.executeJavaScript(`document.getElementById("tab-words").click()`);
      await snap("words");
      await win.webContents.executeJavaScript(`document.getElementById("tab-setup").click()`);
      await new Promise((resolve) => setTimeout(resolve, 1200));
      await snap("setup");
      quitting = true;
      app.quit();
    });
  }
  win.on("close", (event) => {
    if (!quitting) {
      event.preventDefault();
      win.hide();
    }
  });
}

function trayImage() {
  const size = 16;
  const buf = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const dx = x - 7.5;
      const dy = y - 7.5;
      if (dx * dx + dy * dy <= 25) {
        const i = (y * size + x) * 4;
        buf[i] = 61;
        buf[i + 1] = 122;
        buf[i + 2] = 79;
        buf[i + 3] = 255;
      }
    }
  }
  return nativeImage.createFromBitmap(buf, { width: size, height: size });
}

function createTray() {
  try {
    tray = new Tray(trayImage());
    tray.on("click", showWindow);
    updateTray();
  } catch (error) {
    console.warn("tray unavailable:", error.message);
  }
}

app.setName("dictate");

const locked = app.requestSingleInstanceLock();
if (!locked) {
  app.quit();
} else {
  app.on("second-instance", showWindow);
  app.whenReady().then(() => {
    Menu.setApplicationMenu(null);
    resolveBinary();
    createWindow();
    createTray();
  });
}

app.on("before-quit", () => {
  quitting = true;
});

app.on("activate", showWindow);

ipcMain.handle("state", () => {
  try {
    return ok(snapshot());
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("toggle", async () => {
  try {
    await toggleListen();
    return ok(snapshot());
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("set-config", async (_event, key, value) => {
  try {
    const bin = resolveBinary();
    if (!bin) throw new Error("dictate binary not found");
    const result = await lib.runDictate(bin, ["config", "set", String(key), String(value)]);
    if (result.code !== 0) {
      throw new Error((result.stderr || result.stdout || "config set failed").trim());
    }
    broadcast();
    return ok(snapshot());
  } catch (error) {
    lastError = error.message;
    broadcast();
    return fail(error);
  }
});

ipcMain.handle("set-polish", (_event, patch) => {
  try {
    lib.savePolish(patch || {});
    broadcast();
    return ok(snapshot());
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("words:upsert", (_event, row) => {
  try {
    const book = lib.loadWordBook();
    const word = row && row.word;
    const previous = row && row.previous;
    const err = lib.validateNewWord(book, word, previous);
    if (err) throw new Error(err);
    const miss = row && row.misspelling ? String(row.misspelling).trim() : "";
    const next = lib.upsertRow(book, word, miss || null, previous || null);
    lib.saveWordBook(next);
    broadcast();
    return ok(snapshot());
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("words:remove", (_event, word) => {
  try {
    lib.saveWordBook(lib.removePreferredWord(lib.loadWordBook(), word));
    broadcast();
    return ok(snapshot());
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("words:star", (_event, word) => {
  try {
    lib.saveWordBook(lib.toggleStar(lib.loadWordBook(), word));
    broadcast();
    return ok(snapshot());
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("history:clear", () => {
  try {
    lib.clearHistory();
    broadcast();
    return ok(snapshot());
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("doctor", async () => {
  try {
    const bin = resolveBinary();
    if (!bin) throw new Error("dictate binary not found");
    const result = await lib.runDictate(bin, ["doctor"], { timeoutMs: 25000 });
    const text = `${result.stdout}\n${result.stderr}`.trim();
    return ok({ lines: lib.parseDoctor(text), raw: text });
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("autostart", async (_event, action) => {
  try {
    const bin = resolveBinary();
    if (!bin) throw new Error("dictate binary not found");
    if (action !== "install" && action !== "remove" && action !== "status") {
      throw new Error("unknown autostart action");
    }
    const result = await lib.runDictate(bin, ["autostart", action]);
    if (result.code !== 0) {
      throw new Error((result.stderr || result.stdout || "autostart failed").trim());
    }
    broadcast();
    return ok({ text: `${result.stdout}\n${result.stderr}`.trim() });
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("pick-binary", async () => {
  try {
    const picked = await dialog.showOpenDialog(win, {
      title: "Locate dictate",
      properties: ["openFile"],
      filters:
        process.platform === "win32"
          ? [{ name: "Executable", extensions: ["exe"] }]
          : [{ name: "All files", extensions: ["*"] }],
    });
    if (picked.canceled || !picked.filePaths[0]) return ok(snapshot());
    binary = picked.filePaths[0];
    lib.saveBinary(app.getPath("userData"), binary);
    lastError = null;
    broadcast();
    return ok(snapshot());
  } catch (error) {
    return fail(error);
  }
});

ipcMain.handle("open-config", async () => {
  try {
    await shell.openPath(lib.configDir());
    return ok({});
  } catch (error) {
    return fail(error);
  }
});

setInterval(() => {
  if (win && !win.isDestroyed() && win.isVisible()) broadcast();
}, 2500);
