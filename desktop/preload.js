"use strict";

const { contextBridge, ipcRenderer } = require("electron");

async function invoke(channel, ...args) {
  const result = await ipcRenderer.invoke(channel, ...args);
  if (result && result.ok === false) {
    throw new Error(result.error || channel);
  }
  return result;
}

contextBridge.exposeInMainWorld("dictate", {
  getState: () => invoke("state"),
  toggle: () => invoke("toggle"),
  setConfig: (key, value) => invoke("set-config", key, value),
  setPolish: (patch) => invoke("set-polish", patch),
  words: {
    upsert: (row) => invoke("words:upsert", row),
    remove: (word) => invoke("words:remove", word),
    star: (word) => invoke("words:star", word),
  },
  history: {
    clear: () => invoke("history:clear"),
  },
  doctor: () => invoke("doctor"),
  autostart: (action) => invoke("autostart", action),
  pickBinary: () => invoke("pick-binary"),
  openConfig: () => invoke("open-config"),
  onChange: (cb) => {
    const listener = (_event, state) => cb(state);
    ipcRenderer.on("changed", listener);
    return () => ipcRenderer.removeListener("changed", listener);
  },
});
