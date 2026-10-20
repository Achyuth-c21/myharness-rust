/**
 * App shell — Phase 1 (docs/13 §4): title bar, sidebar, mode switch (via TitleBar),
 * routes, loading/empty/error shells, theme, keyboard:
 * - Ctrl+B sidebar toggle (UI-INT-001)
 * - Ctrl+N → /new, clears composer context (UI-INT-005)
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
import { PlaceholderScreen } from "./screens/PlaceholderScreen";
import { SCREENS } from "./routes";

export default function App() {
  const navigate = useNavigate();
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [settingsSection, setSettingsSection] = useState<string | null>(null);

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

  const cowork = SCREENS.find((screen) => screen.path === "/cowork");

  return (
    <div className="app-shell">
      <TitleBar
        sidebarOpen={sidebarOpen}
        onToggleSidebar={() => setSidebarOpen((open) => !open)}
      />
      <div className="app-body">
        {sidebarOpen ? <Sidebar onOpenSettings={() => openSettings("general")} /> : null}
        <main className="main-pane">
          <Routes>
            <Route path="/" element={<Navigate replace to="/new" />} />
            {SCREENS.map((screen) => (
              <Route key={screen.path} path={screen.path} element={<PlaceholderScreen screen={screen} />} />
            ))}
            {cowork ? <Route path="/cowork/:sessionId" element={<PlaceholderScreen screen={cowork} detail />} /> : null}
            <Route path="*" element={<Navigate replace to="/new" />} />
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
