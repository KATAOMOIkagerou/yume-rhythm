const { contextBridge, ipcRenderer } = require("electron");
const path = require("node:path");

function resolveExternalAssetDir() {
  const prefix = "--pet-external-asset-dir=";
  const arg = process.argv.find((item) => typeof item === "string" && item.startsWith(prefix));
  if (arg) {
    return arg.slice(prefix.length);
  }
  return process.defaultApp
    ? path.resolve(__dirname, "../public/character_assets")
    : path.join(path.dirname(process.execPath), "character_assets");
}

contextBridge.exposeInMainWorld("desktopPetBridge", {
  isElectron: true,
  assetProtocolBase: "pet-asset://local",
  externalAssetDir: resolveExternalAssetDir(),
  setLocked: (value) => ipcRenderer.invoke("pet:set-locked", value),
  getLocked: () => ipcRenderer.invoke("pet:get-locked"),
  onLocked: (callback) => {
    const listener = (_event, value) => callback(value);
    ipcRenderer.on("pet:locked", listener);
    return () => ipcRenderer.removeListener("pet:locked", listener);
  },
  showConsole: () => ipcRenderer.send("pet:show-console"),
  hideConsole: () => ipcRenderer.send("pet:hide-console"),
  quit: () => ipcRenderer.send("pet:quit"),
  openSoundSettings: () => ipcRenderer.send("pet:sound-settings"),
});
