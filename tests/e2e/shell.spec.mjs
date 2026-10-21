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
    await page.getByPlaceholder("Search chats and projects").fill("cow");
    await expectTrue(
      (await page.locator(".palette-row").count()) === 1,
      "query 'cow' must shrink results to Cowork",
    );
    await page.keyboard.press("Enter");
    await page.waitForURL("**/cowork");
  });

  await step("T10 palette 'No results' + Escape closes (UI-INT-027/026)", async () => {
    await page.keyboard.press("Control+k");
    await page.getByPlaceholder("Search chats and projects").fill("zzzz");
    await expectTrue(await page.getByText("No results").count() === 1, "No results must show");
    await page.keyboard.press("Escape");
    await expectTrue(await page.getByPlaceholder("Search chats and projects").count() === 0, "palette must close");
  });

  await step("T11 Ctrl+, opens settings hash modal on current route (docs/06 §1)", async () => {
    await page.keyboard.press("Control+,");
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await expectTrue(page.url().includes("#settings/general"), `hash=${page.url()}`);
    await page.getByRole("button", { name: "Usage" }).click();
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

  await step("T14 mode switch is IN the sidebar (docs/06 §4, new-task.md layout)", async () => {
    await page.goto(`${BASE}/new`);
    await page.waitForURL("**/new");
    const navText = await page.getByRole("navigation", { name: "Primary" }).innerText();
    await expectTrue(navText.includes("Cowork") && navText.includes("Code"), "segmented control must render inside the sidebar");
  });

  await step("T15 /new greeting + composer copy (new-task.md + Run 5 reference)", async () => {
    await expectTrue(await page.getByRole("heading", { name: "You’re here!" }).count() === 1, "greeting heading");
    await expectTrue(await page.locator("#composer[placeholder='How can I help you today?']").count() === 1, "composer placeholder");
    await expectTrue(await page.getByText("Project or folder").count() === 1, "project/folder chip");
    // Empty home shows "Ideas for you" (Run 5 reference capture; the Active list
    // replaces it when session data exists — Phase 4).
    await expectTrue(await page.getByText("Ideas for you").count() === 1, "Ideas for you heading");
    await expectTrue(await page.locator(".idea-row").count() === 3, "three suggestion rows");
    await expectTrue(await page.getByText("Send me a daily briefing").count() === 1, "first suggestion");
    const composerBox = await page.locator(".composer-stack").boundingBox();
    await expectTrue(!!composerBox && composerBox.width <= 734, `composer must sit in the 734px column (got ${composerBox?.width})`);
  });

  await step("T16 palette geometry 726×470 (search-palette.md)", async () => {
    await page.keyboard.press("Control+k");
    const box = await page.locator(".palette").boundingBox();
    await expectTrue(!!box, "palette visible");
    await expectTrue(box.width === 726, `width must be 726 (got ${box.width})`);
    await expectTrue(box.height === 470, `height must be 470 (got ${box.height})`);
    await page.keyboard.press("Escape");
  });

  await step("T17 documented empty-state copy (docs/05 §4.4–4.6)", async () => {
    await page.goto(`${BASE}/projects`);
    await expectTrue(await page.getByText("Looking to start a project?").count() === 1, "projects headline");
    await page.goto(`${BASE}/cowork-artifact`);
    await expectTrue(await page.getByText("Create your first artifact").count() === 1, "artifacts headline");
    await page.goto(`${BASE}/scheduled-task`);
    await expectTrue(await page.getByText("No scheduled tasks yet.").count() === 1, "scheduled headline");
    await expectTrue(await page.getByText("awake and online", { exact: false }).count() >= 1, "scheduled info banner");
  });

  await step("T18 settings focus trap cycles + focus ring (UI-DS-04/05)", async () => {
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer"); // deterministic: shell mounted before keys
    await page.keyboard.press("Control+,");
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await expectTrue(
      await page.evaluate(() => document.activeElement?.classList.contains("settings-search") ?? false),
      "section search must take focus on open",
    );
    for (let i = 0; i < 14; i += 1) await page.keyboard.press("Tab");
    await expectTrue(
      await page.evaluate(() => !!document.activeElement?.closest(".settings-modal")),
      "Tab must cycle inside the modal",
    );
    const ring = await page.evaluate(() => {
      const el = document.activeElement;
      return el ? getComputedStyle(el).boxShadow : "";
    });
    await expectTrue(ring && ring !== "none", `focus ring shadow required, got "${ring}"`);
    await page.keyboard.press("Escape");
    await page.waitForTimeout(30);
    await expectTrue(await page.locator("[role=dialog][aria-label=Settings]").count() === 0, "Esc closes settings");
  });

  await step("T19 Ctrl+K over settings; Esc closes TOPMOST only (docs/08 §13)", async () => {
    await page.keyboard.press("Control+,");
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await page.keyboard.press("Control+k"); // palette must be reachable even over the modal
    await page.waitForSelector("[role=dialog][aria-label=Search]");
    await expectTrue(
      await page.locator("[role=dialog][aria-label=Settings]").isVisible(),
      "settings stays open beneath the palette",
    );
    await page.keyboard.press("Escape");
    await page.waitForTimeout(30);
    await expectTrue(await page.locator("[role=dialog][aria-label=Search]").count() === 0, "palette closes");
    await expectTrue(
      await page.locator("[role=dialog][aria-label=Settings]").isVisible(),
      "second Esc must NOT have closed settings",
    );
    await page.keyboard.press("Escape");
    await page.waitForTimeout(30);
    await expectTrue(!page.url().includes("#settings"), "then settings closes");
  });

  await step("T20 outside click (scrim) closes both overlay kinds", async () => {
    await page.keyboard.press("Control+,");
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await page.mouse.click(8, 450);
    await page.waitForTimeout(30);
    await expectTrue(await page.locator("[role=dialog][aria-label=Settings]").count() === 0, "settings closed via scrim");
    await page.keyboard.press("Control+k");
    await page.locator(".palette").waitFor({ state: "visible" });
    await page.mouse.click(8, 450);
    await page.waitForTimeout(30);
    await expectTrue(await page.locator("[role=dialog][aria-label=Search]").count() === 0, "palette closed via scrim");
  });

  await step("T21 palette focus trap + restore-to-trigger (UI-INT-026, docs/06 §5)", async () => {
    await page.getByRole("button", { name: "Search tasks" }).click();
    await page.locator(".palette").waitFor({ state: "visible" });
    await page.keyboard.press("Tab");
    await expectTrue(
      await page.evaluate(() => !!document.activeElement?.closest(".palette")),
      "Tab stays trapped in palette",
    );
    await page.keyboard.press("Escape");
    await page.waitForTimeout(30);
    const restored = await page.evaluate(() => document.activeElement?.getAttribute("aria-label"));
    await expectTrue(restored === "Search tasks", `focus must return to the trigger, got "${restored}"`);
  });

  await step("T22 palette clear button keeps palette open (search-palette.md)", async () => {
    await page.keyboard.press("Control+k");
    await page.getByPlaceholder("Search chats and projects").fill("cow");
    await page.getByRole("button", { name: "Clear search" }).click();
    await expectTrue(
      (await page.getByPlaceholder("Search chats and projects").inputValue()) === "",
      "X clears the query",
    );
    await expectTrue(await page.locator("[role=dialog][aria-label=Search]").count() === 1, "palette stays open");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(40);
    await expectTrue(await page.locator("[role=dialog][aria-label=Search]").count() === 0, "palette closed");
  });

  await step("T23 settings search filters, no-match state, Esc consumed first (settings.md)", async () => {
    await page.keyboard.press("Control+,");
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    // Determinism guard: no lingering palette layer from the previous step.
    if (await page.locator("[role=dialog][aria-label=Search]").count()) {
      await page.keyboard.press("Escape");
      await page.waitForTimeout(60);
    }
    const search = page.locator(".settings-search");
    await search.fill("pri");
    await expectTrue(
      (await page.locator(".settings-link:not([disabled])").count()) === 1,
      "'pri' matches only Privacy",
    );
    await expectTrue(
      (await page.locator(".settings-link:not([disabled])").first().innerText()) === "Privacy",
      "the matching link is Privacy",
    );
    await search.fill("qqqq");
    await expectTrue(await page.getByText("No results").count() === 1, "no-match state");
    await page.keyboard.press("Escape");
    await expectTrue((await search.inputValue()) === "", "Esc consumed: query cleared");
    await expectTrue(await page.locator("[role=dialog][aria-label=Settings]").isVisible(), "settings still open");
    await page.keyboard.press("Escape");
    await page.waitForTimeout(30);
    await expectTrue(!page.url().includes("#settings"), "second Esc closes");
  });

  await step("T24 theme segmented control: live apply + persist + system follow (UI-INT-105)", async () => {
    await page.goto(`${BASE}/new`);
    await page.evaluate(() => localStorage.setItem("theme", "system"));
    await page.reload();
    await page.waitForSelector("#composer"); // deterministic before keys/emulation
    await page.emulateMedia({ colorScheme: "light" });
    await page.keyboard.press("Control+,");
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await page.getByRole("radio", { name: "Dark" }).click();
    await expectTrue((await page.getAttribute("html", "data-mode")) === "dark", "Dark applies live");
    await expectTrue((await page.evaluate(() => localStorage.getItem("theme"))) === "dark", "persisted to `theme`");
    await page.getByRole("radio", { name: "System" }).click();
    await expectTrue((await page.getAttribute("html", "data-mode")) === "light", "System follows OS light");
    await page.emulateMedia({ colorScheme: "dark" });
    await page.waitForTimeout(120); // matchMedia listeners fire asynchronously
    await expectTrue((await page.getAttribute("html", "data-mode")) === "dark", "System follows OS dark LIVE");
    await page.getByRole("radio", { name: "Light" }).click();
    await expectTrue((await page.getAttribute("html", "data-mode")) === "light", "Light forces light over dark OS");
    await page.keyboard.press("Escape");
  });

  await step("T25 hash deep-load opens settings at section; history-back restores", async () => {
    await page.goto(`${BASE}/#settings/usage`);
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await expectTrue(
      (await page.locator(".settings-link.active").innerText()) === "Usage",
      "deep hash lands on Usage",
    );
    await page.goBack();
    await page.waitForTimeout(50);
    await expectTrue(await page.locator("[role=dialog][aria-label=Settings]").count() === 0, "back closes settings");
    await expectTrue(new URL(page.url()).pathname === "/new", "underlying route preserved");
  });

  await step("T26 garbage persisted values tolerated (resilience)", async () => {
    await page.evaluate(() => {
      localStorage.setItem("sidebar.collapsed", "maybe");
      localStorage.setItem("theme", "neon");
    });
    await page.reload();
    await expectTrue(await page.getByRole("navigation", { name: "Primary" }).isVisible(), "sidebar defaults visible");
    await expectTrue(
      ["light", "dark"].includes(await page.getAttribute("html", "data-mode") ?? ""),
      "invalid theme falls back to resolved system",
    );
    await page.evaluate(() => localStorage.setItem("sidebar.collapsed", "0"));
  });

  await step("T27 composer: Shift+Enter newline, Enter inert, Ctrl+N clears+focuses (UI-INT-005/040/041)", async () => {
    await page.goto(`${BASE}/new`);
    const ta = page.locator("#composer");
    await ta.fill("hello");
    await page.keyboard.press("Shift+Enter");
    await page.keyboard.type("world");
    await expectTrue((await ta.inputValue()).includes("\n"), "Shift+Enter inserts a newline");
    await page.keyboard.press("Enter");
    await page.waitForTimeout(30);
    await expectTrue(page.url().endsWith("/new"), "Enter stays inert until the sessions phase");
    await page.keyboard.press("Control+n");
    await page.waitForTimeout(30);
    await expectTrue((await ta.inputValue()) === "", "Ctrl+N clears the draft");
    await expectTrue(
      (await page.evaluate(() => document.activeElement?.id)) === "composer",
      "Ctrl+N focuses the composer",
    );
  });

  await step("T28 task route announces; code greeting distinct; unknown routes redirect", async () => {
    await page.goto(`${BASE}/cowork/local_abc123`);
    await expectTrue(await page.getByRole("status").count() === 1, "loading shell announces via role=status");
    await page.goto(`${BASE}/epitaxy`);
    await expectTrue(await page.getByRole("heading", { name: "What’s up next?" }).count() === 1, "code greeting");
    await expectTrue(await page.getByRole("heading", { name: "You’re here!" }).count() === 0, "cowork greeting must NOT appear in code mode");
    await page.goto(`${BASE}/definitely-not-a-route`);
    await page.waitForURL("**/new");
  });

  await step("T29 rapid Ctrl+B leaves persisted state consistent", async () => {
    await page.goto(`${BASE}/new`);
    for (let i = 0; i < 3; i += 1) await page.keyboard.press("Control+b");
    await page.waitForTimeout(30);
    const visible = await page.getByRole("navigation", { name: "Primary" }).isVisible();
    const stored = await page.evaluate(() => localStorage.getItem("sidebar.collapsed"));
    await expectTrue(stored === (visible ? "0" : "1"), `state mismatch: visible=${visible} stored=${stored}`);
  });

  await step("T30 focus ring uses box-shadow, not outline (docs/07 §3.1)", async () => {
    await page.goto(`${BASE}/new`);
    // T29 may have left the sidebar collapsed — persist open, then reload.
    await page.evaluate(() => localStorage.setItem("sidebar.collapsed", "0"));
    await page.reload();
    await page.waitForSelector("a.nav-item");
    await page.locator("a.nav-item", { hasText: "Projects" }).focus();
    const styles = await page.evaluate(() => {
      const cs = getComputedStyle(document.activeElement);
      return { outline: cs.outlineStyle, shadow: cs.boxShadow };
    });
    await expectTrue(styles.shadow !== "none", "ring shadow must be applied");
  });

  await step("T31 back/forward grey at history ends; Forward navigates (UI-INT-003, report A1)", async () => {
    await page.goto(BASE);
    await page.waitForSelector("a.nav-item");
    const backBtn = page.locator("button[aria-label=Back]");
    const fwdBtn = page.locator("button[aria-label=Forward]");
    await expectTrue(await backBtn.isDisabled(), "Back disabled at the first entry");
    await page.locator("a.nav-item", { hasText: "Projects" }).click();
    await page.waitForURL("**/projects");
    await page.locator("a.nav-item", { hasText: "Scheduled" }).click();
    await page.waitForURL("**/scheduled-task");
    await expectTrue(!(await backBtn.isDisabled()), "Back enabled after pushes");
    await expectTrue(await fwdBtn.isDisabled(), "Forward disabled at the forward end");
    await backBtn.click();
    await page.waitForURL("**/projects");
    await expectTrue(!(await fwdBtn.isDisabled()), "Forward enabled after Back");
    await fwdBtn.click();
    await page.waitForURL("**/scheduled-task");
    await backBtn.click();
    await backBtn.click();
    await page.waitForURL("**/new");
    await expectTrue(await backBtn.isDisabled(), "Back re-greys at the first entry");
    await expectTrue(!(await fwdBtn.isDisabled()), "Forward enabled again at the first entry");
    await fwdBtn.click();
    await page.waitForURL("**/projects");
  });

  await step("T32 closing settings leaves no zombie history entry (report A2)", async () => {
    await page.locator("a.nav-item", { hasText: "New" }).click();
    await page.waitForURL("**/new");
    for (let i = 0; i < 2; i += 1) {
      await page.keyboard.press("Control+,");
      await page.waitForSelector("[role=dialog][aria-label=Settings]");
      await page.keyboard.press("Escape");
      await page.waitForTimeout(60);
    }
    await page.goBack();
    await page.waitForTimeout(100);
    await expectTrue(
      (await page.locator("[role=dialog][aria-label=Settings]").count()) === 0,
      "Back after close must not re-open settings",
    );
    await expectTrue(!page.url().includes("#settings"), `no settings hash after Back (was ${page.url()})`);
  });
} finally {
  await browser.close();
}

console.log(results.join("\n"));
console.log(failures === 0 ? "E2E SUMMARY: ALL PASS" : `E2E SUMMARY: ${failures} FAILURE(S)`);
process.exitCode = failures === 0 ? 0 : 1;
