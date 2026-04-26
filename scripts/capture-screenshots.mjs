#!/usr/bin/env node

import { execFile, spawn } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = path.resolve(SCRIPT_DIR, "..");

const BASE_URL = process.env.SCREENSHOT_BASE_URL || "http://127.0.0.1:1420";
const CDP_PORT = Number(process.env.SCREENSHOT_CDP_PORT || "9333");
const CDP_URL = `http://127.0.0.1:${CDP_PORT}`;
const OUT_DIR = path.resolve(
  ROOT_DIR,
  process.env.SCREENSHOT_OUT_DIR || "docs/screenshots",
);
const STARTUP_TIMEOUT_MS = Number(
  process.env.SCREENSHOT_STARTUP_TIMEOUT_MS || "180000",
);
const ROUTE_WAIT_MS = Number(process.env.SCREENSHOT_WAIT_MS || "1400");
const SKIP_LAUNCH = process.env.SCREENSHOT_SKIP_LAUNCH === "1";
const VERBOSE = process.env.SCREENSHOT_VERBOSE === "1";

const routes = [
  { path: "/dashboard", file: "dashboard.png", title: "Dashboard" },
  { path: "/focus", file: "focus.png", title: "Focus" },
  { path: "/records", file: "records.png", title: "Records" },
  { path: "/charts", file: "charts.png", title: "Charts" },
  { path: "/countdown", file: "countdown.png", title: "Countdown" },
  { path: "/milestones", file: "milestones.png", title: "Milestones" },
  {
    path: "/milestones/categories",
    file: "milestone-categories.png",
    title: "Milestone categories",
  },
  { path: "/stages", file: "stages.png", title: "Stages" },
  { path: "/categories", file: "categories.png", title: "Categories" },
  {
    path: "/settings/data",
    file: "settings-data.png",
    title: "Settings data",
  },
  {
    path: "/settings/stages",
    file: "settings-stages.png",
    title: "Settings stages",
  },
  {
    path: "/settings/categories",
    file: "settings-categories.png",
    title: "Settings categories",
  },
  {
    path: "/settings/mottos",
    file: "settings-mottos.png",
    title: "Settings mottos",
  },
  {
    path: "/settings/about",
    file: "settings-about.png",
    title: "Settings about",
  },
];

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function execFileText(file, args) {
  return new Promise((resolve, reject) => {
    execFile(file, args, (error, stdout, stderr) => {
      if (error) {
        reject(error);
        return;
      }
      resolve({ stdout, stderr });
    });
  });
}

async function ensureNoRunningDesktopApp() {
  if (process.platform !== "win32" || SKIP_LAUNCH) return;

  const { stdout } = await execFileText("tasklist", [
    "/FI",
    "IMAGENAME eq yinghuoji_desktop.exe",
    "/FO",
    "CSV",
    "/NH",
  ]);
  const matches = stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.startsWith('"'));

  if (!matches.length) return;

  const processes = matches
    .map((line) => {
      const match = line.match(/^"([^"]+)","([^"]+)"/);
      return match ? `${match[1]}:${match[2]}` : line;
    })
    .join(", ");

  throw new Error(
    `Close running Yinghuoji windows before capturing screenshots. Running processes: ${processes}`,
  );
}

function appendWebViewDebugArgs(env) {
  const debugArgs = [
    `--remote-debugging-port=${CDP_PORT}`,
    "--remote-allow-origins=*",
  ];
  return {
    ...env,
    VITE_PORT: env.VITE_PORT || "1420",
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: [
      env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS,
      ...debugArgs,
    ]
      .filter(Boolean)
      .join(" "),
  };
}

async function createTauriScreenshotConfig() {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), "yinghuoji-screenshots-"));
  const configPath = path.join(tempDir, "tauri.screenshots.conf.json");
  const browserArgs = [
    `--remote-debugging-port=${CDP_PORT}`,
    "--remote-allow-origins=*",
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection",
  ].join(" ");

  const config = {
    app: {
      windows: [
        {
          title: "萤火集",
          width: 1440,
          height: 920,
          minWidth: 1180,
          minHeight: 760,
          resizable: true,
          additionalBrowserArgs: browserArgs,
        },
      ],
    },
  };

  await writeFile(configPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");

  return { configPath, tempDir };
}

function startTauriDev(configPath) {
  const tauriCli = path.join(
    ROOT_DIR,
    "node_modules",
    "@tauri-apps",
    "cli",
    "tauri.js",
  );
  const command = process.execPath;
  const args = [tauriCli, "dev", "--config", configPath];
  const child = spawn(command, args, {
    cwd: ROOT_DIR,
    env: appendWebViewDebugArgs(process.env),
    stdio: ["ignore", "pipe", "pipe"],
  });

  const recentLogs = [];
  const remember = (source, chunk) => {
    const text = chunk.toString();
    if (VERBOSE) {
      const target = source === "stderr" ? process.stderr : process.stdout;
      target.write(text);
    }

    for (const line of text.split(/\r?\n/).filter(Boolean)) {
      recentLogs.push(`${source}: ${line}`);
    }

    if (recentLogs.length > 80) {
      recentLogs.splice(0, recentLogs.length - 80);
    }
  };

  child.stdout.on("data", (chunk) => remember("stdout", chunk));
  child.stderr.on("data", (chunk) => remember("stderr", chunk));

  child.on("error", (error) => {
    recentLogs.push(`process error: ${error.message}`);
  });

  return { child, recentLogs };
}

function formatRecentLogs(launchState) {
  if (!launchState?.recentLogs.length) return "";
  return `\nRecent desktop:dev output:\n${launchState.recentLogs.join("\n")}`;
}

async function stopProcessTree(child) {
  if (!child || child.exitCode !== null) return;

  if (process.platform === "win32") {
    await new Promise((resolve) => {
      execFile(
        "taskkill",
        ["/PID", String(child.pid), "/T", "/F"],
        () => resolve(),
      );
    });
    return;
  }

  child.kill("SIGTERM");
  await delay(1000);
  if (child.exitCode === null) child.kill("SIGKILL");
}

async function waitForCdpEndpoint(launchState) {
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  let lastError = null;

  while (Date.now() < deadline) {
    if (launchState?.child.exitCode !== null) {
      throw new Error(
        `desktop:dev exited before WebView2 exposed ${CDP_URL}.${formatRecentLogs(
          launchState,
        )}`,
      );
    }

    try {
      const response = await fetch(`${CDP_URL}/json/version`);
      if (response.ok) return;
      lastError = new Error(`HTTP ${response.status}`);
    } catch (error) {
      lastError = error;
    }

    await delay(500);
  }

  throw new Error(
    `Timed out waiting for WebView2 CDP endpoint at ${CDP_URL}: ${
      lastError?.message || "unknown error"
    }.${formatRecentLogs(launchState)}`,
  );
}

async function waitForTauriPage(browser) {
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  const baseOrigin = new URL(BASE_URL).origin;

  while (Date.now() < deadline) {
    const pages = browser.contexts().flatMap((context) => context.pages());
    const page =
      pages.find((candidate) => candidate.url().startsWith(baseOrigin)) ||
      pages.find((candidate) => candidate.url() !== "about:blank");

    if (page) return page;
    await delay(300);
  }

  throw new Error("Connected to WebView2, but no Tauri page target was found.");
}

async function waitForAppReady(page) {
  await page.waitForLoadState("domcontentloaded");
  await page.waitForSelector("#app .page-wrapper", { timeout: 45000 });
  await page
    .waitForFunction(() => document.fonts?.status === "loaded", undefined, {
      timeout: 10000,
    })
    .catch(() => {});
  await page.waitForLoadState("networkidle", { timeout: 12000 }).catch(() => {});
  await page.waitForTimeout(ROUTE_WAIT_MS);
}

async function readUiPrefs(page) {
  return page.evaluate(() => ({
    sidebarCollapsed: localStorage.getItem("ll_sidebar_collapsed"),
    theme: localStorage.getItem("app-theme"),
  }));
}

async function writeUiPrefs(page, prefs) {
  await page.evaluate((values) => {
    const setOrRemove = (key, value) => {
      if (value === null || value === undefined) {
        localStorage.removeItem(key);
        return;
      }
      localStorage.setItem(key, value);
    };

    setOrRemove("ll_sidebar_collapsed", values.sidebarCollapsed);
    setOrRemove("app-theme", values.theme);
  }, prefs);
}

async function applyScreenshotPrefs(page) {
  await writeUiPrefs(page, {
    sidebarCollapsed: "0",
    theme: "paper-light",
  });
}

async function captureRoute(page, route) {
  const url = new URL(route.path, `${BASE_URL}/`).href;
  const outputPath = path.join(OUT_DIR, route.file);

  await page.goto(url, { waitUntil: "domcontentloaded", timeout: 45000 });
  await waitForAppReady(page);
  await page.screenshot({
    path: outputPath,
    fullPage: false,
    scale: "css",
    animations: "disabled",
    caret: "hide",
  });

  console.log(`Saved ${route.title}: ${path.relative(ROOT_DIR, outputPath)}`);
}

async function main() {
  await mkdir(OUT_DIR, { recursive: true });
  await ensureNoRunningDesktopApp();

  const tempConfig = SKIP_LAUNCH ? null : await createTauriScreenshotConfig();
  const launchState = SKIP_LAUNCH ? null : startTauriDev(tempConfig.configPath);
  let browser;
  let page;
  let originalPrefs;
  let prefsRestored = false;

  try {
    await waitForCdpEndpoint(launchState);
    browser = await chromium.connectOverCDP(CDP_URL);
    page = await waitForTauriPage(browser);
    page.setDefaultTimeout(45000);

    page.on("pageerror", (error) => {
      console.warn(`Page error: ${error.message}`);
    });

    await page.goto(new URL("/dashboard", `${BASE_URL}/`).href, {
      waitUntil: "domcontentloaded",
      timeout: 45000,
    });
    await waitForAppReady(page);
    originalPrefs = await readUiPrefs(page);
    await applyScreenshotPrefs(page);

    for (const route of routes) {
      await captureRoute(page, route);
    }

    await writeUiPrefs(page, originalPrefs);
    prefsRestored = true;
    console.log(`Saved ${routes.length} screenshots to ${path.relative(ROOT_DIR, OUT_DIR)}`);
  } finally {
    if (page && originalPrefs && !prefsRestored) {
      await writeUiPrefs(page, originalPrefs).catch(() => {});
    }

    if (browser) {
      await browser.close().catch(() => {});
    }

    if (launchState) {
      await stopProcessTree(launchState.child);
    }

    if (tempConfig) {
      await rm(tempConfig.tempDir, { recursive: true, force: true }).catch(
        () => {},
      );
    }
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
});
