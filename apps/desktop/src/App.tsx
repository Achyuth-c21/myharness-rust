/**
 * App shell — Phase 1 (docs/13 §4): title bar, sidebar (mode switch lives in the
 * sidebar per docs/06 §4), routes, documented empty states, theme, keyboard:
 * - Ctrl+B sidebar toggle (UI-INT-001)
 * - Ctrl+N → /new, clears composer context and focuses the composer (UI-INT-005)
 * - Ctrl+K → search palette (UI-INT-020)
 * - Ctrl+, → settings modal at the default section (docs/06 §2)
 *
 * BrowserRouter is required by the settings-hash overlay (docs/06 §1: settings is a
 * hash ON the current route, never a route itself). Unknown routes redirect to /new —
 * the exact original redirect rule is unknown (docs/06); /new was the observed entry.
 */
import { useCallback, useEffect, useState } from "react";
import { Navigate, Route, Routes, useNavigate } from "react-router-dom";
import { TitleBar } from "./components/TitleBar";
import { Sidebar } from "./components/Sidebar";
import { SearchPalette } from "./components/SearchPalette";
import { SettingsModal } from "./components/SettingsModal";
import { NewTaskScreen } from "./screens/NewTaskScreen";
import { ArtifactsScreen, CodeScreen, ProjectsScreen, ScreenHead, ScheduledScreen } from "./screens/EmptyStates";
import { LoadingShell } from "./components/shells";
import { SCREENS } from "./routes";
import SetupWindow from "./setup/SetupWindow";

/**
 * Which window this renderer instance belongs to.
 *
 * The setup window is a SEPARATE window that loads the same SPA
 * (docs/09 §2.1; docs/screens/setup-desktop-3p.md). It must show only the setup
 * surface — the captured setup window has its own header, navigation, content
 * and footer, and none of the main app's chrome (sidebar, mode switch, account
 * menu, title bar). The main window must never show the setup surface either.
 */
function useWindowKind(): "main" | "setup" | null {
  const [kind, setKind] = useState<"main" | "setup" | null>(null);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      let label = "main";
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        label = await getCurrentWindow().label;
      } catch {
        label = "main";
      }
      if (cancelled) return;
      setKind(label === "setup-desktop-3p" ? "setup" : "main");
    })();
    return () => {
      cancelled = true;
    };
  }, []);
  return kind;
}

export default function App() {
  const kind = useWindowKind();
  if (kind === null) return <LoadingShell label="Starting…" />;
  // The setup window is a standalone surface: no title bar, no sidebar.
  if (kind === "setup") return <SetupWindow />;
  return <MainShell />;
}

function MainShell() {
  const navigate = useNavigate();
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [settingsSection, setSettingsSection] = useState<string | null>(null);
  const [newKey, setNewKey] = useState(0);

  // Sidebar collapse persists (docs/20 §4: renderer state, descriptive key).
  useEffect(() => {
    const raw = localStorage.getItem("sidebar.collapsed");
    if (raw === "1") setSidebarOpen(false);
  }, []);
  useEffect(() => {
    localStorage.setItem("sidebar.collapsed", sidebarOpen ? "0" : "1");
  }, [sidebarOpen]);

  // Settings hash sync (docs/06 §1): "#settings/<section>" overlays the current route.
  useEffect(() => {
    const fromHash = () => {
      const hash = window.location.hash;
      if (hash.startsWith("#settings/")) {
        setSettingsSection(hash.slice("#settings/".length));
      } else {
        setSettingsSection(null);
      }
    };
    fromHash();
    window.addEventListener("hashchange", fromHash);
    return () => window.removeEventListener("hashchange", fromHash);
  }, []);

  // pushState does not fire hashchange — sync state explicitly on open/close.
  // Both calls carry the existing history state along: React Router tracks its
  // position index there, and a `null` state would blind the title-bar
  // back/forward tracker (report-bug-hunt A1/A2 interplay).
  const openSettings = useCallback((section: string) => {
    window.history.pushState(window.history.state, "", `#settings/${section}`);
    setSettingsSection(section);
  }, []);
  const closeSettings = useCallback(() => {
    // Closing removes the hash and returns to the underlying route (docs/06 §1).
    // REPLACE the settings entry instead of pushing (report-bug-hunt A2: a push
    // left a zombie entry — Back then re-opened the modal), and keep the
    // pathname AND query string of the underlying route.
    window.history.replaceState(
      window.history.state,
      "",
      `${window.location.pathname}${window.location.search}`,
    );
    setSettingsSection(null);
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey)) return;
      const key = event.key.toLowerCase();
      // C9 (report-bug-hunt): while the user is typing in an editable control
      // with no overlay open, only New stays global (UI-INT-005 — "opens /new,
      // clears, focuses"); the others would fire mid-sentence (e.g. Ctrl+B
      // collapsing the sidebar). Overlays keep their full shortcut behavior
      // (UI-INT-020/026 flows, tests T19/T21).
      const target = event.target as HTMLElement | null;
      const typing =
        !!target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable);
      const overlayOpen = paletteOpen || settingsSection !== null;
      if (typing && !overlayOpen && key !== "n") return;
      if (key === "b") {
        event.preventDefault();
        setSidebarOpen((open) => !open);
      } else if (key === "n") {
        event.preventDefault();
        // UI-INT-005: New clears the composer context and focuses it — the remount
        // resets the draft (useState("")) and the mount effect re-focuses.
        setNewKey((k) => k + 1);
        navigate("/new");
      } else if (key === "k") {
        event.preventDefault();
        setPaletteOpen(true);
      } else if (key === ",") {
        event.preventDefault();
        openSettings("general");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate, openSettings, paletteOpen, settingsSection]);

  const screens = new Map(SCREENS.map((screen) => [screen.path, screen]));

  return (
    <div className="app-shell">
      <TitleBar
        sidebarOpen={sidebarOpen}
        onToggleSidebar={() => setSidebarOpen((open) => !open)}
      />
      <div className="app-body">
        {sidebarOpen ? (
          <Sidebar onOpenSettings={() => openSettings("general")} onOpenSearch={() => setPaletteOpen(true)} />
        ) : null}
        <main className="main-pane">
          <Routes>
            {/* Redirects must carry any settings hash with them — the hash overlays the
                route it was applied to (docs/06 §1), so `/#settings/usage` lands in the
                settings modal over /new, not nowhere. */}
            <Route
              path="/"
              element={<Navigate replace to={{ pathname: "/new", hash: window.location.hash }} />}
            />
            {/* newKey remounts the screen on Ctrl+N so the draft resets and the
                composer refocuses (UI-INT-005). The key sits on the ELEMENT, not the
                Route, so the remount also happens when already on /new. Focus goes
                with the explicit invocation; boot does not autofocus (report Run 4). */}
            <Route path="/new" element={<NewTaskScreen key={newKey} focusOnMount={newKey > 0} />} />
            {/* The setup window is a SEPARATE window (docs/09 §2.1) and renders
                its own surface; the main window has no setup route at all, so the
                setup surface is never a page of the main app. */}
            <Route path="/setup-desktop-3p" element={<Navigate replace to="/new" />} />
            <Route path="/projects" element={<ProjectsScreen />} />
            <Route path="/cowork-artifact" element={<ArtifactsScreen />} />
            <Route path="/scheduled-task" element={<ScheduledScreen />} />
            <Route
              path="/epitaxy"
              element={<CodeScreen screen={screens.get("/epitaxy") ?? { path: "/epitaxy", label: "Code", paletteSurface: "code" }} />}
            />
            {/* Task detail arrives with the sessions module (docs/13 §Phase 4);
                the loading shell announces the route contract for now. */}
            <Route path="/cowork/:sessionId" element={<LoadingShell label="Opening task…" />} />
            <Route
              path="/cowork"
              element={
                <>
                  <ScreenHead label="Cowork" />
                </>
              }
            />
            <Route
              path="*"
              element={<Navigate replace to={{ pathname: "/new", hash: window.location.hash }} />}
            />
          </Routes>
        </main>
      </div>
      {paletteOpen ? <SearchPalette onClose={() => setPaletteOpen(false)} /> : null}
      {settingsSection ? (
        <SettingsModal section={settingsSection} onSection={openSettings} onClose={closeSettings} />
      ) : null}
    </div>
  );
}
