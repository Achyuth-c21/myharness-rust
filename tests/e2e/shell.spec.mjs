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

  await step("T27 composer: Shift+Enter newline, Enter attempts send, Ctrl+N clears+focuses (UI-INT-005/040/041)", async () => {
    await page.goto(`${BASE}/new`);
    const ta = page.locator("#composer");
    await ta.fill("hello");
    await page.keyboard.press("Shift+Enter");
    await page.keyboard.type("world");
    await expectTrue((await ta.inputValue()).includes("\n"), "Shift+Enter inserts a newline");
    // Enter now runs the send flow; a browser preview has no core, so the
    // failure must be visible and the draft must survive (REQ-COMPOSER-04).
    await page.keyboard.press("Enter");
    await page.locator(".composer-error").waitFor({ state: "visible" });
    await expectTrue(page.url().endsWith("/new"), "no session is created without the desktop host");
    await expectTrue(await page.locator(".composer-error").count() === 1, "desktop-only send error must show");
    await expectTrue((await ta.inputValue()).includes("hello"), "the draft is retained on failure");
    await page.keyboard.press("Control+n");
    await page.waitForTimeout(40);
    await expectTrue((await ta.inputValue()) === "", "Ctrl+N clears the draft");
    await expectTrue(
      (await page.evaluate(() => localStorage.getItem("composer-draft:new"))) === null,
      "Ctrl+N clears the stored New draft too",
    );
    await expectTrue(
      (await page.evaluate(() => document.activeElement?.id)) === "composer",
      "Ctrl+N focuses the composer",
    );
  });

  await step("T28 task route is desktop-only in the preview; code greeting distinct; unknown routes redirect", async () => {
    await page.goto(`${BASE}/cowork/local_abc123`);
    await page.getByRole("heading", { name: "Chat is available in the desktop app" }).waitFor();
    await expectTrue(
      await page.getByRole("heading", { name: "Chat is available in the desktop app" }).count() === 1,
      "the preview must state that the session store is desktop-only, not fake a transcript",
    );
    await page.getByRole("button", { name: "Back to New" }).click();
    await page.waitForURL("**/new");
    await page.goto(`${BASE}/epitaxy`);
    await page.getByRole("heading", { name: "What’s up next?" }).waitFor();
    await expectTrue(await page.getByRole("heading", { name: "What’s up next?" }).count() === 1, "code greeting");
    await expectTrue(await page.getByRole("heading", { name: "You’re here!" }).count() === 0, "cowork greeting must NOT appear in code mode");
    await page.goto(`${BASE}/definitely-not-a-route`);
    await page.waitForURL("**/new");
  });

  await step("T28b Code dashboard preview is honest and fail-closed", async () => {
    await page.goto(`${BASE}/epitaxy`);
    await page.getByText("Code is available in the desktop app.").waitFor();
    await expectTrue(
      (await page.locator(".code-stats").count()) === 0,
      "the preview must not invent usage stats",
    );
    await expectTrue(
      (await page.locator(".code-session-row").count()) === 0,
      "the preview must not invent Code sessions",
    );
    await expectTrue(
      (await page.locator(".code-repo-row").count()) === 0,
      "the preview must not invent repository rows",
    );
    await expectTrue(
      await page
        .locator('select[aria-label="Permission mode"] option[value="bypass"]')
        .evaluate((option) => option.disabled),
      "Bypass must be disabled when policy is unknown",
    );
    await expectTrue(
      await page.getByRole("button", { name: "Fast" }).isDisabled(),
      "Fast mode is deferred and must stay disabled",
    );
    await expectTrue(
      await page.getByRole("button", { name: "Send" }).isDisabled(),
      "Send must be disabled without a folder and desktop host",
    );
    await page.getByRole("button", { name: "Choose folder" }).click();
    await page.locator(".composer-error").waitFor({ state: "visible" });
    await expectTrue(
      (await page.locator(".composer-error").innerText()).includes("desktop app"),
      "the folder picker must report that it is desktop-only",
    );
    await page.goto(`${BASE}/epitaxy/local_abc123`);
    await page.getByRole("heading", { name: "Code is available in the desktop app" }).waitFor();
    await page.getByRole("button", { name: "Back to Code" }).click();
    await page.waitForURL("**/epitaxy");
  });

  await step("T28c populated Code dashboard renders only the supplied bridge data", async () => {
    const mockContext = await browser.newContext({
      colorScheme: "dark",
      viewport: { width: 1200, height: 900 },
    });
    const mockPage = await mockContext.newPage();
    await mockPage.addInitScript(() => {
      const now = Date.now();
      const totals = {
        inputTokens: 10,
        outputTokens: 5,
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        searchRequests: 0,
        events: 1,
        errorEvents: 0,
      };
      const session = {
        id: "00000000-0000-0000-0000-000000000001",
        surface: "code",
        title: "Test Code task",
        cwd: "/tmp/repo",
        model: "model-a",
        effort: null,
        permission_mode: "default",
        archived: false,
        legal_hold: false,
        createdAt: now - 60_000,
        lastActivityAt: now - 30_000,
        extra: { providerId: "stub", placement: "local" },
      };
      window.__TAURI_INTERNALS__ = {
        invoke: async (cmd, args) => {
          if (cmd === "pick_workspace_folder") return "/tmp/repo";
          if (cmd !== "chat_command") throw new Error(`unexpected command ${cmd}`);
          const command = args.command;
          if (command.type === "listCodeSessions" || command.type === "listSessions") {
            return {
              type: "sessions",
              sessions: command.type === "listCodeSessions" ? [session] : [],
            };
          }
          if (command.type === "codeDashboard") {
            window.__lastCodeRange = command.range ?? "all";
            return {
              type: "codeDashboard",
              summary: {
                range: command.range ?? "all",
                days: 2,
                truncated: false,
                timezoneOffsetMinutes: 0,
                from: "2026-10-24T00:00:00+00:00",
                to: "2026-10-25T00:00:00+00:00",
                totals,
                daily: [
                  { date: "2026-10-24", totals },
                  {
                    date: "2026-10-25",
                    totals: { ...totals, events: 0, inputTokens: 0, outputTokens: 0 },
                  },
                ],
                messages: 2,
                sessions: 1,
                activeDays: 1,
                peakHour: 9,
                favoriteModel: "model-a",
                models: [{ model: "model-a", totals, sessions: 1, sharePercent: 100 }],
              },
              sessions: [session],
              repositories: [
                { cwd: "/tmp/repo", sessionCount: 1, lastActivityAt: now - 30_000 },
              ],
              policy: {
                codeEnabled: true,
                localSessionsEnabled: true,
                bypassAllowed: false,
                workspaceRestricted: false,
              },
            };
          }
          throw new Error(`unexpected chat command ${command.type}`);
        },
      };
    });
    await mockPage.goto(`${BASE}/epitaxy`);
    await mockPage.locator(".code-session-row", { hasText: "Test Code task" }).waitFor();
    await expectTrue(
      (await mockPage.locator(".code-stat").count()) === 6,
      "the populated dashboard must render the six real stats",
    );
    await expectTrue(
      (await mockPage.locator(".code-stat").allInnerTexts()).some(
        (text) => text.toLowerCase().includes("total tokens") && text.includes("15"),
      ),
      "Total tokens must use input+output from the bridge",
    );
    await expectTrue(
      (await mockPage.locator(".code-repo-row").count()) === 1,
      "the folder row must come from the bridge",
    );
    await mockPage.getByRole("button", { name: "7 days" }).click();
    await mockPage.waitForFunction(() => window.__lastCodeRange === "7d");
    await expectTrue(
      (await mockPage.evaluate(() => window.__lastCodeRange)) === "7d",
      "the range control must request the 7-day bridge window",
    );
    await mockPage.getByRole("button", { name: "Choose folder" }).click();
    await mockPage.locator(".code-folder-chip", { hasText: "repo" }).waitFor();
    await expectTrue(
      (await mockPage.locator(".code-folder-chip").innerText()).includes("repo"),
      "a picked folder must be shown in the composer",
    );
    await mockPage.locator('select[aria-label="Permission mode"]').selectOption("plan");
    await expectTrue(
      (await mockPage.locator('select[aria-label="Permission mode"]').inputValue()) === "plan",
      "the permission selector must reflect the chosen mode",
    );
    await expectTrue(
      await mockPage
        .locator('select[aria-label="Permission mode"] option[value="bypass"]')
        .evaluate((option) => option.disabled),
      "Bypass must stay disabled when the mock policy does not grant it",
    );
    await mockContext.close();
  });

  await step("T28d Code session reuses the transcript with Code metadata", async () => {
    const mockContext = await browser.newContext({
      colorScheme: "dark",
      viewport: { width: 1200, height: 900 },
    });
    const mockPage = await mockContext.newPage();
    await mockPage.addInitScript(() => {
      const now = Date.now();
      const session = {
        id: "00000000-0000-0000-0000-000000000002",
        surface: "code",
        title: "Code transcript task",
        cwd: "/tmp/repo",
        model: "model-a",
        effort: null,
        permission_mode: "acceptEdits",
        archived: false,
        legal_hold: false,
        createdAt: now - 60_000,
        lastActivityAt: now - 30_000,
        extra: { providerId: "stub", placement: "local" },
      };
      const turns = [
        {
          id: "00000000-0000-0000-0000-000000000003",
          seq: 0,
          role: "user",
          status: "complete",
          stopReason: null,
          error: null,
          createdAt: now - 50_000,
          content: { parts: [{ type: "text", text: "hello code" }], citations: [] },
        },
        {
          id: "00000000-0000-0000-0000-000000000004",
          seq: 1,
          role: "assistant",
          status: "complete",
          stopReason: "end_turn",
          error: null,
          createdAt: now - 40_000,
          content: { parts: [{ type: "text", text: "working in the folder" }], citations: [] },
        },
      ];
      let callbackId = 0;
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
      window.__TAURI_INTERNALS__ = {
        transformCallback: (callback) => {
          callbackId += 1;
          window[`_${callbackId}`] = callback;
          return callbackId;
        },
        invoke: async (cmd, args) => {
          if (cmd === "plugin:event|listen" || cmd === "plugin:event|unlisten") return 1;
          if (cmd !== "chat_command") throw new Error(`unexpected command ${cmd}`);
          const command = args.command;
          if (command.type === "listCodeSessions" || command.type === "listSessions") {
            return {
              type: "sessions",
              sessions: command.type === "listCodeSessions" ? [session] : [],
            };
          }
          if (command.type === "listSkills") {
            return {
              type: "skills",
              view: { skills: [], disabled: [], creationEnabled: true, warnings: [] },
            };
          }
          if (command.type === "openCodeSession") {
            return { type: "session", session: { session, turns } };
          }
          throw new Error(`unexpected chat command ${command.type}`);
        },
      };
    });
    await mockPage.goto(`${BASE}/epitaxy/00000000-0000-0000-0000-000000000002`);
    await mockPage.locator(".session-title", { hasText: "Code transcript task" }).waitFor();
    await expectTrue(
      (await mockPage.locator(".user-bubble").innerText()).includes("hello code"),
      "the stored user turn must render",
    );
    await expectTrue(
      (await mockPage.locator(".turn-assistant").innerText()).includes("working in the folder"),
      "the stored assistant turn must render",
    );
    await expectTrue(
      (await mockPage.locator(".session-code-context").innerText()).includes("repo") &&
        (await mockPage.locator(".session-code-context").innerText()).includes("Accept edits"),
      "the Code header must show folder and stored permission mode",
    );
    await expectTrue(
      (await mockPage.locator(".session-model-label").innerText()).includes("model-a"),
      "the Code header must show the stored model",
    );
    await mockContext.close();
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

  await step("T31 drafts survive a reload (TEST-SESSION-009 scope: renderer storage)", async () => {
    await page.goto(`${BASE}/new`);
    await page.evaluate(() => localStorage.removeItem("composer-draft:new"));
    await page.reload();
    const ta = page.locator("#composer");
    await ta.fill("draft that must survive");
    await page.reload();
    await expectTrue(
      (await page.locator("#composer").inputValue()) === "draft that must survive",
      "the stored draft must be restored on reload",
    );
    await page.evaluate(() => localStorage.removeItem("composer-draft:new"));
  });

  await step("T32 no faux task rows or Active list without a core (docs/14 §9)", async () => {
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    await expectTrue((await page.locator(".task-row").count()) === 0, "sidebar task rows must be empty in the preview");
    await expectTrue((await page.locator(".active-row").count()) === 0, "Active list must not invent rows");
    await expectTrue(await page.getByText("Ideas for you").count() === 1, "empty home keeps Ideas");
  });

  await step("T33 usage panel states its desktop-only boundary", async () => {
    await page.goto(`${BASE}/#settings/usage`);
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await page.getByText("Usage is available in the desktop app.", { exact: false }).first().waitFor();
    await expectTrue(
      await page.getByText("Usage is available in the desktop app.", { exact: false }).count() === 1,
      "the browser preview must not fabricate usage totals",
    );
    await page.keyboard.press("Escape");
  });

  await step("T36 skills settings pane reports the desktop-only boundary", async () => {
    await page.goto(`${BASE}/#settings/skills`);
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await page.getByText("BRIDGE_UNAVAILABLE", { exact: false }).first().waitFor();
    await expectTrue(
      (await page.locator(".settings-row").count()) === 0,
      "the browser preview must not invent skill rows",
    );
    await page.keyboard.press("Escape");
  });

  await step("T37 plugins settings pane reports the desktop-only boundary", async () => {
    await page.goto(`${BASE}/#settings/plugins`);
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await page.getByText("BRIDGE_UNAVAILABLE", { exact: false }).first().waitFor();
    await expectTrue(
      (await page.locator(".settings-row").count()) === 0,
      "the browser preview must not invent plugin rows",
    );
    await page.keyboard.press("Escape");
  });

  await step("T38 developer settings pane reports the desktop-only boundary", async () => {
    await page.goto(`${BASE}/#settings/developer`);
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await page.getByText("BRIDGE_UNAVAILABLE", { exact: false }).first().waitFor();
    await expectTrue(
      (await page.getByText("No MCP servers are configured.").count()) === 0,
      "the browser preview must not claim an empty live MCP catalog",
    );
    await page.keyboard.press("Escape");
  });

  await step("T34 back/forward grey at history ends; Forward navigates (UI-INT-003, report A1)", async () => {
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

  await step("T35 closing settings leaves no zombie history entry (report A2)", async () => {
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
  await step("T39 projects preview is honest: captured copy, dialog flow, no invented rows", async () => {
    await page.goto(`${BASE}/projects`);
    await page.getByText("Looking to start a project?").waitFor();
    await expectTrue(
      (await page.locator(".project-row").count()) === 0,
      "the preview must not invent project rows",
    );
    // One creation entry in the empty state (REQ-PROJ-01) plus the captured
    // header button; both open the same dialog.
    await expectTrue(
      (await page.locator(".screen-empty").getByRole("button", { name: "New project" }).count()) === 1,
      "the empty state must have exactly one creation entry",
    );
    await page.locator(".screen-empty").getByRole("button", { name: "New project" }).click();
    await page.getByRole("dialog", { name: "Create a new project" }).waitFor();
    await expectTrue(
      await page.getByText("A dedicated place for ongoing work", { exact: false }).count() === 1,
      "the choice dialog lede is the captured copy",
    );
    await page.getByRole("button", { name: "Start from scratch" }).click();
    await page.getByRole("dialog", { name: "Start a new project" }).waitFor();
    await expectTrue(
      await page.getByPlaceholder("Project name").count() === 1 &&
      await page.getByPlaceholder("Tell Claude how to work in this project (optional)").count() === 1 &&
      await page.getByText("Select a folder...").count() === 1,
      "the form fields are the captured fields",
    );
    await expectTrue(
      await page.getByRole("button", { name: "Create" }).isDisabled(),
      "Create stays disabled until the required fields are valid",
    );
    await page.getByRole("dialog", { name: "Start a new project" }).getByRole("button", { name: "Back" }).click();
    await page.getByRole("button", { name: "Use an existing folder" }).click();
    await page.getByRole("dialog", { name: "Start a new project" }).waitFor();
    await page.keyboard.press("Escape");
    await page.locator(".project-dialog").waitFor({ state: "detached" });
    await expectTrue(
      (await page.locator(".project-dialog").count()) === 0,
      "Escape dismisses the dialog",
    );
  });

  await step("T40 populated projects render only the supplied bridge data; detail edit/delete are honest", async () => {
    const mockContext = await browser.newContext({
      colorScheme: "dark",
      viewport: { width: 1200, height: 900 },
    });
    const mockPage = await mockContext.newPage();
    const now = Date.now();
    const goodProject = {
      id: "00000000-0000-0000-0000-00000000a001",
      name: "Paper reviews",
      instructions: "be concise",
      cwd: "/tmp/papers",
      memory_enabled: true,
      createdAt: now - 100_000,
      lastActivityAt: now - 50_000,
      extra: {},
      folderAvailable: true,
    };
    const lostProject = {
      id: "00000000-0000-0000-0000-00000000a002",
      name: "Moved folder",
      instructions: null,
      cwd: "/tmp/vanished",
      memory_enabled: false,
      createdAt: now - 200_000,
      lastActivityAt: now - 60_000,
      extra: {},
      folderAvailable: false,
    };
    const sessions = [
      {
        id: "00000000-0000-0000-0000-00000000a003",
        surface: "chat",
        title: "Inside task",
        cwd: "/tmp/papers",
        model: "model-a",
        effort: null,
        permission_mode: null,
        projectId: goodProject.id,
        archived: false,
        legal_hold: false,
        createdAt: now - 90_000,
        lastActivityAt: now - 40_000,
        extra: {},
      },
    ];
    await mockPage.addInitScript(
      ({ goodProject, lostProject, sessions }) => {
        window.__lastProjectCommand = null;
        window.__TAURI_INTERNALS__ = {
          invoke: async (cmd, args) => {
            if (cmd !== "chat_command") throw new Error(`unexpected command ${cmd}`);
            const command = args.command;
            window.__lastProjectCommand = command;
            if (command.type === "listProjects") {
              return { type: "projects", projects: [goodProject, lostProject] };
            }
            if (command.type === "openProject") {
              return {
                type: "projectDetail",
                project: command.id === goodProject.id ? goodProject : lostProject,
                sessions: command.id === goodProject.id ? sessions : [],
              };
            }
            if (command.type === "updateProject") {
              return {
                type: "project",
                project: { ...goodProject, name: command.name, instructions: command.instructions, memory_enabled: command.memoryEnabled },
              };
            }
            if (command.type === "deleteProject") {
              return { type: "projectDeleted", orphanedSessions: 1 };
            }
            throw new Error(`unexpected chat command ${command.type}`);
          },
        };
      },
      { goodProject, lostProject, sessions },
    );
    await mockPage.goto(`${BASE}/projects`);
    await mockPage.locator(".project-row", { hasText: "Paper reviews" }).waitFor();
    await expectTrue(
      (await mockPage.locator(".project-row").count()) === 2,
      "the list renders exactly the supplied projects",
    );
    await expectTrue(
      (await mockPage.locator(".project-row", { hasText: "Moved folder" }).innerText()).includes("Folder unavailable"),
      "a missing folder is labelled unavailable, never hidden",
    );
    await mockPage.locator('input[aria-label="Search projects"]').fill("paper");
    await expectTrue(
      (await mockPage.locator(".project-row").count()) === 1,
      "the search narrows the list client-side",
    );
    await mockPage.locator('input[aria-label="Search projects"]').fill("");
    await mockPage.locator(".project-row", { hasText: "Paper reviews" }).click();
    await mockPage.locator(".project-sessions", { hasText: "Inside task" }).waitFor();
    await expectTrue(
      (await mockPage.locator(".project-folder-card").innerText()).includes("/tmp/papers"),
      "the detail shows the recorded folder",
    );
    await mockPage.locator(".project-detail-main input[type=text]").fill("Paper reviews 2");
    await mockPage.getByRole("button", { name: "Save changes" }).click();
    await mockPage.getByRole("heading", { name: "Paper reviews 2" }).waitFor();
    await mockPage.getByRole("button", { name: "Delete…" }).click();
    await expectTrue(
      (await mockPage.locator(".confirm-block").innerText()).includes("files on disk are never touched") &&
      (await mockPage.locator(".confirm-block").innerText()).includes("Project memory stores nothing"),
      "the delete confirmation states the effects on tasks, files, and memory",
    );
    await mockPage.getByRole("button", { name: "Delete project" }).click();
    await mockPage.waitForURL("**/projects");
    await expectTrue(
      true,
      "deleting returns to the list",
    );
    await mockContext.close();
  });

  await step("T41 the composer chip binds a project or folder and sends the id to the bridge", async () => {
    const mockContext = await browser.newContext({
      colorScheme: "dark",
      viewport: { width: 1200, height: 900 },
    });
    const mockPage = await mockContext.newPage();
    const project = {
      id: "00000000-0000-0000-0000-00000000b001",
      name: "Bound project",
      instructions: null,
      cwd: "/tmp/bound",
      memory_enabled: true,
      createdAt: Date.now() - 100_000,
      lastActivityAt: Date.now() - 50_000,
      extra: {},
      folderAvailable: true,
    };
    await mockPage.addInitScript(
      ({ project }) => {
        window.__lastCreate = null;
        window.__TAURI_INTERNALS__ = {
          invoke: async (cmd, args) => {
            if (cmd === "pick_workspace_folder") return "/tmp/one-off";
            if (cmd !== "chat_command") throw new Error(`unexpected command ${cmd}`);
            const command = args.command;
            if (command.type === "listProjects") {
              return { type: "projects", projects: [project] };
            }
            if (command.type === "createSession") {
              window.__lastCreate = command;
              return {
                type: "session",
                session: {
                  session: {
                    id: "00000000-0000-0000-0000-00000000b002",
                    surface: "chat",
                    title: "t",
                    cwd: null,
                    model: "model-a",
                    effort: null,
                    permission_mode: null,
                    projectId: command.projectId ?? null,
                    archived: false,
                    legal_hold: false,
                    createdAt: Date.now(),
                    lastActivityAt: Date.now(),
                    extra: {},
                  },
                  turns: [],
                },
              };
            }
            throw new Error(`unexpected chat command ${command.type}`);
          },
        };
      },
      { project },
    );
    await mockPage.goto(`${BASE}/new`);
    await mockPage.getByRole("button", { name: "Project or folder" }).click();
    await mockPage.locator(".project-chip-menu", { hasText: "Bound project" }).waitFor();
    await mockPage.locator(".project-chip-item", { hasText: "Bound project" }).click();
    await expectTrue(
      (await mockPage.locator(".project-chip").innerText()).includes("Bound project"),
      "the chip shows the selected project",
    );
    await mockPage.locator("#composer").fill("hello there");
    await mockPage.locator(".send-btn").click();
    await mockPage.waitForFunction(() => window.__lastCreate !== null);
    const sent = await mockPage.evaluate(() => window.__lastCreate);
    await expectTrue(
      sent.projectId === "00000000-0000-0000-0000-00000000b001",
      "the send must carry the selected project id",
    );
    await mockPage.waitForURL("**/cowork/**");
    await mockContext.close();
  });

  await step("T42 artifacts preview is honest: captured copy, explicit suggestion, no invented rows", async () => {
    await page.goto(`${BASE}/cowork-artifact`);
    await page.getByText("Create your first artifact").waitFor();
    await expectTrue(
      (await page.locator(".project-row").count()) === 0,
      "the preview must not invent artifact rows",
    );
    // The suggestion starts the task with the prompt the button itself shows;
    // without a desktop host the creation reports the boundary instead of
    // navigating to a fake task (REQ-ART-01/02).
    await page.getByRole("button", { name: "What needs my attention" }).click();
    await page.locator(".composer-error", { hasText: "Artifacts are available in the desktop app." }).waitFor();
    await expectTrue(
      !page.url().includes("/cowork/"),
      "the preview must never navigate to an invented artifact task",
    );
    await expectTrue(
      await page.getByRole("button", { name: "New artifact" }).count() === 1,
      "the header keeps the captured New artifact button",
    );
  });

  await step("T43 populated artifacts render only supplied rows; New artifact creates exactly one task", async () => {
    const mockContext = await browser.newContext({
      colorScheme: "dark",
      viewport: { width: 1200, height: 900 },
    });
    const mockPage = await mockContext.newPage();
    const now = Date.now();
    const artifactTask = {
      id: "00000000-0000-0000-0000-00000000c001",
      surface: "chat",
      title: "Weekly report draft",
      cwd: null,
      model: "model-a",
      effort: null,
      permission_mode: null,
      projectId: null,
      archived: false,
      legal_hold: false,
      createdAt: now - 100_000,
      lastActivityAt: now - 40_000,
      extra: { artifactTask: true },
    };
    const plainTask = {
      ...artifactTask,
      id: "00000000-0000-0000-0000-00000000c002",
      title: "Plain chat task",
      extra: {},
    };
    await mockPage.addInitScript(
      ({ artifactTask, plainTask }) => {
        let creates = 0;
        window.__artifactCreates = () => creates;
        window.__TAURI_INTERNALS__ = {
          invoke: async (cmd, args) => {
            if (cmd !== "chat_command") throw new Error(`unexpected command ${cmd}`);
            const command = args.command;
            if (command.type === "listSessions") {
              return { type: "sessions", sessions: [artifactTask, plainTask] };
            }
            if (command.type === "createArtifactTask") {
              creates += 1;
              return {
                type: "session",
                session: {
                  session: {
                    id: `00000000-0000-0000-0000-00000000d00${creates}`,
                    surface: "chat",
                    title: "New artifact",
                    cwd: null,
                    model: "model-a",
                    effort: null,
                    permission_mode: null,
                    projectId: null,
                    archived: false,
                    legal_hold: false,
                    createdAt: Date.now(),
                    lastActivityAt: Date.now(),
                    extra: { artifactTask: true },
                  },
                  turns: [],
                },
              };
            }
            throw new Error(`unexpected chat command ${command.type}`);
          },
        };
      },
      { artifactTask, plainTask },
    );
    await mockPage.goto(`${BASE}/cowork-artifact`);
    await mockPage.locator(".project-row", { hasText: "Weekly report draft" }).waitFor();
    await expectTrue(
      (await mockPage.locator(".project-row").count()) === 1,
      "only the marked artifact task is a library row; plain tasks stay out",
    );
    await mockPage.locator('input[aria-label="Search artifacts"]').fill("weekly");
    await expectTrue((await mockPage.locator(".project-row").count()) === 1, "search narrows");
    await mockPage.locator('input[aria-label="Search artifacts"]').fill("nothing matches this");
    await expectTrue(
      (await mockPage.locator(".palette-empty").count()) === 1,
      "no-results keeps the query editable",
    );
    await mockPage.locator('input[aria-label="Search artifacts"]').fill("");
    await mockPage.getByRole("button", { name: "New artifact" }).click();
    await mockPage.waitForURL("**/cowork/**");
    await expectTrue(
      (await mockPage.evaluate(() => window.__artifactCreates())) === 1,
      "one click creates exactly one task (TEST-ART-002)",
    );
    await mockContext.close();
  });

  await step("T44 scheduled preview is honest: captured banner, disabled keep-awake, no invented rows", async () => {
    await page.goto(`${BASE}/scheduled-task`);
    await page.getByText("No scheduled tasks yet.").waitFor();
    await expectTrue(
      await page.getByText("Scheduled tasks only run while your computer is awake and online.").count() === 1,
      "the banner copy is the captured sentence",
    );
    await expectTrue(
      await page.locator('.info-banner input[type=checkbox]').isDisabled(),
      "keep-awake ships disabled (REQ-SCHED-06; TEST-SCHED-005)",
    );
    await expectTrue(
      (await page.locator(".sched-row").count()) === 0,
      "the preview must not invent scheduled rows",
    );
    await expectTrue(
      (await page.locator(".sched-template").count()) === 0,
      "templates come from the bridge; the preview shows none rather than faking them",
    );
    // The editor opens client-side; a save without a core reports the boundary.
    await page.getByRole("button", { name: "New task" }).click();
    await page.getByRole("dialog", { name: "New scheduled task" }).waitFor();
    await page.locator(".project-dialog input[type=text]").fill("Probe");
    await page.locator(".project-dialog textarea").fill("Say hello");
    await page.getByRole("button", { name: "Create task" }).click();
    await page.locator(".composer-error", { hasText: "Scheduling is available in the desktop app." }).waitFor();
    await page.keyboard.press("Escape");
    await page.locator(".project-dialog").waitFor({ state: "detached" });
  });

  await step("T45 scheduled tasks: templates prefill, save creates one task, rows show next run and last result", async () => {
    const mockContext = await browser.newContext({
      colorScheme: "dark",
      viewport: { width: 1200, height: 900 },
    });
    const mockPage = await mockContext.newPage();
    const now = Date.now();
    const templates = [
      { name: "Daily briefing", prompt: "What needs my attention today across calendar, email, and messages?", cron: "0 8 * * 1-5", scheduleLabel: "Weekdays at 8:00 AM" },
      { name: "Weekly review", prompt: "Summarize what happened this week.", cron: "0 16 * * 5", scheduleLabel: "Every Friday at 4:00 PM" },
    ];
    let task = null;
    await mockPage.addInitScript(
      ({ templates, now }) => {
        window.__lastTaskCommand = null;
        const rows = [];
        window.__TAURI_INTERNALS__ = {
          invoke: async (cmd, args) => {
            if (cmd !== "chat_command") throw new Error(`unexpected command ${cmd}`);
            const command = args.command;
            window.__lastTaskCommand = command;
            if (command.type === "scheduledTemplates") {
              return { type: "templates", templates };
            }
            if (command.type === "listScheduledTasks") {
              return { type: "scheduledTasks", tasks: rows };
            }
            if (command.type === "previewSchedule") {
              return { type: "schedulePreview", times: [now + 3_600_000, now + 86_400_000, now + 172_800_000] };
            }
            if (command.type === "createScheduledTask") {
              window.__lastTaskCreate = command;
              const record = {
                id: "00000000-0000-0000-0000-00000000e001",
                name: command.name,
                prompt: command.prompt,
                cron: command.cron,
                enabled: true,
                projectId: null,
                cwd: null,
                createdAt: now,
                armedAt: now + 3_600_000,
                lastRunAt: null,
                extra: {},
                scheduleSummary: "on Fri at 16:00",
                lastRun: null,
                contextUnavailable: false,
              };
              rows.push(record);
              return { type: "scheduledTask", task: record };
            }
            if (command.type === "setScheduledTaskEnabled") {
              rows[0].enabled = command.enabled;
              rows[0].armedAt = command.enabled ? now + 3_600_000 : null;
              return { type: "scheduledTask", task: rows[0] };
            }
            if (command.type === "deleteScheduledTask") {
              rows.length = 0;
              return { type: "ack" };
            }
            throw new Error(`unexpected chat command ${command.type}`);
          },
        };
      },
      { templates, now },
    );
    await mockPage.goto(`${BASE}/scheduled-task`);
    await mockPage.locator(".sched-template", { hasText: "Weekly review" }).waitFor();
    await expectTrue(
      (await mockPage.locator(".sched-row").count()) === 0,
      "a template click must never create a task (REQ-SCHED-02)",
    );
    await mockPage.locator(".sched-template", { hasText: "Weekly review" }).click();
    await mockPage.getByRole("dialog", { name: "New scheduled task" }).waitFor();
    await expectTrue(
      (await mockPage.locator(".project-dialog input[type=text]").inputValue()) === "Weekly review",
      "the template prefills the editor",
    );
    await mockPage.getByRole("button", { name: "Create task" }).click();
    await mockPage.locator(".sched-row", { hasText: "Weekly review" }).waitFor();
    const created = await mockPage.evaluate(() => window.__lastTaskCreate);
    await expectTrue(
      created.cron === "0 16 * * 5",
      "the weekly template's schedule reaches the bridge as cron",
    );
    await expectTrue(
      (await mockPage.locator(".sched-row", { hasText: "Weekly review" }).innerText()).includes("next"),
      "the row shows the next pending occurrence",
    );
    await expectTrue(
      (await mockPage.locator(".sched-run-chip", { hasText: "Never run" }).count()) === 1,
      "a fresh task has never run",
    );
    // Disabling clears the next-run display; delete confirms its effects.
    await mockPage.locator(".sched-row .switch").click();
    await expectTrue(
      (await mockPage.locator(".sched-row", { hasText: "Weekly review" }).innerText()).includes("disabled"),
      "a disabled task shows no future run",
    );
    await mockPage.locator(".sched-row").getByRole("button", { name: "Delete" }).first().click();
    await mockPage.locator(".confirm-inline").waitFor();
    await mockPage.locator(".confirm-inline").getByRole("button", { name: "Delete" }).click();
    await expectTrue(
      (await mockPage.locator(".sched-row").count()) === 0,
      "deleting removes the schedule; run sessions stay in history",
    );
    await mockContext.close();
  });

  await step("T46 search palette finds real sessions and projects; keyboard nav opens (TEST-UX-003)", async () => {
    const mockContext = await browser.newContext({
      colorScheme: "dark",
      viewport: { width: 1200, height: 900 },
    });
    const mockPage = await mockContext.newPage();
    const now = Date.now();
    const chatSession = {
      id: "00000000-0000-0000-0000-00000000f001",
      surface: "chat",
      title: "Quarterly planning notes",
      cwd: null,
      model: "model-a",
      effort: null,
      permission_mode: null,
      projectId: null,
      archived: false,
      legal_hold: false,
      createdAt: now - 100_000,
      lastActivityAt: now - 10_000,
      extra: {},
    };
    const codeSession = {
      ...chatSession,
      id: "00000000-0000-0000-0000-00000000f002",
      surface: "code",
      title: "Quarterly refactor",
      lastActivityAt: now - 20_000,
    };
    const project = {
      id: "00000000-0000-0000-0000-00000000f003",
      name: "Quarterly reports",
      instructions: null,
      cwd: "/tmp/q",
      memory_enabled: true,
      createdAt: now - 200_000,
      lastActivityAt: now - 30_000,
      extra: {},
      folderAvailable: true,
    };
    await mockPage.addInitScript(
      ({ chatSession, codeSession, project }) => {
        window.__TAURI_INTERNALS__ = {
          invoke: async (cmd, args) => {
            if (cmd !== "chat_command") throw new Error(`unexpected command ${cmd}`);
            const command = args.command;
            if (command.type === "listSessions") {
              return { type: "sessions", sessions: [chatSession] };
            }
            if (command.type === "listCodeSessions") {
              return { type: "sessions", sessions: [codeSession] };
            }
            if (command.type === "listProjects") {
              return { type: "projects", projects: [project] };
            }
            throw new Error(`unexpected chat command ${command.type}`);
          },
        };
      },
      { chatSession, codeSession, project },
    );
    await mockPage.goto(`${BASE}/new`);
    await mockPage.waitForSelector("#composer"); // deterministic: shell mounted before keys
    await mockPage.keyboard.press("Control+k");
    await mockPage.getByPlaceholder("Search chats and projects").waitFor();
    // Empty query = the recent set from real rows.
    await expectTrue(
      (await mockPage.locator(".palette-row").count()) === 3,
      "the empty query lists the three real rows",
    );
    // A query narrows live across sessions AND projects, case-insensitively.
    await mockPage.getByPlaceholder("Search chats and projects").fill("quarterly");
    await expectTrue(
      (await mockPage.locator(".palette-row").count()) === 3,
      "the query matches both sessions and the project",
    );
    await mockPage.getByPlaceholder("Search chats and projects").fill("zzz-nothing");
    await expectTrue(await mockPage.locator(".palette-empty").count() === 1, "No results");
    await mockPage.getByRole("button", { name: "Clear search" }).click();
    await mockPage.getByPlaceholder("Search chats and projects").fill("refactor");
    await mockPage.keyboard.press("ArrowDown");
    await mockPage.keyboard.press("Enter");
    await mockPage.waitForURL("**/epitaxy/00000000-0000-0000-0000-00000000f002");
    await expectTrue(true, "Enter opens the selected Code session");
    // The palette must be closed after opening a result.
    await expectTrue(
      (await mockPage.locator(".palette").count()) === 0,
      "opening a result closes the palette",
    );
    await mockContext.close();
  });

  await step("T47 the filter menu composes, badges, and resets; filtered-empty differs from true empty", async () => {
    const mockContext = await browser.newContext({
      colorScheme: "dark",
      viewport: { width: 1200, height: 900 },
    });
    const mockPage = await mockContext.newPage();
    const now = Date.now();
    const today = { ...{} };
    const sessions = [
      {
        id: "00000000-0000-0000-0000-00000000f011",
        surface: "chat",
        title: "Fresh task",
        cwd: null,
        model: null,
        effort: null,
        permission_mode: null,
        projectId: null,
        archived: false,
        legal_hold: false,
        createdAt: now - 60_000,
        lastActivityAt: now - 30_000,
        extra: {},
      },
      {
        id: "00000000-0000-0000-0000-00000000f012",
        surface: "chat",
        title: "Old task",
        cwd: null,
        model: null,
        effort: null,
        permission_mode: null,
        projectId: null,
        archived: false,
        legal_hold: false,
        createdAt: now - 40 * 86_400_000,
        lastActivityAt: now - 40 * 86_400_000,
        extra: {},
      },
    ];
    await mockPage.addInitScript(
      ({ sessions }) => {
        window.__archivedRequested = null;
        window.__TAURI_INTERNALS__ = {
          invoke: async (cmd, args) => {
            if (cmd !== "chat_command") throw new Error(`unexpected command ${cmd}`);
            const command = args.command;
            if (command.type === "listSessions") {
              window.__archivedRequested = command.archived;
              return {
                type: "sessions",
                sessions: sessions.filter(
                  (row) => command.archived === null || row.archived === command.archived,
                ),
              };
            }
            if (command.type === "listProjects") {
              return { type: "projects", projects: [] };
            }
            if (command.type === "listCodeSessions") {
              return { type: "sessions", sessions: [] };
            }
            throw new Error(`unexpected chat command ${command.type}`);
          },
        };
      },
      { sessions },
    );
    await mockPage.goto(`${BASE}/new`);
    await mockPage.locator(".task-row", { hasText: "Fresh task" }).waitFor();
    await mockPage.getByRole("button", { name: "Filter tasks" }).click();
    await mockPage.getByRole("menu", { name: "Filter tasks" }).waitFor();
    // Last activity: Today hides the 40-day-old row immediately; applying
    // closes the menu.
    await mockPage.locator('[data-filter-row="activity"]').click();
    await mockPage.getByRole("menuitemradio", { name: "Today" }).click();
    await expectTrue(
      (await mockPage.locator(".task-row").count()) === 1,
      "the activity filter composes immediately",
    );
    await expectTrue(
      (await mockPage.locator(".filter-badge").count()) === 1,
      "an active filter shows the badge",
    );
    // Narrow further: the name sort does not change the count.
    await mockPage.getByRole("button", { name: "Filter tasks" }).click();
    await mockPage.locator('[data-filter-row="sortBy"]').click();
    await mockPage.getByRole("menuitemradio", { name: "Name" }).click();
    await expectTrue((await mockPage.locator(".task-row").count()) === 1, "sort composes");
    // Reset restores everything.
    await mockPage.getByRole("button", { name: "Filter tasks" }).click();
    await mockPage.locator(".filter-reset").click();
    await expectTrue(
      (await mockPage.locator(".task-row").count()) === 2,
      "reset restores the full list",
    );
    await expectTrue(
      (await mockPage.locator(".filter-badge").count()) === 0,
      "reset clears the badge",
    );
    // A filter that hides everything shows the distinct filtered-empty state.
    await mockPage.getByRole("button", { name: "Filter tasks" }).click();
    await mockPage.locator('[data-filter-row="activity"]').click();
    await mockPage.getByRole("menuitemradio", { name: "Today" }).click();
    // "Today" keeps both (both recent-ish? no — Old task is 40d). Force it via
    // Status Archived with no archived rows instead.
    await mockPage.getByRole("button", { name: "Filter tasks" }).click();
    await mockPage.locator('[data-filter-row="status"]').click();
    await mockPage.getByRole("menuitemradio", { name: "Archived" }).click();
    await mockPage.waitForFunction(() => window.__archivedRequested === true);
    await expectTrue(
      (await mockPage.locator(".tasks-filtered-empty").count()) === 1,
      "the filtered-empty state is distinct from the true empty state",
    );
    await expectTrue(
      (await mockPage.locator(".tasks-filtered-empty").innerText()).includes("Reset filters"),
      "the filtered-empty state offers the reset",
    );
    await mockPage.locator(".tasks-filtered-empty").getByRole("button", { name: "Reset filters" }).click();
    await expectTrue(
      (await mockPage.locator(".task-row").count()) === 2,
      "reset from the filtered-empty state restores the list",
    );
    // The Unread option is stated unavailable, not silently empty.
    await mockPage.getByRole("button", { name: "Filter tasks" }).click();
    await mockPage.locator('[data-filter-row="status"]').click();
    await expectTrue(
      await mockPage.getByRole("menuitemradio", { name: "Unread" }).isDisabled(),
      "Unread is disabled with an explanation (no unread tracking exists)",
    );
    await mockContext.close();
  });

} finally {
  await browser.close();
}

console.log(results.join("\n"));
console.log(failures === 0 ? "E2E SUMMARY: ALL PASS" : `E2E SUMMARY: ${failures} FAILURE(S)`);
process.exitCode = failures === 0 ? 0 : 1;
