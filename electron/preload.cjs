const { contextBridge } = require("electron");
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
});
