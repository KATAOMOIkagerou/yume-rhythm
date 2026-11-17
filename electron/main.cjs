const { app, BrowserWindow, desktopCapturer, net, protocol, session } = require("electron");
const fs = require("node:fs");
const http = require("node:http");
const path = require("node:path");
const { pathToFileURL } = require("node:url");

const isDev = !app.isPackaged;

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
    width: 1120,
    height: 780,
    minWidth: 820,
    minHeight: 620,
    title: "桌宠节奏师",
    backgroundColor: "#fdf4f8",
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
  registerPetAssetProtocol();
  configureMediaPermissions();
  await createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      void createWindow();
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
