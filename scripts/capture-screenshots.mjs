#!/usr/bin/env node
/**
 * Captures README screenshots using Playwright.
 * Run from repo root: npm run screenshots
 * If port 1420 is free, start dev server first: npm run dev (then run this in another terminal).
 * If 1420 is in use, this script starts Vite on port 1421 automatically.
 */
import { mkdir } from "fs";
import { get } from "http";
import { spawn } from "child_process";
import { fileURLToPath } from "url";
import { dirname, join } from "path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const SCREENSHOTS_DIR = join(ROOT, "screenshots");
const PORT = Number(process.env.PORT) || 1420;
const FALLBACK_PORTS = [1520, 1521, 1522];
const HOST = process.env.HOST || "localhost";
const BASE = (p) => `http://${HOST}:${p}`;

function waitForPort(port, timeoutMs = 30_000) {
  const start = Date.now();
  return new Promise((resolve, reject) => {
    const tryConnect = () => {
      const req = get(`http://${HOST}:${port}/`, (res) => {
        res.resume();
        resolve();
      });
      req.on("error", () => {
        if (Date.now() - start > timeoutMs) reject(new Error(`Port ${port} not ready in time`));
        else setTimeout(tryConnect, 500);
      });
      req.setTimeout(3000, () => { req.destroy(); });
    };
    setTimeout(tryConnect, 2000);
  });
}

async function portReady(port, timeoutMs = 3000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      await new Promise((resolve, reject) => {
        const req = get(`http://${HOST}:${port}/`, (res) => { res.resume(); resolve(); });
        req.on("error", reject);
        req.setTimeout(2000, () => { req.destroy(); reject(new Error("timeout")); });
      });
      return true;
    } catch {
      await new Promise((r) => setTimeout(r, 200));
    }
  }
  return false;
}

async function main() {
  await new Promise((resolve, reject) => mkdir(SCREENSHOTS_DIR, { recursive: true }, (err) => (err ? reject(err) : resolve())));

  let serverProcess = null;
  let port = PORT;
  if (await portReady(PORT)) {
    console.log("Using existing dev server at", BASE(PORT));
  } else {
    for (const p of FALLBACK_PORTS) {
      port = p;
      console.log("Starting Vite on port", port, "...");
      serverProcess = spawn("npx", ["vite", "--port", String(port)], {
        cwd: ROOT,
        stdio: ["ignore", "pipe", "pipe"],
        shell: true,
      });
      serverProcess.stdout?.on("data", (d) => process.stdout.write(d));
      serverProcess.stderr?.on("data", (d) => process.stderr.write(d));
      await new Promise((r) => setTimeout(r, 3000));
      if (serverProcess.exitCode != null) {
        serverProcess = null;
        continue;
      }
      try {
        await waitForPort(port);
        break;
      } catch {
        serverProcess?.kill();
        serverProcess = null;
      }
    }
    if (!serverProcess) {
      throw new Error("Could not start Vite. Run 'npm run dev' in another terminal, then run 'npm run screenshots'.");
    }
  }
  const base = BASE(port);

  const { chromium } = await import("playwright");
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    deviceScaleFactor: 1,
  });
  const page = await context.newPage();

  const capture = async (name, path) => {
    await page.goto(path, { waitUntil: "domcontentloaded", timeout: 15_000 });
    await page.waitForTimeout(800);
    const out = join(SCREENSHOTS_DIR, `${name}.png`);
    await page.screenshot({ path: out, type: "png" });
    console.log("Saved:", out);
  };

  // 1. Loading screen (no Tauri; main.tsx renders only LoadingScreen)
  await capture("loading", `${base}/?screenshot=loading`);

  // 2. Home: wait for init to finish (invoke fails in browser; finally sets isRefreshing false)
  await page.goto(base, { waitUntil: "domcontentloaded", timeout: 15_000 });
  await page.waitForTimeout(3500);
  await page.screenshot({ path: join(SCREENSHOTS_DIR, "home.png"), type: "png" });
  console.log("Saved:", join(SCREENSHOTS_DIR, "home.png"));

  // 3. Browse: scroll main content to show category grid
  const scrollEl = page.locator("main div.overflow-y-auto").first();
  if ((await scrollEl.count()) > 0) {
    await scrollEl.evaluate((el) => (el.scrollTop = 400));
    await page.waitForTimeout(400);
    await scrollEl.evaluate((el) => (el.scrollTop = 900));
    await page.waitForTimeout(400);
  }
  await page.screenshot({ path: join(SCREENSHOTS_DIR, "browse.png"), type: "png" });
  console.log("Saved:", join(SCREENSHOTS_DIR, "browse.png"));

  // 4. Library (Installed)
  await page.getByRole("button", { name: "Installed" }).click();
  await page.waitForTimeout(800);
  await page.screenshot({ path: join(SCREENSHOTS_DIR, "library.png"), type: "png" });
  console.log("Saved:", join(SCREENSHOTS_DIR, "library.png"));

  // 5. Settings
  await page.getByRole("button", { name: "Settings" }).click();
  await page.waitForTimeout(800);
  await page.screenshot({ path: join(SCREENSHOTS_DIR, "settings.png"), type: "png" });
  console.log("Saved:", join(SCREENSHOTS_DIR, "settings.png"));

  await browser.close();
  if (serverProcess) serverProcess.kill();
  console.log("Done.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
