/**
 * Setup-window E2E — Phase 2 (docs/13 §4; docs/09 §2.1–§2.10;
 * docs/screens/setup-desktop-3p.md).
 *
 * The setup window is a separate window (docs/09 §2.1) that reaches the
 * configuration library only through the `settings.Custom3pSetup` bridge
 * (docs/18 §3.4). This suite drives the RENDERER against a stubbed bridge, so
 * the window's own behavior is exercised without a live configuration library:
 * the draft model, the Apply/Discard footer, the tab navigation, the read-only
 * Egress tab, and the credential flow.
 *
 * The bridge seam is `window.__TAURI_INTERNALS__.invoke` — the same seam the
 * real shell uses (apps/desktop/src/setup/client.ts).
 *
 * Prereqs: `npm install` here (root), `npx playwright install chromium`,
 * renderer dev server: `npm run dev` in apps/desktop (port 5173).
 * Run: node tests/e2e/setup-window.spec.mjs
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

/** The profile the stub serves. */
const PROFILE = {
  schemaVersion: 1,
  id: "11111111-1111-4111-8111-111111111111",
  name: "Workspace",
  provider: "gateway",
  connection: {
    inferenceCredentialKind: "static",
    inferenceGatewayBaseUrl: "https://gateway.example",
    inferenceGatewayApiKey: { kind: "keychain", service: "myharness", account: "profile/x" },
    inferenceGatewayAuthScheme: "bearer",
  },
  capabilities: {},
  connectors: {},
  telemetry: {},
  limits: {},
  appearance: {},
  plugins: {},
  egress: {},
  source: {},
};

/** The Connection tab schema (mirrors crates/config::forms::connection_schema). */
const CONNECTION_SCHEMA = {
  provider: "gateway",
  fields: [
    { key: "inferenceCredentialKind", label: "Credential kind", kind: "select", group: "credentials", required: true, options: [{ value: "static", label: "Static API key" }, { value: "helper-script", label: "Helper script" }] },
    { key: "inferenceGatewayApiKey", label: "Gateway API key", kind: "secret", group: "credentials", required: true },
    { key: "inferenceGatewayBaseUrl", label: "Gateway base URL", kind: "url", group: "endpoint", required: true },
    { key: "inferenceGatewayAuthScheme", label: "Auth scheme", kind: "select", group: "credentials", required: true, options: [{ value: "bearer", label: "bearer" }, { value: "x-api-key", label: "x-api-key" }] },
    { key: "modelDiscoveryEnabled", label: "Model discovery", kind: "switch", group: "models" },
    { key: "inferenceModels", label: "Model list", kind: "tag-list", group: "models" },
  ],
};

/** A tab schema (mirrors crates/config::tabs::tab_schema). */
function tabSchema(tab) {
  if (tab === "connection") return CONNECTION_SCHEMA;
  return { tab, title: tab, fields: [], readOnly: tab === "egress" };
}

/**
 * The stubbed bridge. It records every command so the suite can assert on what
 * the window sent, and answers with the shapes the real bridge returns.
 */
function installStub(page, state) {
  return page.addInitScript((seed) => {
    const recorded = [];
    window.__mhCommands = recorded;

    const respond = (command) => {
      switch (command.type) {
        case "listProfiles":
        case "applyProfile":
          return { type: "profiles", profiles: [{ id: seed.id, name: seed.name, provider: seed.provider, applied: true }] };
        case "readProfile":
          return { type: "profile", document: seed.document };
        case "writeProfile":
          return { type: "profile", document: command.document };
        case "merged":
          return { type: "merged", merged: { profileId: seed.id, fields: {}, overlayPresent: false } };
        case "health":
          return { type: "health", health: { status: "healthy", localFallbackAllowed: true, overlayPresent: false, lastChecked: null, detail: null } };
        case "egress":
          return {
            type: "egress",
            allowlist: {
              groups: [
                { group: "core-downloads", hosts: ["downloads.claude.ai"], hostsUnobserved: false },
                { group: "essential-telemetry", hosts: [], hostsUnobserved: true },
              ],
            },
          };
        case "tabSchema":
          return { type: "schema", schema: seed.tabSchema(command.tab) };
        case "connectionSchema":
          return { type: "schema", schema: seed.connectionSchema };
        case "familyCatalog":
          return {
            type: "familyCatalog",
            families: [
              { id: "anthropic", label: "Claude API (first-party)", probeSupported: true, discovery: "none", baseKey: null, baseRequired: false, credentialSlot: "inferenceAnthropicApiKey" },
              { id: "gateway", label: "Anthropic-compatible gateway", probeSupported: true, discovery: "gateway", baseKey: "inferenceGatewayBaseUrl", baseRequired: true, credentialSlot: "inferenceGatewayApiKey" },
            ],
          };
        case "validate":
          return { type: "issues", issues: seed.issues };
        case "export":
          return {
            type: "export",
            bundle: {
              contents: [{ name: "profiles/x.json", description: "A third-party inference profile (redacted).", mayContainPrivateHosts: true }],
              files: {},
              warnings: ["This export can contain configuration that identifies your organization."],
              consentRequired: true,
            },
          };
        default:
          return { type: "error", error: { code: "UNKNOWN", message: `unknown command ${command.type}`, retryable: false } };
      }
    };

    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: "setup-desktop-3p" } },
      transformCallback: (callback) => callback,
      convertFileSrc: (path) => path,
      invoke: (cmd, args) => {
        if (cmd === "setup_command") {
          const command = args.command;
          recorded.push(command);
          return Promise.resolve(respond(command));
        }
        if (cmd === "setup_discover_models") {
          recorded.push({ type: "discoverModels", request: args.request });
          return Promise.resolve({
            models: [{ id: "claude-opus-4-1", raw: {} }],
            pages: 1,
            partial: false,
            warning: null,
          });
        }
        return Promise.reject(new Error(`unexpected command ${cmd}`));
      },
    };
  }, {
    id: PROFILE.id,
    name: PROFILE.name,
    provider: PROFILE.provider,
    document: PROFILE,
    connectionSchema: CONNECTION_SCHEMA,
    tabSchema,
    issues: [],
  });
}

const browser = await chromium.launch();
try {
  const context = await browser.newContext({ viewport: { width: 900, height: 720 } });
  const page = await context.newPage();
  await installStub(page);

  await step("S01 the setup window renders the captured layout", async () => {
    await page.goto(`${BASE}/setup-desktop-3p`);
    await page.waitForSelector(".setup-window");
    const title = await page.textContent(".setup-title");
    await expectTrue(title === "Configure third-party inference", `title was ${title}`);
    const nav = await page.$$(".setup-nav-item");
    await expectTrue(nav.length === 9, `expected 9 nav entries, found ${nav.length}`);
    const footer = await page.textContent(".setup-footer");
    await expectTrue(footer.includes("Apply Changes"), `footer was ${footer}`);
  });

  await step("S02 the nine tabs are the documented ones, in order", async () => {
    const labels = await page.$$eval(".setup-nav-item", (nodes) => nodes.map((n) => n.textContent.trim()));
    await expectTrue(
      labels.join("|") ===
        "Connection|Capabilities|Connectors|Telemetry & updates|Limits|Appearance|Plugins|Egress|Source",
      `nav was ${labels.join("|")}`,
    );
  });

  await step("S03 the Connection form is schema-driven and marks required fields", async () => {
    await page.click(".setup-nav-item:has-text('Connection')");
    await page.waitForSelector("#field-inferenceGatewayBaseUrl");
    const required = await page.$$(".setup-required");
    await expectTrue(required.length >= 3, `expected required markers, found ${required.length}`);
    const secret = await page.getAttribute("#field-inferenceGatewayApiKey", "type");
    await expectTrue(secret === "password", `the credential field was type ${secret}`);
  });

  await step("S04 the stored credential arrives as a handle, never a value", async () => {
    const value = await page.inputValue("#field-inferenceGatewayApiKey");
    await expectTrue(value === "", `the credential field held ${value}`);
    const placeholder = await page.getAttribute("#field-inferenceGatewayApiKey", "placeholder");
    await expectTrue(
      (placeholder ?? "").includes("Stored"),
      `the placeholder was ${placeholder}`,
    );
  });

  await step("S05 a clean draft leaves Apply disabled", async () => {
    const disabled = await page.getAttribute(".setup-primary-button", "disabled");
    await expectTrue(disabled !== null, "Apply should be disabled on a clean draft");
    const discard = await page.$(".setup-footer .setup-secondary-button");
    await expectTrue(discard === null, "Discard must be absent on a clean draft");
  });

  await step("S06 editing a field makes the draft dirty and enables Apply", async () => {
    await page.fill("#field-inferenceGatewayBaseUrl", "https://edited.example");
    const disabled = await page.getAttribute(".setup-primary-button", "disabled");
    await expectTrue(disabled === null, "Apply should be enabled on a dirty draft");
    const discard = await page.textContent(".setup-footer .setup-secondary-button");
    await expectTrue(discard.includes("Discard Changes"), `footer was ${discard}`);
  });

  await step("S07 Discard restores the applied values", async () => {
    await page.click(".setup-footer .setup-secondary-button");
    const value = await page.inputValue("#field-inferenceGatewayBaseUrl");
    await expectTrue(value === "https://gateway.example", `the field was ${value}`);
    const disabled = await page.getAttribute(".setup-primary-button", "disabled");
    await expectTrue(disabled !== null, "Apply should be disabled again after Discard");
  });

  await step("S08 Apply writes the draft and then applies the profile", async () => {
    await page.fill("#field-inferenceGatewayBaseUrl", "https://edited.example");
    await page.click(".setup-primary-button");
    await page.waitForFunction(() => {
      const commands = window.__mhCommands ?? [];
      return commands.some((c) => c.type === "applyProfile");
    });
    const commands = await page.evaluate(() => window.__mhCommands);
    const write = commands.find((c) => c.type === "writeProfile");
    const apply = commands.find((c) => c.type === "applyProfile");
    await expectTrue(!!write, "the window must write the draft before applying");
    await expectTrue(!!apply, "the window must apply the profile");
    await expectTrue(
      write.document.connection.inferenceGatewayBaseUrl === "https://edited.example",
      "the written document did not carry the edit",
    );
    // The written document must not carry the credential as a value.
    await expectTrue(
      typeof write.document.connection.inferenceGatewayApiKey === "object",
      "the credential must stay a handle in the written document",
    );
  });

  await step("S09 a typed credential travels on the request, not in the document", async () => {
    await page.fill("#field-inferenceGatewayApiKey", "typed-credential");
    // Regression (manual campaign finding): a secret is stored outside the
    // profile document, but the masked input must still show what was typed.
    const shown = await page.inputValue("#field-inferenceGatewayApiKey");
    await expectTrue(shown === "typed-credential", `the masked field showed "${shown}"`);
    const inputType = await page.getAttribute("#field-inferenceGatewayApiKey", "type");
    await expectTrue(inputType === "password", `the credential field type was ${inputType}`);
    await page.click(".setup-primary-button");
    await page.waitForFunction(() => {
      const commands = window.__mhCommands ?? [];
      return commands.filter((c) => c.type === "writeProfile").length >= 2;
    });
    const commands = await page.evaluate(() => window.__mhCommands);
    const writes = commands.filter((c) => c.type === "writeProfile");
    const last = writes[writes.length - 1];
    await expectTrue(
      last.secrets["connection/inferenceGatewayApiKey"] === "typed-credential",
      "the typed credential did not travel on the request",
    );
    await expectTrue(
      typeof last.document.connection.inferenceGatewayApiKey === "object",
      "the typed credential must not be written into the document",
    );
  });

  await step("S13 the setup window shows none of the main app's chrome", async () => {
    // The captured setup window is standalone (docs/screens/setup-desktop-3p.md):
    // its own header, navigation, content and footer, and no sidebar, mode
    // switch, account menu or title bar.
    for (const selector of [".sidebar", ".titlebar", ".mode-switch", ".account-item"]) {
      const found = await page.$(selector);
      await expectTrue(found === null, `${selector} leaked into the setup window`);
    }
  });

  await step("S14 a select shows the stored value, not an empty control", async () => {
    // The credential kind is stored as the wire value `static` and shown with its
    // label; a select that carried only labels could never show it.
    const shown = await page.inputValue("#field-inferenceCredentialKind");
    await expectTrue(shown === "static", `the credential kind showed ${shown}`);
    const label = await page.textContent("#field-inferenceCredentialKind option:checked");
    await expectTrue(label === "Static API key", `the label was ${label}`);
  });

  await step("S15 the export bundle is a modal over a scrim", async () => {
    await page.click(".setup-header .setup-secondary-button");
    await page.waitForSelector(".setup-export-scrim .setup-export");
    const modal = await page.getAttribute(".setup-export", "aria-modal");
    await expectTrue(modal === "true", `the export dialog was aria-modal=${modal}`);
    // The consent step must not sit under the footer's primary action.
    const exportBox = await (await page.$(".setup-export")).boundingBox();
    const applyBox = await (await page.$(".setup-primary-button")).boundingBox();
    const overlaps = !(exportBox.y + exportBox.height < applyBox.y || applyBox.y + applyBox.height < exportBox.y);
    await expectTrue(!overlaps, "the export dialog covers the Apply button");
    await page.click(".setup-export .setup-secondary-button");
  });

  await step("S16 the layout does not overflow a narrow window", async () => {
    await page.setViewportSize({ width: 700, height: 600 });
    await page.waitForTimeout(200);
    const widths = await page.evaluate(() => ({
      doc: document.documentElement.scrollWidth,
      win: window.innerWidth,
    }));
    await expectTrue(widths.doc <= widths.win, `the content overflowed: ${widths.doc} > ${widths.win}`);
    // The credential field's Reveal control must stay reachable.
    const reveal = await page.$(".setup-secret-toggle");
    await expectTrue(reveal !== null, "the credential reveal control disappeared");
    const box = await reveal.boundingBox();
    await expectTrue(box.x + box.width <= widths.win, "the reveal control is off-screen");
    await page.setViewportSize({ width: 900, height: 720 });
  });

  await step("S10 the Egress tab is read-only and shows the derived list", async () => {
    await page.click(".setup-nav-item:has-text('Egress')");
    await page.waitForSelector(".setup-egress-group");
    const hosts = await page.textContent(".setup-egress-hosts");
    await expectTrue(hosts.includes("downloads.claude.ai"), `hosts were ${hosts}`);
    // The group heading is the human label (docs/09 §2.9 names the groups); the
    // raw id stays in the data.
    const note = await page.textContent(".setup-egress-group:has-text('Essential telemetry') .setup-note");
    await expectTrue(
      note.includes("never captured"),
      `the unobserved-group note was ${note}`,
    );
    const inputs = await page.$$(".setup-content input, .setup-content select");
    await expectTrue(inputs.length === 0, `the Egress tab had ${inputs.length} editable controls`);
  });

  await step("S11 the export bundle is enumerated and warns", async () => {
    await page.click(".setup-header .setup-secondary-button");
    await page.waitForSelector(".setup-export");
    const text = await page.textContent(".setup-export");
    await expectTrue(text.includes("profiles/x.json"), `the bundle listed ${text}`);
    await expectTrue(text.includes("identifies your organization"), `the warning was ${text}`);
  });

  await step("S12 the main window never renders the setup surface", async () => {
    const mainContext = await browser.newContext({ viewport: { width: 1200, height: 800 } });
    const mainPage = await mainContext.newPage();
    await mainPage.addInitScript(() => {
      window.__TAURI_INTERNALS__ = {
        metadata: { currentWindow: { label: "main" } },
        transformCallback: (callback) => callback,
        invoke: () => Promise.reject(new Error("not available")),
      };
    });
    await mainPage.goto(`${BASE}/setup-desktop-3p`);
    await mainPage.waitForURL("**/new");
    const setupVisible = await mainPage.$(".setup-window");
    await expectTrue(setupVisible === null, "the setup surface must not render in the main window");
    await mainContext.close();
  });
} finally {
  await browser.close();
}

console.log(results.join("\n"));
console.log("");
console.log(
  `SETUP E2E SUMMARY: ${results.length} case(s) — PASS ${results.length - failures} / FAIL ${failures}`,
);
process.exit(failures === 0 ? 0 : 1);
