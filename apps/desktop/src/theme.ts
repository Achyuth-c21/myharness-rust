/**
 * Theme handling — docs/07 §2.1 (system/light/dark; root carries `data-theme="claude"`
 * and a resolved `data-mode`) and docs/20 §4 (persisted key: `theme`).
 *
 * Theme must resolve before the first paint (docs/02 §2: the original applies the
 * persisted mode before the window is shown) — so this module is invoked synchronously
 * at startup in main.tsx, before React renders.
 */

export type ThemeMode = "system" | "light" | "dark";
export type ResolvedMode = "light" | "dark";

const KEY = "theme";
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");

export function loadMode(): ThemeMode {
  const raw = localStorage.getItem(KEY);
  if (raw === "light" || raw === "dark" || raw === "system") return raw;
  return "system";
}

export function resolveMode(mode: ThemeMode): ResolvedMode {
  return mode === "system" ? (darkQuery.matches ? "dark" : "light") : mode;
}

export function applyMode(mode: ThemeMode): ResolvedMode {
  localStorage.setItem(KEY, mode);
  const resolved = resolveMode(mode);
  const root = document.documentElement;
  root.dataset.theme = "claude";
  root.dataset.mode = resolved;
  return resolved;
}

/** Sync module init: no flash, no CSP-incompatible inline script (docs/10 §5.1). */
export function initTheme(): void {
  applyMode(loadMode());
  darkQuery.addEventListener("change", () => {
    if (loadMode() === "system") applyMode("system");
  });
}
