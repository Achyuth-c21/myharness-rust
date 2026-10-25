/**
 * Developer settings — MCP status and tool search (Phase 5).
 *
 * The shipped Developer section hash was not captured (`docs/06` §1); this
 * pane is a rebuild-defined placeholder behind `#settings/developer`. It shows
 * redacted server states, retries, forgets OAuth, and searches live/cached
 * tools. Cached hits are metadata only and cannot execute without a connected
 * server. Tool enablement for a session is chosen in that session's Tools menu.
 */
import { useCallback, useEffect, useState } from "react";
import {
  chat,
  onMcpStatus,
  type BridgeError,
  type McpServerView,
  type ToolSearchHit,
} from "../chat/client";

function statusText(server: McpServerView): string {
  switch (server.status.state) {
    case "connected":
      return `connected · ${server.status.toolCount} tool(s)`;
    case "failed":
      return `failed · ${server.status.message}`;
    case "needsAuth":
      return `needs auth · ${server.status.message}`;
    case "starting":
      return "starting";
    default:
      return "stopped";
  }
}

export function DeveloperPanel() {
  const [servers, setServers] = useState<McpServerView[]>([]);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [error, setError] = useState<BridgeError | null>(null);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ToolSearchHit[] | null>(null);
  const [searchEnabled, setSearchEnabled] = useState(false);
  const [searched, setSearched] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    const response = await chat({ type: "mcpStatus" });
    if (response.type === "mcpStatus") {
      setServers(response.servers);
      setWarnings(response.warnings);
      setError(null);
    } else if (response.type === "error") {
      setError(response.error);
    }
  }, []);

  useEffect(() => {
    void load();
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void onMcpStatus((payload) => {
      if (payload.servers) setServers(payload.servers);
      if (payload.warnings) setWarnings(payload.warnings);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [load]);

  const search = async () => {
    setBusy("search");
    setSearched(true);
    const response = await chat({
      type: "mcpSearchTools",
      query,
      limit: 50,
    });
    if (response.type === "toolSearch") {
      setHits(response.hits);
      setSearchEnabled(response.enabled);
      setError(null);
    } else if (response.type === "error") {
      setError(response.error);
      setHits(null);
      setSearchEnabled(false);
    }
    setBusy(null);
  };

  const retry = async (name: string) => {
    setBusy(name);
    const response = await chat({ type: "retryMcpServer", server: name });
    if (response.type === "mcpStatus") {
      setServers(response.servers);
      setWarnings(response.warnings);
      setError(null);
    } else if (response.type === "error") {
      setError(response.error);
    }
    setBusy(null);
  };

  const forget = async (name: string) => {
    setBusy(name);
    const response = await chat({ type: "forgetMcpOAuth", server: name });
    if (response.type === "ack") {
      await load();
    } else if (response.type === "error") {
      setError(response.error);
    }
    setBusy(null);
  };

  return (
    <div className="settings-panel">
      <p className="settings-note">
        This pane is rebuild-defined; the shipped Developer section hash was not captured.
        Server errors are redacted and bounded. A <strong>needs auth</strong> server
        cannot succeed on retry without a token.
      </p>
      {error ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{error.code}</span>
          <span>{error.message}</span>
        </div>
      ) : null}
      <h3>MCP servers</h3>
      {servers.length === 0 ? (
        error ? null : <div className="settings-note">No MCP servers are configured.</div>
      ) : (
        <div className="settings-list">
          {servers.map((server) => (
            <div key={server.name} className="settings-row">
              <div>
                <div className="settings-row-title">{server.name}</div>
                <div className="settings-row-sub">{statusText(server)}</div>
              </div>
              <div className="settings-row-actions">
                <button
                  type="button"
                  className="text-btn"
                  disabled={busy === server.name || server.status.state === "needsAuth"}
                  title={
                    server.status.state === "needsAuth"
                      ? "Sign-in is not implemented in this build; retrying without a token cannot succeed"
                      : "Retry this server"
                  }
                  onClick={() => void retry(server.name)}
                >
                  Retry
                </button>
                <button
                  type="button"
                  className="text-btn"
                  disabled={busy === server.name}
                  onClick={() => void forget(server.name)}
                >
                  Forget OAuth
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
      <h3>Tool search</h3>
      <div className="settings-inline">
        <input
          className="settings-search settings-path"
          placeholder="Search tool names and descriptions"
          aria-label="Tool search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void search();
          }}
        />
        <button
          type="button"
          className="text-btn"
          disabled={busy === "search"}
          onClick={() => void search()}
        >
          Search
        </button>
      </div>
      {hits ? (
        hits.length === 0 ? (
          <div className="settings-note">No tools matched.</div>
        ) : (
          <div className="settings-list">
            {hits.map((hit) => (
              <div key={hit.tool.name} className="settings-row">
                <div>
                  <div className="settings-row-title">
                    {hit.tool.name}
                    {hit.cached ? <span className="settings-tag">cached</span> : null}
                  </div>
                  <div className="settings-row-sub">
                    {hit.tool.description || "No description."}
                  </div>
                </div>
                <div className="settings-row-sub">score {hit.score}</div>
              </div>
            ))}
          </div>
        )
      ) : null}
      {searched && !searchEnabled && hits === null ? (
        <div className="settings-note">
          Tool search is disabled; the command reported why rather than returning a
          partial catalog.
        </div>
      ) : null}
      {warnings.length > 0 ? (
        <details className="settings-warnings" open>
          <summary>{warnings.length} MCP warning(s)</summary>
          <ul>
            {warnings.map((warning, index) => (
              <li key={index}>{warning}</li>
            ))}
          </ul>
        </details>
      ) : null}
    </div>
  );
}
