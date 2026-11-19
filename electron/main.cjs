const { app, BrowserWindow, desktopCapturer, net, protocol, session, ipcMain, globalShortcut, Tray, Menu, nativeImage, shell, screen } = require("electron");
const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");
const { pathToFileURL } = require("node:url");

const isDev = !app.isPackaged;
if (process.env.PET_DISABLE_GPU === "1") app.disableHardwareAcceleration();
let petWindow;
let consoleWindow;
let tray;
let locked = false;
let quitting = false;

function showConsole() {
  if (consoleWindow && !consoleWindow.isDestroyed()) {
    consoleWindow.show();
    consoleWindow.focus();
  }
}

function setLocked(value) {
  locked = Boolean(value);
  if (petWindow && !petWindow.isDestroyed()) {
    petWindow.setMovable(!locked);
    petWindow.setIgnoreMouseEvents(locked, { forward: true });
    petWindow.webContents.send("pet:locked", locked);
  }
  tray?.setContextMenu(Menu.buildFromTemplate([
    { label: "打开控制台", click: showConsole },
    { label: locked ? "解除桌宠锁定" : "锁定桌宠（点击穿透）", click: () => setLocked(!locked) },
    { type: "separator" },
    { label: "退出桌宠节奏师", click: () => app.quit() },
  ]));
}

protocol.registerSchemesAsPrivileged([
  {
    scheme: "pet-asset",
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
      corsEnabled: true,
    },
  },
]);

function getExternalAssetDir() {
  return app.isPackaged
    ? path.join(path.dirname(process.execPath), "character_assets")
    : path.join(__dirname, "../public/character_assets");
}

function registerPetAssetProtocol() {
  protocol.handle("pet-asset", async (request) => {
    let relPath = "";
    try {
      const parsedUrl = new URL(request.url);
      relPath = decodeURIComponent(parsedUrl.pathname).replace(/^\/+/, "");
    } catch {
      return new Response("Bad Request", {
        status: 400,
        headers: { "Cache-Control": "no-store" },
      });
    }

    if (!relPath || relPath.includes("\0")) {
      return new Response("Forbidden", {
        status: 403,
        headers: { "Cache-Control": "no-store" },
      });
    }

    const candidateDirs = app.isPackaged
      ? [
          path.join(path.dirname(process.execPath), "character_assets"),
          path.join(__dirname, "../dist/character_assets"),
        ]
      : [path.join(__dirname, "../public/character_assets")];

    for (const baseDir of candidateDirs) {
      const resolvedBase = path.resolve(baseDir);
      const resolvedTarget = path.resolve(resolvedBase, relPath);
      const rel = path.relative(resolvedBase, resolvedTarget);
      if (!rel || rel.startsWith("..") || path.isAbsolute(rel)) {
        return new Response("Forbidden", {
          status: 403,
          headers: { "Cache-Control": "no-store" },
        });
      }

      try {
        if (fs.existsSync(resolvedTarget) && fs.statSync(resolvedTarget).isFile()) {
          const response = await net.fetch(pathToFileURL(resolvedTarget).toString());
          const headers = new Headers(response.headers);
          headers.set("Cache-Control", "no-store");
          headers.set("Access-Control-Allow-Origin", "*");
          return new Response(response.body, {
            status: response.status,
            statusText: response.statusText,
            headers,
          });
        }
      } catch {
        // Continue to next candidate or return 404
      }
    }

    return new Response("Not Found", {
      status: 404,
      headers: { "Cache-Control": "no-store" },
    });
  });
}

function configureMediaPermissions() {
  session.defaultSession.setPermissionCheckHandler((_webContents, permission) => {
    return permission === "media" || permission === "display-capture";
  });

  session.defaultSession.setPermissionRequestHandler((_webContents, permission, callback) => {
    callback(permission === "media" || permission === "display-capture");
  });

  session.defaultSession.setDisplayMediaRequestHandler(async (request, callback) => {
    try {
      const sources = await desktopCapturer.getSources({
        types: ["screen", "window"],
        thumbnailSize: { width: 0, height: 0 },
      });
      const source = sources.find((item) => item.id.startsWith("screen:")) ?? sources[0];
      if (!source) {
        callback({});
        return;
      }

      callback({
        video: source,
        ...(request.audioRequested ? { audio: "loopback" } : {}),
      });
    } catch {
      callback({});
    }
  });
}

function isViteServerReachable(urlString) {
  return new Promise((resolve) => {
    const req = http.get(urlString, { timeout: 350 }, (res) => {
      res.resume();
      resolve(typeof res.statusCode === "number" && res.statusCode < 500);
    });
    req.on("timeout", () => {
      req.destroy();
      resolve(false);
    });
    req.on("error", () => {
      resolve(false);
    });
  });
}

async function createWindow() {
  const window = new BrowserWindow({
    width: 540,
    height: 590,
    resizable: false,
    frame: false,
    transparent: true,
    hasShadow: false,
    alwaysOnTop: true,
    title: "桌宠节奏师",
    backgroundColor: "#00000000",
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
      backgroundThrottling: false,
      additionalArguments: [`--pet-external-asset-dir=${getExternalAssetDir()}`],
    },
  });
  petWindow = window;
  window.on("close", () => { if (!quitting) app.quit(); });
  window.webContents.setWindowOpenHandler(({ url, frameName }) => {
    if (url !== "about:blank" || frameName !== "pet-console") return { action: "deny" };
    const bounds = window.getBounds();
    const area = screen.getDisplayMatching(bounds).workArea;
    const preferredX = bounds.x + bounds.width + 12;
    return {
      action: "allow",
      overrideBrowserWindowOptions: {
        width: 470, height: 780, minWidth: 430, minHeight: 500,
        x: Math.max(area.x, Math.min(preferredX, area.x + area.width - 470)),
        y: Math.max(area.y, Math.min(bounds.y, area.y + area.height - 780)),
        alwaysOnTop: true,
        title: "桌宠节奏师 · 控制台", frame: false, transparent: false,
        backgroundColor: "#fff9fc", autoHideMenuBar: true,
        webPreferences: { contextIsolation: true, nodeIntegration: false, backgroundThrottling: false },
      },
    };
  });
  window.webContents.on("did-create-window", (child) => {
    consoleWindow = child;
    child.setMenu(null);
    child.on("close", (event) => {
      if (!quitting) { event.preventDefault(); child.hide(); }
    });
    child.on("closed", () => { consoleWindow = undefined; });
  });

  if (isDev) {
    const devUrl = process.env.VITE_DEV_SERVER_URL || "http://127.0.0.1:5173";
    const useDevServer =
      Boolean(process.env.VITE_DEV_SERVER_URL) ||
      (!process.env.ELECTRON_FORCE_DIST && (await isViteServerReachable(devUrl)));

    if (useDevServer) {
      await window.loadURL(devUrl);
    } else {
      await window.loadFile(path.join(__dirname, "../dist/index.html"));
    }

    if (process.env.OPEN_DEVTOOLS === "1" && !process.env.PET_E2E_DRIVER) {
      window.webContents.openDevTools({ mode: "detach" });
    }
  } else {
    await window.loadFile(path.join(__dirname, "../dist/index.html"));
  }

  if (!app.isPackaged && process.env.PET_E2E_DRIVER) {
    const driverPath = path.resolve(process.env.PET_E2E_DRIVER);
    const driver = require(driverPath);
    if (typeof driver === "function") {
      await driver(window, app);
    }
  }
}

app.whenReady().then(async () => {
  ipcMain.handle("pet:set-locked", (event, value) => {
    if (event.sender !== petWindow?.webContents) return locked;
    setLocked(value);
    return locked;
  });
  ipcMain.handle("pet:get-locked", () => locked);
  ipcMain.on("pet:show-console", showConsole);
  ipcMain.on("pet:hide-console", () => consoleWindow?.hide());
  ipcMain.on("pet:quit", () => app.quit());
  ipcMain.on("pet:sound-settings", () => {
    if (process.platform === "win32") void shell.openExternal("ms-settings:sound");
  });
  registerPetAssetProtocol();
  configureMediaPermissions();
  tray = new Tray(nativeImage.createFromPath(path.join(__dirname, "../dist/character_assets/head_base.png")).resize({ width: 24, height: 24 }));
  tray.setToolTip("桌宠节奏师");
  tray.on("double-click", showConsole);
  await createWindow();
  setLocked(false);
  globalShortcut.register("CommandOrControl+Alt+L", () => setLocked(!locked));
  globalShortcut.register("CommandOrControl+Alt+C", showConsole);

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      void createWindow();
    }
  });
});

app.on("before-quit", () => { quitting = true; });
app.on("will-quit", () => { globalShortcut.unregisterAll(); tray?.destroy(); });

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
