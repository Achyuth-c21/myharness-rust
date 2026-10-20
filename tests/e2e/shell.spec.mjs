/**
 * Phase 1 shell E2E — EXERCISES the renderer like a user (headless chromium).
 * Cases trace to docs/06–08 (UI-INT-*) and docs/02/20; TEST-ARCH-001 spirit included.
 *
 * Prereqs: `npm install` here (root), `npx playwright install chromium`,
 * renderer dev server: `npm run dev` in apps/desktop (port 5173, has SPA fallback).
 * Run: node tests/e2e/shell.spec.mjs
 */
import { chromium } from "playwright";

const BASE = process.env.BASE_URL ?? "http://localhost:5173";

const results = [];
let failures = 0;

async function step(name, fn) {
  try {
    await fn();
    results.push(`PASS ${name}`);
  } catch (error) {
    failures += 1;
    results.push(`FAIL ${name}\n      ${String(error).split("\n")[0]}`);
  }
}

async function expectTrue(cond, message) {
  if (!cond) throw new Error(message);
}

const browser = await chromium.launch();
try {
  // colorScheme: dark emulates an OS preference for deterministic theme checks.
  const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 800 } });
  const page = await context.newPage();

  await step("T01 root redirects to /new (docs/06 §1)", async () => {
    await page.goto(BASE);
    await page.waitForURL("**/new");
    await expectTrue(page.url().endsWith("/new"), `URL was ${page.url()}`);
  });

  await step("T02 document title + root data attrs (docs/07 §2.1)", async () => {
    await expectTrue(
      (await page.title()) === "myharness",
      `title=${await page.title()}`,
    );
    await expectTrue(
      (await page.getAttribute("html", "data-theme")) === "claude",
      "data-theme must be claude",
    );
    await expectTrue(
      (await page.getAttribute("html", "data-mode")) === "dark",
      "system theme with dark OS emulation must resolve dark",
    );
  });

  await step("T03 TEST-ARCH-001: renderer has no Node surface", async () => {
    const hasProcess = await page.evaluate(() => typeof window.process !== "undefined");
    await expectTrue(!hasProcess, "window.process must be undefined in the renderer");
  });

  await step("T04 sidebar shows exactly the evidenced items (docs/06 §2)", async () => {
    const sidebar = page.getByRole("navigation", { name: "Primary" });
    for (const label of ["New", "Projects", "Artifacts", "Scheduled"]) {
      await expectTrue(await sidebar.getByText(label, { exact: true }).count() === 1, `missing ${label}`);
    }
    const customize = sidebar.getByRole("button", { name: "Customize" });
    await expectTrue(await customize.isDisabled(), "Customize must be disabled (route unobserved, docs/06)");
  });

  await step("T05 Ctrl+N returns to /new from a deep route (UI-INT-005)", async () => {
    await page.getByRole("navigation", { name: "Primary" }).getByText("Projects").click();
    await page.waitForURL("**/projects");
    await page.keyboard.press("Control+n");
    await page.waitForURL("**/new");
  });

  await step("T06 nav click + active fill (UI-INT-006)", async () => {
    await page.getByRole("navigation", { name: "Primary" }).getByText("Artifacts").click();
    await page.waitForURL("**/cowork-artifact");
    const activeRows = await page.locator("a.nav-item.active").allInnerTexts();
    await expectTrue(
      activeRows.some((t) => t.includes("Artifacts")),
      `active row must be Artifacts, got: ${activeRows.join("|")}`,
    );
  });

  await step("T07 segmented Cowork|Code switch routes to /epitaxy (UI-INT-004)", async () => {
    await page.getByRole("tab", { name: "Code" }).click();
    await page.waitForURL("**/epitaxy");
    await expectTrue(await page.getByRole("tab", { name: "Code" }).getAttribute("aria-selected") === "true", "Code must be selected");
    await page.getByRole("tab", { name: "Cowork" }).click();
    await page.waitForURL("**/new");
  });

  await step("T08 back arrow navigates history (UI-INT-003)", async () => {
    await page.getByRole("button", { name: "Back" }).click();
    await page.waitForURL("**/epitaxy");
  });

  await step("T09 Ctrl+K opens palette; Enter opens row (UI-INT-020/023)", async () => {
    await page.keyboard.press("Control+k");
    await page.getByPlaceholder("Search locally…").fill("cow");
    await expectTrue(
      (await page.locator(".palette-row").count()) === 1,
      "query 'cow' must shrink results to Cowork",
    );
    await page.keyboard.press("Enter");
    await page.waitForURL("**/cowork");
  });

  await step("T10 palette 'No results' + Escape closes (UI-INT-027/026)", async () => {
    await page.keyboard.press("Control+k");
    await page.getByPlaceholder("Search locally…").fill("zzzz");
    await expectTrue(await page.getByText("No results").count() === 1, "No results must show");
    await page.keyboard.press("Escape");
    await expectTrue(await page.getByPlaceholder("Search locally…").count() === 0, "palette must close");
  });

  await step("T11 Ctrl+, opens settings hash modal on current route (docs/06 §1)", async () => {
    await page.keyboard.press("Control+,");
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await expectTrue(page.url().includes("#settings/general"), `hash=${page.url()}`);
    await page.getByRole("button", { name: "usage" }).click();
    await expectTrue(page.url().includes("#settings/usage"), "section click must move hash");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(50);
    await expectTrue(!page.url().includes("#settings"), `hash must clear, url=${page.url()}`);
    await expectTrue(new URL(page.url()).pathname === "/cowork", "underlying route must be preserved");
  });

  await step("T12 theme persists via `theme` key (docs/20 §4)", async () => {
    await page.evaluate(() => localStorage.setItem("theme", "light"));
    await page.reload();
    await expectTrue(
      (await page.getAttribute("html", "data-mode")) === "light",
      "persisted light must prevail over OS dark emulation",
    );
    await expectTrue((await page.evaluate(() => localStorage.getItem("theme"))) === "light", "theme key must persist");
  });

  await step("T13 Ctrl+B collapses sidebar and persists (UI-INT-001, docs/20)", async () => {
    await expectTrue(await page.getByRole("navigation", { name: "Primary" }).isVisible(), "sidebar starts visible");
    await page.keyboard.press("Control+b");
    await page.waitForTimeout(50);
    await expectTrue(await page.getByRole("navigation", { name: "Primary" }).count() === 0, "sidebar must unmount");
    await expectTrue((await page.evaluate(() => localStorage.getItem("sidebar.collapsed"))) === "1", "collapsed state persisted");
    await page.keyboard.press("Control+b");
    await page.waitForSelector("[aria-label=Primary]");
  });
} finally {
  await browser.close();
}

console.log(results.join("\n"));
console.log(failures === 0 ? "E2E SUMMARY: ALL PASS" : `E2E SUMMARY: ${failures} FAILURE(S)`);
process.exitCode = failures === 0 ? 0 : 1;
