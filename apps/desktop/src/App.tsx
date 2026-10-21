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

export default function App() {
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
  const openSettings = useCallback((section: string) => {
    window.history.pushState(null, "", `#settings/${section}`);
    setSettingsSection(section);
  }, []);
  const closeSettings = useCallback(() => {
    // Closing removes the hash and returns to the underlying route (docs/06 §1).
    window.history.pushState(null, "", window.location.pathname);
    setSettingsSection(null);
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey)) return;
      const key = event.key.toLowerCase();
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
  }, [navigate, openSettings]);

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
