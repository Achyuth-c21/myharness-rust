/**
 * Customize — the observed route `/customize` (UI-SCREEN-15/16/17;
 * docs/screens/customize.md; docs/23 §4 item 1; IMP for the route build).
 *
 * The live audit observed this route for the first time: tabs Skills |
 * Connectors | Plugins, search + filter + sort tools, the black `+ Add ˅`
 * split button, the "Add your first skills" banner with a paper-plane
 * illustration, and the "From Anthropic & Partners" catalog. The rebuild's
 * sidebar item was disabled-honest until this route rendered.
 *
 * Data is real: Skills from the bridge's `listSkills` view, Connectors from
 * `mcpStatus` (the honest source for a local-MCP build — the observed badge
 * vocabulary is Desktop + Local dev), Plugins from the catalog view. The Add
 * split's menu and the Discover flow were never observed; they render as
 * labelled placeholders rather than dead or fake actions (docs/14 §9).
 */
import { useEffect, useMemo, useRef, useState } from "react";
import {
  chat,
  type BridgeError,
  type McpServerView,
  type PluginView,
  type SkillView,
} from "../chat/client";

type Tab = "skills" | "connectors" | "plugins";

function PaperPlaneArt() {
  return (
    <svg width="88" height="72" viewBox="0 0 88 72" fill="none" aria-hidden="true" className="empty-art">
      <path d="M8 40 76 12 52 62l-10-16L8 40Z" stroke="currentColor" strokeWidth="2.4" strokeLinejoin="round" />
      <path d="M42 46 76 12M32 52c-4 4-6 10-6 14 4-2 9-4 12-8" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" strokeDasharray="2.5 5" />
    </svg>
  );
}

function PuzzleArt() {
  return (
    <svg width="88" height="72" viewBox="0 0 88 72" fill="none" aria-hidden="true" className="empty-art">
      <path
        d="M30 24h10a6 6 0 1 1 12 0h10v10a6 6 0 1 0 0 12v10H52a6 6 0 1 0-12 0H30V46a6 6 0 1 1 0-12V24Z"
        stroke="currentColor"
        strokeWidth="2.4"
        strokeLinejoin="round"
      />
      <path d="M14 18c4-6 10-8 16-6M74 50c-2 8-8 12-16 12" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeDasharray="2.5 5.5" />
    </svg>
  );
}

function SearchGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="7" cy="7" r="4.25" stroke="currentColor" strokeWidth="1.2" />
      <path d="m10.2 10.2 3 3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function FilterGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M2.5 4h11M4.5 8h7M6.5 12h3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function SortGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m5 6.5 1.75-2 1.75 2M6.75 4.5v7M11 9.5l-1.75 2-1.75-2M9.25 11.5v-7" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function CheckGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m3.5 8.5 3 3 6-6.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function ServerGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="2" y="2.75" width="12" height="4.5" rx="1.2" stroke="currentColor" strokeWidth="1.2" />
      <rect x="2" y="8.75" width="12" height="4.5" rx="1.2" stroke="currentColor" strokeWidth="1.2" />
      <circle cx="4.6" cy="5" r="0.7" fill="currentColor" />
      <circle cx="4.6" cy="11" r="0.7" fill="currentColor" />
    </svg>
  );
}

function PluginGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M6.5 2.5h3v2a1.75 1.75 0 1 0 0 3.5v2h-3v-2a1.75 1.75 0 1 1 0-3.5v-2Z"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
        transform="translate(0 2)"
      />
    </svg>
  );
}

/** The observed source label: bundled catalog skills read "from Anthropic". */
function skillSourceLabel(skill: SkillView): string {
  if (skill.source === "bundled") return "from Anthropic";
  if (skill.source === "org") return "from your organization";
  if (skill.source === "plugin") return "from a plugin";
  return "by you";
}

export function CustomizeScreen() {
  const [tab, setTab] = useState<Tab>("skills");
  const [query, setQuery] = useState("");
  const [connectorChip, setConnectorChip] = useState<"all" | "connected" | "not-connected">("all");
  const [skills, setSkills] = useState<SkillView[] | null>(null);
  const [servers, setServers] = useState<McpServerView[] | null>(null);
  const [plugins, setPlugins] = useState<PluginView[] | null>(null);
  const [error, setError] = useState<BridgeError | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const addRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([
      chat({ type: "listSkills" }),
      chat({ type: "mcpStatus" }),
      chat({ type: "listPlugins" }),
    ]).then(([skillRows, serverRows, pluginRows]) => {
      if (cancelled) return;
      if (skillRows.type === "skills") {
        setSkills(skillRows.view.skills);
      }
      if (serverRows.type === "mcpStatus") setServers(serverRows.servers);
      if (pluginRows.type === "plugins") setPlugins(pluginRows.view.plugins);
      const firstError = [skillRows, serverRows, pluginRows].find((r) => r.type === "error");
      if (firstError && firstError.type === "error") setError(firstError.error);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!addOpen) return;
    const onOutside = (event: MouseEvent) => {
      if (event.target instanceof Node && addRef.current?.contains(event.target)) return;
      setAddOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setAddOpen(false);
    };
    document.addEventListener("mousedown", onOutside);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onOutside);
      window.removeEventListener("keydown", onKey);
    };
  }, [addOpen]);

  const queryTrimmed = query.trim().toLowerCase();

  const catalogSkills = useMemo(() => {
    if (!skills) return [];
    return skills.filter(
      (skill) =>
        skill.source !== "user" &&
        (!queryTrimmed ||
          skill.displayName.toLowerCase().includes(queryTrimmed) ||
          skill.name.toLowerCase().includes(queryTrimmed) ||
          skill.description.toLowerCase().includes(queryTrimmed)),
    );
  }, [skills, queryTrimmed]);

  const yourSkills = useMemo(() => {
    if (!skills) return [];
    return skills.filter(
      (skill) =>
        skill.source === "user" &&
        (!queryTrimmed ||
          skill.displayName.toLowerCase().includes(queryTrimmed) ||
          skill.name.toLowerCase().includes(queryTrimmed)),
    );
  }, [skills, queryTrimmed]);

  const visibleServers = useMemo(() => {
    if (!servers) return [];
    return servers.filter((server) => {
      if (connectorChip === "connected") return server.status.state === "connected";
      if (connectorChip === "not-connected") return server.status.state !== "connected";
      return true;
    });
  }, [servers, connectorChip]);

  const visiblePlugins = useMemo(() => {
    if (!plugins) return [];
    return plugins.filter(
      (plugin) =>
        !queryTrimmed ||
        plugin.displayName.toLowerCase().includes(queryTrimmed) ||
        plugin.name.toLowerCase().includes(queryTrimmed),
    );
  }, [plugins, queryTrimmed]);

  const bridgeUnavailable = error?.code === "BRIDGE_UNAVAILABLE";
  const loaded = skills !== null || servers !== null || plugins !== null;

  return (
    <div className="customize">
      <header className="customize-head">
        <h1 className="customize-title">Customize</h1>
        <div className="customize-tools">
          <span className="customize-search-box">
            <SearchGlyph />
            <input
              type="search"
              className="customize-search"
              placeholder="Search"
              aria-label="Search customize"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
          </span>
          <button type="button" className="icon-btn" aria-label="Filter" disabled title="Filtering vocabulary was not observed on this route (docs/screens/customize.md)">
            <FilterGlyph />
          </button>
          <button type="button" className="icon-btn" aria-label="Sort" disabled title="The sort order was not observed on this route (docs/screens/customize.md)">
            <SortGlyph />
          </button>
          {/* The observed black split button (docs/23 §4 item 1). Its menu's
              contents were never observed — inert rather than a dead action. */}
          <span className="sched-split" ref={addRef}>
            <button type="button" className="btn-primary sched-split-main" disabled title="The Add menu's contents were not observed (docs/screens/customize.md)">
              + Add
            </button>
            <button
              type="button"
              className="btn-primary sched-split-caret"
              aria-label="Add menu"
              aria-expanded={addOpen}
              disabled
              title="The Add menu's contents were not observed (docs/screens/customize.md)"
            >
              ›
            </button>
          </span>
        </div>
      </header>

      <div className="customize-tabs" role="tablist" aria-label="Customize sections">
        {(["skills", "connectors", "plugins"] as Tab[]).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={tab === value}
            className={`customize-tab${tab === value ? " active" : ""}`}
            onClick={() => setTab(value)}
          >
            {value === "skills" ? "Skills" : value === "connectors" ? "Connectors" : "Plugins"}
          </button>
        ))}
      </div>

      {error && !bridgeUnavailable ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{error.code}</span>
          <span>{error.message}</span>
        </div>
      ) : null}
      {bridgeUnavailable ? (
        <div className="code-notice" role="status">
          <strong>Customize is available in the desktop app.</strong>
          <span>This browser preview has no skill catalog or MCP state to read.</span>
        </div>
      ) : null}

      {!loaded && !bridgeUnavailable ? <div className="screen-empty" aria-busy="true" /> : null}

      {tab === "skills" && loaded ? (
        <>
          {yourSkills.length === 0 && catalogSkills.length === 0 ? (
            <div className="customize-banner">
              <div className="customize-banner-main">
                <h2>Add your first skills</h2>
                <p>Give Claude reusable instructions it can apply to any task.</p>
              </div>
              <PaperPlaneArt />
            </div>
          ) : null}
          {yourSkills.length > 0 ? (
            <section className="customize-section">
              <h3>Created by you</h3>
              <div className="customize-list">
                {yourSkills.map((skill) => (
                  <div key={skill.name} className="customize-row">
                    <span className="customize-row-icon" aria-hidden="true">
                      <PluginGlyph />
                    </span>
                    <span className="customize-row-main">
                      <span className="customize-row-name">{skill.displayName}</span>
                      <span className="customize-row-sub">
                        by you · {skill.description || "No description."}
                      </span>
                    </span>
                    <span className="customize-row-tag">{skill.runnable ? "" : "not runnable"}</span>
                  </div>
                ))}
              </div>
            </section>
          ) : null}
          {catalogSkills.length > 0 ? (
            <section className="customize-section">
              <h3>From Anthropic &amp; Partners</h3>
              <div className="customize-list">
                {catalogSkills.map((skill) => (
                  <div key={skill.name} className="customize-row">
                    <span className="customize-row-icon" aria-hidden="true">
                      <PluginGlyph />
                    </span>
                    <span className="customize-row-main">
                      <span className="customize-row-name">{skill.displayName}</span>
                      <span className="customize-row-sub">
                        {skillSourceLabel(skill)} · {skill.description || "No description."}
                      </span>
                    </span>
                  </div>
                ))}
              </div>
            </section>
          ) : null}
        </>
      ) : null}

      {tab === "connectors" && loaded ? (
        <>
          <div className="customize-chips" role="group" aria-label="Connector filter">
            {(["all", "connected", "not-connected"] as const).map((chip) => (
              <button
                key={chip}
                type="button"
                className={`customize-chip${connectorChip === chip ? " active" : ""}`}
                aria-pressed={connectorChip === chip}
                onClick={() => setConnectorChip(chip)}
              >
                {chip === "all" ? "All" : chip === "connected" ? "Connected" : "Not connected"}
              </button>
            ))}
          </div>
          {visibleServers.length === 0 ? (
            <div className="settings-note">No connectors to show.</div>
          ) : (
            <table className="customize-table">
              <thead>
                <tr>
                  <th scope="col">Connector</th>
                  <th scope="col">Type</th>
                  <th scope="col">Status</th>
                </tr>
              </thead>
              <tbody>
                {visibleServers.map((server) => (
                  <tr key={server.name}>
                    <td>
                      <span className="customize-row-icon" aria-hidden="true">
                        <ServerGlyph />
                      </span>
                      {server.name}
                    </td>
                    <td>
                      {/* The observed badge vocabulary (docs/23 §3.4 item 26):
                          a local-MCP build is Desktop + Local dev. */}
                      <span className="customize-badge">Desktop</span>
                      <span className="customize-badge">Local dev</span>
                    </td>
                    <td>
                      {server.status.state === "connected" ? (
                        <span className="customize-status-ok" aria-label="Connected">
                          <CheckGlyph />
                        </span>
                      ) : (
                        <button
                          type="button"
                          className="text-btn"
                          disabled
                          title="Connector setup is not built yet; retry lives in the Developer pane"
                        >
                          Connect
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      ) : null}

      {tab === "plugins" && loaded ? (
        visiblePlugins.length === 0 ? (
          <div className="customize-banner">
            <div className="customize-banner-main">
              <h2>Add your first plugins</h2>
              <p>Give Claude role-level expertise with plugins.</p>
            </div>
            <PuzzleArt />
          </div>
        ) : (
          <div className="customize-list">
            {visiblePlugins.map((plugin) => (
              <div key={plugin.name} className="customize-row">
                <span className="customize-row-icon" aria-hidden="true">
                  <PluginGlyph />
                </span>
                <span className="customize-row-main">
                  <span className="customize-row-name">{plugin.displayName}</span>
                  <span className="customize-row-sub">
                    {plugin.description || "No description."}
                  </span>
                </span>
              </div>
            ))}
          </div>
        )
      ) : null}
    </div>
  );
}
