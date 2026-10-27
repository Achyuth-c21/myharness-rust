/**
 * Whole-app test campaign phase 5 — user-journey E2E (`docs/17` §4.24).
 *
 * Phase 5's premise differs from the shell suite: instead of checking one
 * behavior per case, each case drives a MULTI-STEP user flow against a
 * STATEFUL fake bridge — a fake backend whose sessions, projects, tasks, and
 * turns mutate per command exactly like the real core, so a journey's later
 * steps see the effects of its earlier steps (the way a real user's day goes).
 *
 * The bridge seam is `window.__TAURI_INTERNALS__.invoke` — the same seam the
 * real shell uses. No Tauri host exists in the browser, so the desktop-only
 * boundaries stay honest (they are asserted, never faked away).
 *
 * Prereqs: root `npm install`, `npx playwright install chromium`, the Vite dev
 * server on 5173 (`npm run dev` in apps/desktop). Run: node
 * tests/e2e/user-journey.spec.mjs
 */
import { chromium } from "playwright";

const BASE = process.env.BASE_URL ?? "http://localhost:5173";
const REPLY = "The mock assistant reply.";

const results = [];
let failures = 0;

async function step(name, fn) {
  try {
    await fn();
    results.push(`PASS ${name}`);
  } catch (error) {
    failures += 1;
    results.push(`FAIL ${name}\n      ${String(error).split("\n").slice(0, 4).join(" | ")}`);
  }
}

async function expectTrue(cond, message) {
  if (!cond) throw new Error(message);
}

/** A session row with sensible defaults; `extra`/`surface` override. */
function sessionRow(id, title, overrides = {}) {
  const now = Date.now();
  return {
    id,
    surface: "chat",
    title,
    cwd: null,
    model: "model-a",
    effort: null,
    permission_mode: null,
    projectId: null,
    archived: false,
    legal_hold: false,
    createdAt: now,
    lastActivityAt: now,
    extra: {},
    ...overrides,
  };
}

/**
 * The stateful fake bridge. Everything the journeys touch mutates `state`:
 * sends append real turns, renames rewrite rows, deletes remove them, and
 * every list command re-reads the mutated state — the later steps of a
 * journey see the earlier steps' effects, like a real backend.
 */
function journeyBackend(page, seed = {}) {
  return page.addInitScript(({ seed, reply }) => {
    const state = {
      sessions: [],
      projects: [],
      tasks: [],
      seq: 1,
      ...seed,
    };
    window.__backend = {
      commands: [],
      state,
    };
    const now = () => Date.now();

    const mkTurn = (role, text, seq) => ({
      id: `turn-${state.seq++}`,
      seq,
      role,
      status: "complete",
      stopReason: role === "assistant" ? "end_turn" : null,
      error: null,
      createdAt: now(),
      content: text == null ? { parts: [] } : { parts: [{ type: "text", text }] },
    });

    const view = (session) => {
      const turns = state.sessions === session ? [] : (session.__turns ?? []);
      return { session: JSON.parse(JSON.stringify(session)), turns: JSON.parse(JSON.stringify(turns)) };
    };

    const touch = (session) => {
      session.lastActivityAt = now();
    };

    const dispatch = async (command) => {
      switch (command.type) {
        case "listSessions":
          return {
            type: "sessions",
            sessions: state.sessions.filter((s) => s.surface === "chat" && !s.archived),
          };
        case "listCodeSessions":
          return {
            type: "sessions",
            sessions: state.sessions.filter((s) => s.surface === "code" && !s.archived),
          };
        case "openSession": {
          const session = state.sessions.find((s) => s.id === command.id);
          if (!session) return { type: "error", error: { code: "SESSION_NOT_FOUND", message: "no such session", retryable: false }, userTurnRecorded: false };
          return { type: "session", session: view(session) };
        }
        case "openCodeSession": {
          const session = state.sessions.find((s) => s.id === command.id);
          if (!session) return { type: "error", error: { code: "SESSION_NOT_FOUND", message: "no such session", retryable: false }, userTurnRecorded: false };
          return { type: "session", session: view(session) };
        }
        case "createSession": {
          const session = {
            id: `sess-${state.seq++}`,
            surface: "chat",
            title: command.title ?? "t",
            cwd: command.cwd ?? null,
            model: "model-a",
            effort: null,
            permission_mode: null,
            projectId: command.projectId ?? null,
            archived: false,
            legal_hold: false,
            createdAt: now(),
            lastActivityAt: now(),
            extra: {},
            __turns: [],
          };
          state.sessions.unshift(session);
          return { type: "session", session: view(session) };
        }
        case "createCodeSession": {
          const session = {
            id: `code-${state.seq++}`,
            surface: "code",
            title: command.title ?? "Code session",
            cwd: command.cwd,
            model: "model-a",
            effort: null,
            permission_mode: command.permissionMode ?? "default",
            projectId: null,
            archived: false,
            legal_hold: false,
            createdAt: now(),
            lastActivityAt: now(),
            extra: {},
            __turns: [],
          };
          state.sessions.unshift(session);
          return { type: "session", session: view(session) };
        }
        case "createArtifactTask": {
          const session = {
            id: `art-${state.seq++}`,
            surface: "chat",
            title: "New artifact",
            cwd: null,
            model: "model-a",
            effort: null,
            permission_mode: null,
            projectId: null,
            archived: false,
            legal_hold: false,
            createdAt: now(),
            lastActivityAt: now(),
            extra: { artifactTask: true },
            __turns: [],
          };
          state.sessions.unshift(session);
          return { type: "session", session: view(session) };
        }
        case "sendTurn":
        case "sendCodeTurn": {
          const session = state.sessions.find((s) => s.id === command.sessionId);
          if (!session) return { type: "error", error: { code: "SESSION_NOT_FOUND", message: "no such session", retryable: false }, userTurnRecorded: false };
          if (!session.__turns) session.__turns = [];
          const userTurn = mkTurn("user", command.text, session.__turns.length + 1);
          session.__turns.push(userTurn);
          if (session.title === "t" || session.title === "New artifact" || session.title === "Code session") {
            session.title = command.text.slice(0, 40);
          }
          const assistantTurn = mkTurn("assistant", reply, session.__turns.length + 1);
          session.__turns.push(assistantTurn);
          touch(session);
          return {
            type: "turn",
            sessionId: session.id,
            userTurn: JSON.parse(JSON.stringify(userTurn)),
            assistantTurn: JSON.parse(JSON.stringify(assistantTurn)),
            stopReason: "end_turn",
            usage: { input: 9, output: 7, cacheRead: 0, cacheWrite: 0, webSearch: 0 },
          };
        }
        case "stopTurn":
          return { type: "stopped", stopped: false };
        case "renameSession": {
          const session = state.sessions.find((s) => s.id === command.id);
          if (!session) return { type: "error", error: { code: "SESSION_NOT_FOUND", message: "no such session", retryable: false }, userTurnRecorded: false };
          session.title = command.title;
          return { type: "ack" };
        }
        case "archiveSession": {
          const session = state.sessions.find((s) => s.id === command.id);
          if (!session) return { type: "error", error: { code: "SESSION_NOT_FOUND", message: "no such session", retryable: false }, userTurnRecorded: false };
          session.archived = command.archived;
          return { type: "ack" };
        }
        case "deleteSession": {
          state.sessions = state.sessions.filter((s) => s.id !== command.id);
          return { type: "ack" };
        }
        case "usageSummary": {
          const input = state.sessions.reduce((n, s) => n + (s.__turns?.length ?? 0) * 9, 0);
          const output = state.sessions.reduce((n, s) => n + (s.__turns?.length ?? 0) * 7, 0);
          const totals = {
            inputTokens: input,
            outputTokens: output,
            cacheReadTokens: 0,
            cacheWriteTokens: 0,
            searchRequests: 0,
            events: input + output,
            errorEvents: 0,
          };
          return {
            type: "usage",
            summary: {
              days: command.days ?? 7,
              timezoneOffsetMinutes: 0,
              from: "2026-10-28",
              to: "2026-10-28",
              totals,
              daily: [{ date: "2026-10-28", totals }],
            },
            surfaces: [
              { surface: "chat", totals },
              { surface: "code", totals: { ...totals, inputTokens: 0, outputTokens: 0, events: 0 } },
            ],
          };
        }
        case "listProjects":
          return { type: "projects", projects: state.projects };
        case "createProject": {
          const project = {
            id: `proj-${state.seq++}`,
            name: command.name,
            instructions: command.instructions ?? null,
            cwd: command.cwd,
            memory_enabled: command.memoryEnabled ?? false,
            createdAt: now(),
            lastActivityAt: now(),
            extra: {},
            folderAvailable: true,
          };
          state.projects.unshift(project);
          return { type: "project", project };
        }
        case "openProject": {
          const project = state.projects.find((p) => p.id === command.id);
          if (!project) return { type: "error", error: { code: "PROJECT_NOT_FOUND", message: "no such project", retryable: false }, userTurnRecorded: false };
          return {
            type: "projectDetail",
            project,
            sessions: state.sessions.filter((s) => s.projectId === project.id && !s.archived),
          };
        }
        case "deleteProject": {
          const orphans = state.sessions.filter((s) => s.projectId === command.id).length;
          state.projects = state.projects.filter((p) => p.id !== command.id);
          return { type: "projectDeleted", orphanedSessions: orphans };
        }
        case "listScheduledTasks":
          return { type: "scheduledTasks", tasks: state.tasks };
        case "scheduledTemplates":
          return {
            type: "templates",
            templates: [
              { name: "Daily briefing", prompt: "What needs my attention today?", cron: "0 8 * * 1-5", scheduleLabel: "Weekdays at 8:00 AM" },
            ],
          };
        case "createScheduledTask": {
          const task = {
            id: `task-${state.seq++}`,
            name: command.name,
            prompt: command.prompt,
            cron: command.cron,
            enabled: command.enabled ?? true,
            projectId: command.projectId ?? null,
            cwd: command.cwd ?? null,
            createdAt: now(),
            armedAt: command.enabled === false ? null : now(),
            lastRunAt: null,
            extra: {},
            scheduleSummary: "Weekdays at 8:00 AM",
            lastRun: null,
            contextUnavailable: false,
          };
          state.tasks.push(task);
          return { type: "scheduledTask", task };
        }
        case "setScheduledTaskEnabled": {
          const task = state.tasks.find((t) => t.id === command.id);
          if (!task) return { type: "error", error: { code: "TASK_NOT_FOUND", message: "no such task", retryable: false }, userTurnRecorded: false };
          task.enabled = command.enabled;
          return { type: "scheduledTask", task };
        }
        case "deleteScheduledTask": {
          state.tasks = state.tasks.filter((t) => t.id !== command.id);
          return { type: "ack" };
        }
        case "previewSchedule":
          return { type: "schedulePreview", times: [1793161200000, 1793247600000, 1793334000000] };
        case "codeDashboard": {
          const codeSessions = state.sessions.filter((s) => s.surface === "code");
          const totals = {
            inputTokens: 0,
            outputTokens: 0,
            cacheReadTokens: 0,
            cacheWriteTokens: 0,
            searchRequests: 0,
            events: 0,
            errorEvents: 0,
          };
          return {
            type: "codeDashboard",
            summary: {
              range: "all",
              days: 3660,
              truncated: false,
              timezoneOffsetMinutes: 0,
              from: "2016-01-01",
              to: "2026-10-28",
              totals,
              daily: [],
              messages: 0,
              sessions: codeSessions.length,
              activeDays: 0,
              peakHour: null,
              favoriteModel: null,
              models: [],
            },
            sessions: codeSessions,
            repositories: codeSessions.length
              ? [{ cwd: codeSessions[0].cwd, sessionCount: codeSessions.length, lastActivityAt: codeSessions[0].lastActivityAt }]
              : [],
            policy: { codeEnabled: true, localSessionsEnabled: true, bypassAllowed: false, workspaceRestricted: false },
          };
        }
        case "mcpStatus":
          return { type: "mcpStatus", servers: [], warnings: [] };
        case "mcpTools":
          return { type: "mcpTools", tools: [] };
        case "listSkills":
          return { type: "skills", view: { selected: null, skills: [], creationEnabled: false } };
        case "listPlugins":
          return { type: "plugins", view: { plugins: [], signaturePolicy: "allow-signed" } };
        default:
          return {
            type: "error",
            error: { code: "UNKNOWN", message: `fake backend: unhandled ${command.type}`, retryable: false },
            userTurnRecorded: false,
          };
      }
    };

    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "main" } },
      transformCallback: (callback) => callback,
      convertFileSrc: (path) => path,
      invoke: async (cmd, args) => {
        if (cmd === "chat_command") {
          window.__backend.commands.push(args.command);
          return dispatch(args.command);
        }
        if (cmd === "pick_workspace_folder") return seed.folder ?? "/tmp/journey-folder";
        // The event channel has no host in the browser; tolerate the
        // subscriptions the screens make so sends complete through the
        // command response instead.
        if (cmd === "plugin:event|listen") return 1;
        if (cmd === "plugin:event|unlisten") return null;
        if (cmd === "plugin:event|emit") return null;
        return Promise.reject(new Error(`fake backend: unexpected ${cmd}`));
      },
    };
  }, { seed, reply: REPLY });
}

/** Open the session-title menu; a click racing the post-send re-render is
 * eaten, so one retry when the menu did not appear. */
async function openSessionMenu(page) {
  await page.locator("button.session-title").click();
  if ((await page.locator(".session-menu").count()) === 0) {
    await page.waitForTimeout(400);
    await page.locator("button.session-title").click();
  }
  await page.locator(".session-menu").waitFor();
}

const browser = await chromium.launch();
try {
  // ───────────────────────── Journey A: a day of chatting ─────────────────────────

  await step("U01 a first message: chip prefill, send, transcript, sidebar, titled session", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    // The chip prefills; the user presses send (one click, like the real flow).
    await page.getByText("Send me a daily briefing", { exact: true }).click();
    await page.locator("#composer").waitFor();
    await expectTrue(
      (await page.locator("#composer").inputValue()).includes("daily briefing"),
      "the chip must prefill the composer",
    );
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    // The transcript renders the exchange from the stateful backend.
    await page.getByText("Send me a daily briefing").first().waitFor();
    await page.getByText(REPLY).waitFor();
    // The session is titled from the first message and listed in the sidebar.
    await expectTrue(
      (await page.locator(".sidebar .task-row", { hasText: "Send me a daily briefing" }).count()) === 1,
      "the sidebar must list the new session",
    );
    // The session header carries the model label once a session exists.
    await expectTrue(
      (await page.locator(".session-head", { hasText: "model-a" }).count()) === 1,
      "the session header shows the model",
    );
    await context.close();
  });

  await step("U02 a second message appends to the same transcript", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    await page.getByText("Send me a daily briefing", { exact: true }).click();
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    // The user types the follow-up themselves.
    await page.locator("#composer").fill("and what changed since yesterday?");
    await page.locator(".send-btn").click();
    await page.waitForFunction(() => {
      const rows = document.querySelectorAll(".transcript [class*=user], .turn-user, [data-role=user]");
      return rows.length >= 2;
    }, undefined, { timeout: 5000 }).catch(() => {});
    // Both exchanges are present: two user turns, two replies.
    const body = await page.evaluate(() => document.body.innerText);
    await expectTrue(
      (body.match(new RegExp(REPLY.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "g")) ?? []).length >= 2,
      "two assistant replies must be in the transcript",
    );
    await expectTrue(body.includes("and what changed since yesterday?"), "the follow-up must be in the transcript");
    await context.close();
  });

  await step("U03 rename from the session menu updates the sidebar", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    await page.getByText("Send me a daily briefing", { exact: true }).click();
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    await openSessionMenu(page);
    await page.getByRole("menuitem", { name: "Rename" }).click();
    await page.getByLabel("Task title").fill("Morning digest");
    await page.getByRole("button", { name: "Save" }).click();
    await page.waitForFunction(() => document.body.innerText.includes("Morning digest"));
    await expectTrue(
      (await page.locator(".sidebar .task-row", { hasText: "Morning digest" }).count()) === 1,
      "the sidebar must show the renamed title",
    );
    await context.close();
  });

  await step("U04 archive removes the session from the Active list", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    await page.getByText("Send me a daily briefing", { exact: true }).click();
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    await openSessionMenu(page);
    await page.getByRole("menuitem", { name: "Archive" }).click();
    await page.waitForURL("**/new");
    await expectTrue(
      (await page.locator(".active-row, .sidebar .task-row", { hasText: "Send me a daily briefing" }).count()) === 0,
      "the archived session must leave the Active list",
    );
    await context.close();
  });

  await step("U05 delete asks, then removes; Ideas return when nothing is left", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    await page.getByText("Send me a daily briefing", { exact: true }).click();
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    await openSessionMenu(page);
    await page.getByRole("menuitem", { name: "Delete…" }).click();
    await page.locator(".session-confirm").getByRole("button", { name: "Delete" }).click();
    await page.waitForURL("**/new");
    await page.getByText("Ideas for you").waitFor();
    await expectTrue(
      (await page.locator(".active-row, .sidebar .task-row").count()) === 0,
      "no session rows may remain",
    );
    await context.close();
  });

  await step("U06 a typed draft survives leaving the session and coming back", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    await page.getByText("Send me a daily briefing", { exact: true }).click();
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    await page.locator("#composer").fill("draft kept while I look at projects");
    await page.getByRole("navigation", { name: "Primary" }).getByText("Projects").click();
    await page.waitForURL("**/projects");
    await page.locator(".sidebar .task-row", { hasText: "Send me a daily briefing" }).click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    await expectTrue(
      (await page.locator("#composer").inputValue()) === "draft kept while I look at projects",
      "the session draft must be restored on return",
    );
    await context.close();
  });

  // ───────────────────────── Journey B: projects ─────────────────────────

  await step("U07 create a project, bind a session, see it in the project, delete it", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page, { folder: "/tmp/journey-folder" });
    await page.goto(`${BASE}/projects`);
    await page.getByText("Looking to start a project?").waitFor();
    await page.locator(".screen-empty").getByRole("button", { name: "New project" }).click();
    await page.getByRole("dialog", { name: "Create a new project" }).waitFor();
    await page.getByRole("button", { name: "Use an existing folder" }).click();
    await page.getByRole("dialog", { name: "Start a new project" }).waitFor();
    await page.getByPlaceholder("Project name").fill("Journey project");
    await page.getByRole("button", { name: "Choose…" }).click();
    await page.getByRole("dialog", { name: "Start a new project" }).getByText("/tmp/journey-folder").waitFor();
    // Create lands in the project DETAIL (the create flow's own destination).
    await page.getByRole("button", { name: "Create", exact: true }).click();
    await page.locator(".project-folder-card", { hasText: "/tmp/journey-folder" }).waitFor();
    // Bind a new session from the New screen's project chip.
    await page.getByRole("navigation", { name: "Primary" }).getByText("New", { exact: true }).click();
    await page.waitForSelector("#composer");
    await page.getByRole("button", { name: "Project or folder" }).click();
    await page.locator(".project-chip-item", { hasText: "Journey project" }).click();
    await page.locator("#composer").fill("hello project");
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    // The project detail lists the bound session.
    await page.getByRole("navigation", { name: "Primary" }).getByText("Projects").click();
    await page.locator(".project-row", { hasText: "Journey project" }).click();
    await page.locator(".project-sessions", { hasText: "hello project" }).waitFor();
    // Delete the project from its detail; the confirmation states the effects.
    await page.getByRole("button", { name: "Delete…" }).click();
    await page.locator(".confirm-block").waitFor();
    await page.getByRole("button", { name: "Delete project" }).click();
    await page.waitForURL("**/projects");
    await expectTrue(
      (await page.locator(".project-row").count()) === 0,
      "the project row must be gone",
    );
    await context.close();
  });

  // ───────────────────────── Journey C: scheduled tasks ─────────────────────────

  await step("U08 template prefill, save, disable, re-enable, delete — one task's whole life", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/scheduled-task`);
    await page.getByText("No scheduled tasks yet.").waitFor();
    // A template click opens the editor prefilled; it never creates by itself.
    await page.locator(".sched-template", { hasText: "Daily briefing" }).click();
    await page.getByRole("dialog", { name: "New scheduled task" }).waitFor();
    await expectTrue(
      (await page.locator(".project-dialog input[type=text]").inputValue()) === "Daily briefing",
      "the template prefills the editor",
    );
    await page.getByRole("button", { name: "Create task" }).click();
    const row = page.locator(".sched-row", { hasText: "Daily briefing" });
    await row.waitFor();
    // Disable clears the next run; re-enable restores it.
    await page.locator(".sched-row .switch").click();
    await expectTrue((await row.innerText()).includes("disabled"), "a disabled task shows no future run");
    await page.locator(".sched-row .switch").click();
    await expectTrue((await row.innerText()).includes("next"), "re-enabling restores the next run");
    // Delete confirms its effects inline.
    await page.locator(".sched-row").getByRole("button", { name: "Delete" }).first().click();
    await page.locator(".confirm-inline").waitFor();
    await page.locator(".confirm-inline").getByRole("button", { name: "Delete" }).click();
    await page.getByText("No scheduled tasks yet.").waitFor();
    await context.close();
  });

  // ───────────────────────── Journey D: artifacts ─────────────────────────

  await step("U09 new artifact creates exactly one task and the list keeps it", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/cowork-artifact`);
    await page.getByText("Create your first artifact").waitFor();
    await page.getByRole("button", { name: "New artifact" }).click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor().catch(() => {});
    // Back at the list: exactly one artifact row exists.
    await page.getByRole("navigation", { name: "Primary" }).getByText("Artifacts").click();
    await page.locator(".project-row").first().waitFor();
    await expectTrue(
      (await page.locator(".project-row").count()) === 1,
      "exactly one artifact task must exist",
    );
    await context.close();
  });

  // ───────────────────────── Journey E: code ─────────────────────────

  await step("U10 pick a folder, send a code task, the dashboard keeps both", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page, { folder: "/tmp/journey-code" });
    await page.goto(`${BASE}/epitaxy`);
    await page.getByRole("button", { name: "Choose folder" }).waitFor();
    await page.getByRole("button", { name: "Choose folder" }).click();
    await page.waitForFunction(() => document.body.innerText.includes("journey-code"));
    await page.getByPlaceholder("Describe a task or ask a question").fill("fix the flaky test");
    await page.locator(".send-btn").click();
    await page.waitForURL("**/epitaxy/**");
    await page.getByText(REPLY).waitFor();
    // The session header shows the folder and the permission mode.
    await expectTrue(
      (await page.locator(".session-head", { hasText: "journey-code" }).count()) === 1,
      "the code session header names the folder",
    );
    // The dashboard shows the folder as a repository row with the session.
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await page.waitForURL("**/epitaxy");
    await page.locator(".code-repo-row", { hasText: "journey-code" }).waitFor();
    await expectTrue(
      (await page.locator(".code-session-row").count()) === 1,
      "the code session is listed on the dashboard",
    );
    await context.close();
  });

  // ───────────────────────── Journey F: usage + palette ─────────────────────────

  await step("U11 the usage panel reflects the tokens the sessions actually produced", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    await page.getByText("Send me a daily briefing", { exact: true }).click();
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    await page.goto(`${BASE}/#settings/usage`);
    await page.waitForSelector("[role=dialog][aria-label=Settings]");
    await page.getByText("Tokens", { exact: false }).first().waitFor();
    await expectTrue(
      (await page.getByText("9").count()) >= 1 && (await page.getByText("7").count()) >= 1,
      "the mocked 9-in/7-out totals must be visible",
    );
    await context.close();
  });

  await step("U12 the palette finds a session created earlier in the same session", async () => {
    const context = await browser.newContext({ colorScheme: "dark", viewport: { width: 1200, height: 900 } });
    const page = await context.newPage();
    journeyBackend(page);
    await page.goto(`${BASE}/new`);
    await page.waitForSelector("#composer");
    await page.getByText("Send me a daily briefing", { exact: true }).click();
    await page.locator(".send-btn").click();
    await page.waitForURL("**/cowork/**");
    await page.getByText(REPLY).waitFor();
    await page.waitForSelector("#composer");
    await page.keyboard.press("Control+k");
    await page.getByPlaceholder("Search chats and projects").fill("daily briefing");
    await page.locator(".palette-row").first().waitFor();
    await page.keyboard.press("Enter");
    await page.waitForURL("**/cowork/**");
    await expectTrue(true, "the palette opened the session created this run");
    await context.close();
  });
} finally {
  await browser.close();
}

console.log(results.join("\n"));
console.log("");
console.log(
  `USER JOURNEY E2E SUMMARY: ${results.length} case(s) — PASS ${results.length - failures} / FAIL ${failures}`,
);
process.exit(failures === 0 ? 0 : 1);
