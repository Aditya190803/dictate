"use strict";

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");
const lib = require("./lib");

function tmp() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "dictate-desktop-"));
}

{
  const env = lib.parseEnv(`
# comment
TRANSCRIPTION_PROVIDER=mistral
MISTRAL_API_KEY=secret
SHORTCUT_OUTPUT=type
`);
  assert.equal(env.TRANSCRIPTION_PROVIDER, "mistral");
  assert.equal(env.MISTRAL_API_KEY, "secret");
  assert.equal(env.SHORTCUT_OUTPUT, "type");
}

{
  const dir = tmp();
  const file = path.join(dir, "text.toml");
  fs.writeFileSync(
    file,
    `preferred_words = ["Hyprland"]

[[snippets]]
trigger = "sig"
text = "Best"

[polish]
style = "concise"
`,
  );
  const book = lib.loadWordBook(file);
  const next = lib.upsertRow(book, "Supabase", "super base", null);
  lib.saveWordBook(next, file);
  const saved = fs.readFileSync(file, "utf8");
  assert.match(saved, /Supabase/);
  assert.match(saved, /super base/);
  assert.match(saved, /\[\[snippets\]\]/);
  assert.match(saved, /style = "concise"/);
  const rows = lib.rowsFromBook(lib.loadWordBook(file));
  const supabase = rows.find((r) => r.word === "Supabase");
  assert.equal(supabase.misspelling, "super base");
}

{
  let book = lib.normalizeBook({ preferred_words: [], starred_words: [], dictionary: {} });
  book = lib.upsertRow(book, "Hyprland", null, null);
  assert.equal(lib.validateNewWord(book, "hyprland"), "That word is already in your dictionary.");
  book = lib.toggleStar(book, "Hyprland");
  assert.equal(lib.rowsFromBook(book)[0].starred, true);
  book = lib.removePreferredWord(book, "Hyprland");
  assert.equal(book.preferred_words.length, 0);
}

{
  const entries = lib.parseHistoryJsonl(
    `{"ts":1,"text":"hello","profile":"segmented"}\nnot-json\n{"ts":2,"text":"world","profile":"segmented"}\n`,
    10,
  );
  assert.equal(entries.length, 2);
  assert.equal(entries[1].text, "world");
}

{
  const lines = lib.parseDoctor("dictate doctor\n\n✓ Config file: C:\\\\a\n✗ MISTRAL_API_KEY missing\n  Profile: segmented\n");
  assert.equal(lines[0].kind, "ok");
  assert.equal(lines[1].kind, "bad");
  assert.equal(lines[2].kind, "info");
}

console.log("ok");
