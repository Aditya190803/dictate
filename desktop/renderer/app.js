"use strict";

const tabs = {
  listen: document.getElementById("listen"),
  words: document.getElementById("words"),
  setup: document.getElementById("setup"),
};

let state = null;
let editing = null;
let formDirty = false;
let busy = false;

function $(id) {
  return document.getElementById(id);
}

function showBanner(message) {
  const banner = $("banner");
  if (!message) {
    banner.hidden = true;
    banner.textContent = "";
    return;
  }
  banner.hidden = false;
  banner.textContent = message;
}

function when(ts) {
  if (!ts) return "";
  return new Date(ts * 1000).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

let doctorRan = false;

function switchTab(name) {
  for (const [key, panel] of Object.entries(tabs)) {
    const selected = key === name;
    panel.hidden = !selected;
    $(`tab-${key}`).setAttribute("aria-selected", selected ? "true" : "false");
  }
  if (name === "setup" && !doctorRan) {
    doctorRan = true;
    $("run-doctor").click();
  }
}

function applyConfigFields(config) {
  const form = $("setup-form");
  if (!form || formDirty) return;
  for (const [key, value] of Object.entries(config)) {
    const field = form.elements[key];
    if (field && document.activeElement !== field) field.value = value;
  }
  const provider = config.TRANSCRIPTION_PROVIDER || "mistral";
  for (const key of ["MISTRAL_API_KEY", "GROQ_API_KEY", "DEEPGRAM_API_KEY"]) {
    const wrap = form.querySelector(`[data-key="${key}"]`);
    if (!wrap) continue;
    const needed =
      (key === "MISTRAL_API_KEY" && provider === "mistral") ||
      (key === "GROQ_API_KEY" && provider === "groq") ||
      (key === "DEEPGRAM_API_KEY" && provider === "deepgram");
    wrap.hidden = !needed;
  }
}

function renderListen(s) {
  const live = Boolean(s.listening);
  $("pulse").textContent = live ? "Listening" : "Idle";
  $("pulse").classList.toggle("is-live", live);
  $("listen-btn").classList.toggle("is-live", live);
  $("listen-label").textContent = live ? "Stop" : "Listen";
  $("listen-hint").textContent = s.shortcut
    ? `${s.shortcut.replace(/\+/g, "+")} · ${s.config.SHORTCUT_OUTPUT || "type"}`
    : "Same as your shortcut";
  $("listen-btn").disabled = busy;

  const latest = s.history[0];
  const take = $("take-text");
  const copy = $("copy-take");
  if (latest) {
    take.textContent = latest.text;
    take.classList.remove("is-empty");
    $("take-when").textContent = when(latest.ts);
    copy.hidden = false;
  } else {
    take.textContent = "Speak, press again, text lands in the focused app.";
    take.classList.add("is-empty");
    $("take-when").textContent = "";
    copy.hidden = true;
  }

  const list = $("history");
  if (!s.history.length) {
    list.innerHTML = `<li class="empty">Takes land here after you stop.</li>`;
    return;
  }
  list.innerHTML = s.history
    .slice(0, 12)
    .map(
      (entry) =>
        `<li><p>${escapeHtml(entry.text)}</p><p class="when">${escapeHtml(when(entry.ts))}</p></li>`,
    )
    .join("");
}

function renderWords(s) {
  const filter = $("word-filter").value.trim().toLowerCase();
  const rows = s.words.filter((row) => {
    if (!filter) return true;
    return (
      row.word.toLowerCase().includes(filter) ||
      (row.misspelling && row.misspelling.toLowerCase().includes(filter))
    );
  });
  const list = $("word-list");
  if (!rows.length) {
    list.innerHTML = `<li class="empty">${
      s.words.length
        ? "No matches."
        : "Add names you say often. Dictate will keep the spelling."
    }</li>`;
    return;
  }
  list.innerHTML = rows
    .map(
      (row) => `<li data-word="${escapeAttr(row.word)}">
        <button type="button" class="star ${row.starred ? "is-on" : ""}" data-act="star" aria-label="Star ${escapeAttr(row.word)}">${row.starred ? "★" : "☆"}</button>
        <button type="button" class="name" data-act="edit">
          <strong>${escapeHtml(row.word)}</strong>
          ${row.misspelling ? `<span>heard as ${escapeHtml(row.misspelling)}</span>` : ""}
        </button>
        <button type="button" class="kill" data-act="remove" aria-label="Remove ${escapeAttr(row.word)}">Remove</button>
      </li>`,
    )
    .join("");
}

function renderSetup(s) {
  $("binary-path").textContent = s.binary || "dictate was not found on PATH";
  applyConfigFields(s.config);
  if (!formDirty) {
    $("polish-enabled").value = s.polish.enabled ? "true" : "false";
    $("polish-style").value = s.polish.style || "";
  }
}

function render(s) {
  state = s;
  showBanner(s.error);
  renderListen(s);
  renderWords(s);
  renderSetup(s);
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function escapeAttr(value) {
  return escapeHtml(value);
}

function fillWordForm(row) {
  editing = row ? row.word : null;
  $("word-form-title").textContent = row ? "Edit entry" : "Add a name";
  $("word-input").value = row ? row.word : "";
  $("word-wrong-on").checked = Boolean(row && row.misspelling);
  $("word-wrong").value = row && row.misspelling ? row.misspelling : "";
  $("word-wrong-wrap").hidden = !$("word-wrong-on").checked;
  $("word-cancel").hidden = !row;
}

async function withBusy(fn) {
  busy = true;
  $("listen-btn").disabled = true;
  try {
    await fn();
  } catch (error) {
    showBanner(error.message);
  } finally {
    busy = false;
    if (state) $("listen-btn").disabled = false;
  }
}

document.querySelectorAll(".tabs button").forEach((btn) => {
  btn.addEventListener("click", () => switchTab(btn.id.replace("tab-", "")));
});

$("listen-btn").addEventListener("click", () => withBusy(() => window.dictate.toggle()));

$("copy-take").addEventListener("click", async () => {
  if (!state || !state.history[0]) return;
  await navigator.clipboard.writeText(state.history[0].text);
});

$("clear-history").addEventListener("click", () =>
  withBusy(() => window.dictate.history.clear()),
);

$("word-wrong-on").addEventListener("change", () => {
  $("word-wrong-wrap").hidden = !$("word-wrong-on").checked;
});

$("word-cancel").addEventListener("click", () => fillWordForm(null));

$("word-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const word = $("word-input").value.trim();
  const misspelling = $("word-wrong-on").checked ? $("word-wrong").value.trim() : "";
  if ($("word-wrong-on").checked && !misspelling) {
    showBanner("Enter the misspelling Dictate produces, or turn the option off.");
    return;
  }
  withBusy(async () => {
    await window.dictate.words.upsert({
      word,
      misspelling,
      previous: editing,
    });
    fillWordForm(null);
  });
});

$("word-filter").addEventListener("input", () => {
  if (state) renderWords(state);
});

$("word-list").addEventListener("click", (event) => {
  const button = event.target.closest("button");
  const row = event.target.closest("li");
  if (!button || !row) return;
  const word = row.dataset.word;
  const act = button.dataset.act;
  if (act === "star") withBusy(() => window.dictate.words.star(word));
  if (act === "remove") withBusy(() => window.dictate.words.remove(word));
  if (act === "edit") {
    const found = state.words.find((item) => item.word === word);
    fillWordForm(found);
    $("word-input").focus();
  }
});

const setupForm = $("setup-form");
setupForm.addEventListener("input", () => {
  formDirty = true;
});

setupForm.addEventListener("change", (event) => {
  const field = event.target;
  if (!field.name) return;
  withBusy(async () => {
    await window.dictate.setConfig(field.name, field.value);
    formDirty = false;
  });
});

$("polish-enabled").addEventListener("change", () => {
  withBusy(async () => {
    await window.dictate.setPolish({ enabled: $("polish-enabled").value === "true" });
    formDirty = false;
  });
});

$("polish-style").addEventListener("change", () => {
  withBusy(async () => {
    await window.dictate.setPolish({ style: $("polish-style").value });
    formDirty = false;
  });
});

$("pick-binary").addEventListener("click", () => withBusy(() => window.dictate.pickBinary()));
$("open-config").addEventListener("click", () => withBusy(() => window.dictate.openConfig()));
$("autostart-on").addEventListener("click", () =>
  withBusy(() => window.dictate.autostart("install")),
);
$("autostart-off").addEventListener("click", () =>
  withBusy(() => window.dictate.autostart("remove")),
);

$("run-doctor").addEventListener("click", () =>
  withBusy(async () => {
    const result = await window.dictate.doctor();
    $("doctor").innerHTML = (result.lines || [])
      .map((line) => `<li class="${line.kind}">${escapeHtml(line.text)}</li>`)
      .join("") || `<li class="empty">No doctor output.</li>`;
  }),
);

document.addEventListener("keydown", (event) => {
  if (event.target.closest("input, select, textarea")) return;
  if (event.key === "1") switchTab("listen");
  if (event.key === "2") switchTab("words");
  if (event.key === "3") switchTab("setup");
  if (event.code === "Space" && !tabs.listen.hidden) {
    event.preventDefault();
    $("listen-btn").click();
  }
});

window.dictate.onChange(render);
window.dictate
  .getState()
  .then(render)
  .catch((error) => showBanner(error.message));
