/** Route map — strict copies from docs/06-navigation-map.md §1 (paths are contract). */

export interface ScreenDef {
  path: string;
  label: string;
}

/** All evidenced routes (docs/06 §1). */
export const SCREENS: ScreenDef[] = [
  { path: "/new", label: "New" }, // default entry (docs/06: runtime entered /new)
  { path: "/cowork", label: "Cowork" }, // /cowork/<session-id> detail handled too
  { path: "/cowork-artifact", label: "Artifacts" },
  { path: "/projects", label: "Projects" },
  { path: "/scheduled-task", label: "Scheduled" },
  { path: "/epitaxy", label: "Code" }, // Code dashboard (route name preserved)
];

/** Sidebar items — docs/06 §2: New, Projects, Artifacts, Scheduled (+ Customize later). */
export const NAV_ITEMS: ScreenDef[] = [
  { path: "/new", label: "New" },
  { path: "/projects", label: "Projects" },
  { path: "/cowork-artifact", label: "Artifacts" },
  { path: "/scheduled-task", label: "Scheduled" },
];

/** Settings modal sections — docs/06 §1 hash list (settings is a modal, not a route). */
export const SETTINGS_SECTIONS = [
  "general",
  "data-privacy-controls",
  "usage",
  "claude-code",
  "cowork",
  "import",
  "desktop",
] as const;

export type SettingsSection = (typeof SETTINGS_SECTIONS)[number];
