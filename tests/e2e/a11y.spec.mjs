/**
 * Whole-application test campaign — phase 1: accessibility baseline.
 *
 * Runs axe-core over the shell's primary routes in both colour modes. The
 * ruleset is WCAG 2.0/2.1/2.2 A+AA (docs/07 design system, docs/08 §13 focus
 * management, TEST-UX-012/014). The audit follows the accessibility skill's
 * evidence-led workflow: automated findings are a baseline, not a replacement
 * for the manual keyboard/screen-reader pass recorded in docs/17.
 *
 * Prereqs: `npm install` here (root), `npx playwright install chromium`,
 * renderer dev server: `npm run dev` in apps/desktop (port 5173).
 * Run: node tests/e2e/a11y.spec.mjs
 *
 * Exit code: 0 when no serious/critical WCAG violation is found; 1 otherwise.
 * Moderate/minor findings are reported but do not fail the baseline.
 */
import { chromium } from "playwright";
import AxeBuilder from "@axe-core/playwright";

const BASE = process.env.BASE_URL ?? "http://localhost:5173";

const ROUTES = [
  ["/new", "new task"],
  ["/projects", "projects"],
  ["/cowork-artifact", "artifacts"],
  ["/scheduled-task", "scheduled"],
  ["/epitaxy", "code dashboard"],
  ["/#settings/general", "settings / general"],
  ["/#settings/data-privacy-controls", "settings / privacy"],
  ["/#settings/skills", "settings / skills"],
  ["/#settings/desktop", "settings / desktop"],
];

const TAGS = ["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"];
const FAIL_IMPACTS = new Set(["serious", "critical"]);

const browser = await chromium.launch();
const findings = [];
let failing = 0;

for (const mode of ["light", "dark"]) {
  const context = await browser.newContext({
    colorScheme: mode,
    viewport: { width: 1200, height: 800 },
  });
  const page = await context.newPage();

  for (const [path, label] of ROUTES) {
    try {
      await page.goto(`${BASE}${path}`, { waitUntil: "networkidle" });
      await page.waitForTimeout(350); // overlays settle before the scan
      const { violations } = await new AxeBuilder({ page }).withTags(TAGS).analyze();
      const serious = violations.filter((v) => FAIL_IMPACTS.has(v.impact));
      if (serious.length > 0) failing += serious.length;
      findings.push({ mode, path, label, violations });
      const summary = violations.length === 0
        ? "clean"
        : violations.map((v) => `${v.id}(${v.impact}:${v.nodes.length})`).join(", ");
      console.log(`${serious.length > 0 ? "FAIL" : "PASS"} [${mode}] ${label} — ${summary}`);
    } catch (error) {
      failing += 1;
      findings.push({ mode, path, label, error: String(error) });
      console.log(`FAIL [${mode}] ${label} — audit error: ${String(error).split("\n")[0]}`);
    }
  }
  await context.close();
}
await browser.close();

const details = findings.flatMap((f) =>
  (f.violations ?? [])
    .filter((v) => FAIL_IMPACTS.has(v.impact))
    .flatMap((v) =>
      v.nodes.map((node) => {
        const check = node.any?.[0]?.data ?? {};
        return {
          mode: f.mode,
          path: f.path,
          id: v.id,
          impact: v.impact,
          target: node.target.join(" "),
          ratio: check.contrastRatio,
          fg: check.fgColor,
          bg: check.bgColor,
          expected: check.expectedContrastRatio,
          size: check.fontSize,
        };
      }),
    ),
);

console.log("");
if (details.length > 0) {
  console.log("SERIOUS/CRITICAL DETAIL");
  for (const d of details) {
    const ratio = d.ratio ? `${d.ratio}:1 (need ${d.expected}) ${d.fg} on ${d.bg} ${d.size ?? ""}` : "see axe rule";
    console.log(`  [${d.mode}] ${d.path} ${d.id} ${d.target} — ${ratio}`);
  }
}
const total = findings.reduce((n, f) => n + (f.violations?.length ?? 0), 0);
console.log(`A11Y SUMMARY: ${findings.length} route/mode scans — ${total} violation type(s), ${failing} serious/critical`);
process.exit(failing > 0 ? 1 : 0);
