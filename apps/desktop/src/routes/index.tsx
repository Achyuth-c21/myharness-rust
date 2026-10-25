/** Route map — strict copies from docs/06-navigation-map.md §1 (paths are contract). */

export interface ScreenDef {
  path: string;
  label: string;
  /** Palette row icon surface (docs/screens/search-palette.md: task-list icon for Cowork, `</>` for Code). */
  paletteSurface: "cowork" | "code" | "projects" | "scheduled" | "artifacts";
}

/** All evidenced routes (docs/06 §1). */
export const SCREENS: ScreenDef[] = [
  { path: "/new", label: "New", paletteSurface: "cowork" }, // default entry (docs/06: runtime entered /new)
  { path: "/cowork", label: "Cowork", paletteSurface: "cowork" }, // /cowork/<session-id> detail handled too
  { path: "/cowork-artifact", label: "Artifacts", paletteSurface: "artifacts" },
  { path: "/projects", label: "Projects", paletteSurface: "projects" },
  { path: "/scheduled-task", label: "Scheduled", paletteSurface: "scheduled" },
  { path: "/epitaxy", label: "Code", paletteSurface: "code" }, // Code dashboard (route name preserved)
];

/** Sidebar items — docs/06 §2: New, Projects, Artifacts, Scheduled (+ Customize later). */
export const NAV_ITEMS: ScreenDef[] = [
  { path: "/new", label: "New", paletteSurface: "cowork" },
  { path: "/projects", label: "Projects", paletteSurface: "projects" },
  { path: "/cowork-artifact", label: "Artifacts", paletteSurface: "artifacts" },
  { path: "/scheduled-task", label: "Scheduled", paletteSurface: "scheduled" },
];

/**
 * Settings sections — docs/06 §1 hash list AND docs/screens/settings.md §Left navigation
 * (hash slugs are the verified contract; display labels + groups are the verified UI copy).
 * `developer` has no captured hash; Phase 5 renders a rebuild-defined pane behind
 * the placeholder key `developer` and labels it as unobserved.
 */
export const SETTINGS_GROUPS: {
  group: string;
  sections: { key: string; label: string; disabled?: true }[];
}[] = [
  {
    group: "Settings",
    sections: [
      { key: "general", label: "General" },
      { key: "data-privacy-controls", label: "Privacy" },
      { key: "usage", label: "Usage" },
      { key: "claude-code", label: "Claude Code" },
      { key: "cowork", label: "Cowork" },
      { key: "import", label: "Import & export" },
    ],
  },
  {
    group: "This computer",
    sections: [
      { key: "desktop", label: "System" },
      // Hash never captured (settings.md lists the section; docs/06 §1 lists hashes).
      { key: "developer", label: "Developer" },
    ],
  },
  {
    // These three are shortcuts to the Customize route, not settings panes
    // (docs/screens/settings.md §Left navigation); the Customize route was never
    // captured (docs/06 §1). Skills and Plugins open rebuild-defined panes here
    // for Phase 5; Connectors still has no implementation and stays disabled.
    group: "Customize",
    sections: [
      { key: "skills", label: "Skills" },
      { key: "connectors", label: "Connectors", disabled: true },
      { key: "plugins", label: "Plugins" },
    ],
  },
];

export type SettingsSection = string;
