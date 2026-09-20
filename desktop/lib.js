"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFile, execFileSync } = require("child_process");
const TOML = require("@iarna/toml");

const MAX_WORD_LEN = 60;

function configDir() {
  if (process.platform === "win32") {
    return path.join(process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming"), "dictate");
  }
  return path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config"), "dictate");
}

function envPath() {
  return path.join(configDir(), ".env");
}

function textPath() {
  return path.join(configDir(), "text.toml");
}

function historyPath() {
  return path.join(configDir(), "history.jsonl");
}

function parseEnv(text) {
  const out = {};
  if (!text) return out;
  for (const line of String(text).split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const i = trimmed.indexOf("=");
    if (i < 0) continue;
    out[trimmed.slice(0, i).trim()] = trimmed.slice(i + 1);
  }
  return out;
}

function loadEnvFile(filePath = envPath()) {
  if (!fs.existsSync(filePath)) return {};
  return parseEnv(fs.readFileSync(filePath, "utf8"));
}

function wordCharLen(word) {
  return Array.from(word).length;
}

function normalizeBook(raw) {
  const preferred = [];
  const seen = new Set();
  for (const word of raw.preferred_words || []) {
    const w = String(word).trim();
    if (!w || wordCharLen(w) > MAX_WORD_LEN) continue;
    const key = w.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    preferred.push(w);
  }
  preferred.sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));

  const starred = [];
  const starSeen = new Set();
  for (const word of raw.starred_words || []) {
    const w = String(word).trim();
    if (!w) continue;
    const key = w.toLowerCase();
    if (starSeen.has(key)) continue;
    starSeen.add(key);
    starred.push(w);
  }
  starred.sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()));

  const dictionary = {};
  const dict = raw.dictionary && typeof raw.dictionary === "object" ? raw.dictionary : {};
  for (const [heard, preferredWord] of Object.entries(dict)) {
    if (typeof preferredWord !== "string") continue;
    dictionary[heard] = preferredWord;
  }

  return { preferred_words: preferred, starred_words: starred, dictionary };
}

function misspellingFor(book, correct) {
  const needle = correct.toLowerCase();
  for (const [heard, preferred] of Object.entries(book.dictionary)) {
    if (String(preferred).toLowerCase() === needle) return heard;
  }
  return null;
}

function rowsFromBook(book) {
  const rows = book.preferred_words.map((word) => ({
    word,
    misspelling: misspellingFor(book, word),
    starred: book.starred_words.some((s) => s.toLowerCase() === word.toLowerCase()),
  }));
  rows.sort((a, b) => {
    if (a.starred !== b.starred) return a.starred ? -1 : 1;
    return a.word.toLowerCase().localeCompare(b.word.toLowerCase());
  });
  return rows;
}

function loadTomlFile(filePath) {
  if (!fs.existsSync(filePath)) return {};
  const contents = fs.readFileSync(filePath, "utf8");
  if (!contents.trim()) return {};
  return TOML.parse(contents);
}

function loadWordBook(filePath = textPath()) {
  const root = loadTomlFile(filePath);
  return normalizeBook({
    preferred_words: root.preferred_words || [],
    starred_words: root.starred_words || [],
    dictionary: root.dictionary || {},
  });
}

function saveWordBook(book, filePath = textPath()) {
  const dir = path.dirname(filePath);
  fs.mkdirSync(dir, { recursive: true });
  const root = loadTomlFile(filePath);
  const next = normalizeBook(book);
  root.preferred_words = next.preferred_words;
  if (next.starred_words.length) root.starred_words = next.starred_words;
  else delete root.starred_words;
  root.dictionary = next.dictionary;
  fs.writeFileSync(filePath, TOML.stringify(root));
  return next;
}

function validateNewWord(book, word, exceptWord) {
  const trimmed = String(word || "").trim();
  if (!trimmed) return "Enter a word or phrase.";
  if (wordCharLen(trimmed) > MAX_WORD_LEN) {
    return `Use at most ${MAX_WORD_LEN} characters.`;
  }
  const except = exceptWord ? String(exceptWord).trim().toLowerCase() : null;
  const clash = book.preferred_words.some((existing) => {
    if (existing.toLowerCase() !== trimmed.toLowerCase()) return false;
    return !except || existing.toLowerCase() !== except;
  });
  if (clash) return "That word is already in your dictionary.";
  return null;
}

function addPreferredWord(book, word) {
  const trimmed = String(word || "").trim();
  if (!trimmed || wordCharLen(trimmed) > MAX_WORD_LEN) return book;
  if (book.preferred_words.some((w) => w.toLowerCase() === trimmed.toLowerCase())) {
    return book;
  }
  return normalizeBook({
    ...book,
    preferred_words: [...book.preferred_words, trimmed],
  });
}

function removePreferredWord(book, word) {
  const needle = String(word || "").trim().toLowerCase();
  const dictionary = {};
  for (const [heard, preferred] of Object.entries(book.dictionary)) {
    if (String(preferred).toLowerCase() === needle) continue;
    dictionary[heard] = preferred;
  }
  return normalizeBook({
    preferred_words: book.preferred_words.filter((w) => w.toLowerCase() !== needle),
    starred_words: book.starred_words.filter((w) => w.toLowerCase() !== needle),
    dictionary,
  });
}

function setMisspelling(book, correct, wrong) {
  let next = addPreferredWord(book, correct);
  const target = String(correct || "").trim();
  const dictionary = {};
  for (const [heard, preferred] of Object.entries(next.dictionary)) {
    if (String(preferred).toLowerCase() === target.toLowerCase()) continue;
    dictionary[heard] = preferred;
  }
  const heard = wrong ? String(wrong).trim() : "";
  if (heard && wordCharLen(heard) <= MAX_WORD_LEN && wordCharLen(target) <= MAX_WORD_LEN) {
    dictionary[heard] = target;
  }
  return normalizeBook({ ...next, dictionary });
}

function toggleStar(book, word) {
  const trimmed = String(word || "").trim();
  if (!trimmed) return book;
  const has = book.starred_words.some((w) => w.toLowerCase() === trimmed.toLowerCase());
  const starred_words = has
    ? book.starred_words.filter((w) => w.toLowerCase() !== trimmed.toLowerCase())
    : [...book.starred_words, trimmed];
  return normalizeBook({ ...book, starred_words });
}

function upsertRow(book, word, misspelling, previousWord) {
  let next = book;
  const target = String(word || "").trim();
  const prev = previousWord ? String(previousWord).trim() : "";
  if (prev && prev.toLowerCase() !== target.toLowerCase()) {
    next = removePreferredWord(next, prev);
  } else if (prev && prev !== target) {
    next = {
      ...next,
      preferred_words: next.preferred_words.map((w) =>
        w.toLowerCase() === prev.toLowerCase() ? target : w,
      ),
      starred_words: next.starred_words.map((w) =>
        w.toLowerCase() === prev.toLowerCase() ? target : w,
      ),
    };
    next = setMisspelling(next, prev, null);
  } else if (prev) {
    next = setMisspelling(next, prev, null);
  }
  next = addPreferredWord(next, target);
  return setMisspelling(next, target, misspelling);
}

function loadPolish(filePath = textPath()) {
  const root = loadTomlFile(filePath);
  const polish = root.polish && typeof root.polish === "object" ? root.polish : {};
  return {
    enabled: polish.enabled !== false,
    style: String(polish.style || ""),
    model: String(polish.model || "big-pickle"),
    on_failure: String(polish.on_failure || "fallback"),
  };
}

function savePolish(patch, filePath = textPath()) {
  const dir = path.dirname(filePath);
  fs.mkdirSync(dir, { recursive: true });
  const root = loadTomlFile(filePath);
  root.polish = { ...(root.polish || {}), ...patch };
  fs.writeFileSync(filePath, TOML.stringify(root));
  return loadPolish(filePath);
}

function parseHistoryJsonl(text, limit = 40) {
  const entries = [];
  if (!text) return entries;
  for (const line of String(text).split(/\r?\n/)) {
    if (!line.trim()) continue;
    try {
      const row = JSON.parse(line);
      if (row && typeof row.text === "string") {
        entries.push({
          ts: Number(row.ts) || 0,
          text: row.text,
          profile: String(row.profile || ""),
        });
      }
    } catch {
      // skip a bad line; history is append-only and may be mid-write
    }
  }
  if (limit > 0 && entries.length > limit) {
    return entries.slice(entries.length - limit);
  }
  return entries;
}

function loadHistory(limit = 40, filePath = historyPath()) {
  if (!fs.existsSync(filePath)) return [];
  return parseHistoryJsonl(fs.readFileSync(filePath, "utf8"), limit);
}

function clearHistory(filePath = historyPath()) {
  if (fs.existsSync(filePath)) fs.unlinkSync(filePath);
}

function parseDoctor(text) {
  const lines = [];
  for (const raw of String(text || "").split(/\r?\n/)) {
    const line = raw.trimEnd();
    if (!line.trim() || /^dictate doctor$/i.test(line.trim())) continue;
    if (line.startsWith("✓")) lines.push({ kind: "ok", text: line.slice(1).trim() });
    else if (line.startsWith("✗")) lines.push({ kind: "bad", text: line.slice(1).trim() });
    else if (line.startsWith("⚠") || line.startsWith("!")) {
      lines.push({ kind: "warn", text: line.replace(/^[⚠!]\s*/, "").trim() });
    } else lines.push({ kind: "info", text: line.trim() });
  }
  return lines;
}

function which(cmd) {
  const tool = process.platform === "win32" ? "where" : "which";
  try {
    const out = execFileSync(tool, [cmd], {
      encoding: "utf8",
      windowsHide: true,
      timeout: 4000,
    }).trim();
    return out.split(/\r?\n/).map((s) => s.trim()).find(Boolean) || null;
  } catch {
    return null;
  }
}

function storedBinary(userData) {
  if (!userData) return null;
  try {
    const file = path.join(userData, "binary.json");
    const { path: bin } = JSON.parse(fs.readFileSync(file, "utf8"));
    if (bin && fs.existsSync(bin)) return bin;
  } catch {
    // no saved path yet
  }
  return null;
}

function saveBinary(userData, bin) {
  fs.mkdirSync(userData, { recursive: true });
  fs.writeFileSync(path.join(userData, "binary.json"), JSON.stringify({ path: bin }));
}

function findBinary(userData) {
  const name = process.platform === "win32" ? "dictate.exe" : "dictate";
  const fromStore = storedBinary(userData);
  if (fromStore) return fromStore;
  if (process.env.DICTATE_BIN && fs.existsSync(process.env.DICTATE_BIN)) {
    return process.env.DICTATE_BIN;
  }
  const home = os.homedir();
  const extras = [
    path.join(home, "bin", name),
    path.join(home, ".cargo", "bin", name),
    path.join(home, ".local", "bin", "dictate"),
    path.join(__dirname, "..", "target", "release", name),
    path.join(__dirname, "..", "target", "debug", name),
  ];
  for (const candidate of extras) {
    if (fs.existsSync(candidate)) return candidate;
  }
  return which("dictate") || which(name);
}

function runDictate(bin, args, { timeoutMs = 20000 } = {}) {
  return new Promise((resolve, reject) => {
    const child = execFile(
      bin,
      args,
      { windowsHide: true, timeout: timeoutMs, maxBuffer: 2 * 1024 * 1024 },
      (err, stdout, stderr) => {
        if (err && err.killed) {
          reject(new Error(`dictate ${args.join(" ")} timed out`));
          return;
        }
        resolve({
          code: err && typeof err.code === "number" ? err.code : 0,
          stdout: String(stdout || ""),
          stderr: String(stderr || ""),
          error: err && err.code === "ENOENT" ? err : null,
        });
      },
    );
    child.stdin && child.stdin.end();
  });
}

module.exports = {
  MAX_WORD_LEN,
  configDir,
  envPath,
  textPath,
  historyPath,
  parseEnv,
  loadEnvFile,
  normalizeBook,
  rowsFromBook,
  loadWordBook,
  saveWordBook,
  validateNewWord,
  addPreferredWord,
  removePreferredWord,
  setMisspelling,
  toggleStar,
  upsertRow,
  loadPolish,
  savePolish,
  parseHistoryJsonl,
  loadHistory,
  clearHistory,
  parseDoctor,
  findBinary,
  saveBinary,
  runDictate,
};
