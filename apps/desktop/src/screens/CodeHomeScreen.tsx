/**
 * Code home — the observed session-list landing for Code mode
 * (`/epitaxy`; UI-SCREEN-31; docs/screens/code-shell.md; FEAT-CODE-12;
 * IMP-40; TEST-CODE-010).
 *
 * The live audit (docs/23 §3.6 item 31) observed this surface for the first
 * time: a "New session" header with terminal and globe glyphs, a bottom
 * composer ("Describe a task or ask a question") with folder chips
 * ("This computer" + the bound folder) and a right-aligned
 * `Auto | Default model | send` row. The rebuild's previous landing was a
 * usage-stats dashboard — a rebuild design written before this IA was
 * observed; it stays reachable, honestly labeled, at `/epitaxy/dashboard`
 * (IMP-40 records the disposition; it must not silently disappear).
 *
 * The main pane's body below the header was not populated in the audit, so
 * nothing is invented there: the middle stays open. Everything interactive is
 * real: the folder picker, the trust gate (UI-SCREEN-32 — an untrusted folder
 * opens the observed modal before any session exists), the permission modes,
 * and the create flow. "Default model" is the observed placeholder; the model
 * picker itself arrives with the model phase and stays disabled-honest. The
 * placement chip is "This computer" only — remote placement does not exist in
 * this build, so it renders as a chip, not a control that would pretend to
 * switch (`docs/14` §9).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import {
  chat,
  checkWorkspaceTrust,
  emitSessionsChanged,
  pickWorkspaceFolder,
  trustWorkspace,
  type BridgeError,
  type ChatResponse,
  type CodePermissionMode,
} from "../chat/client";
import { codeDraftKey, loadDraft, saveDraft } from "../chat/drafts";
import { titleFromMessage } from "../chat/format";
import { TrustWorkspaceModal } from "../components/TrustWorkspaceModal";
import { LoadingShell } from "../components/shells";

type CodeDashboard = Extract<ChatResponse, { type: "codeDashboard" }>;

function TerminalGlyph() {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
      <path d="m4.5 6 2.4 2L4.5 10M8.4 10.4h3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function GlobeGlyph() {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="1.2" />
      <ellipse cx="8" cy="8" rx="2.6" ry="6" stroke="currentColor" strokeWidth="1.1" />
      <path d="M2.2 8h11.6" stroke="currentColor" strokeWidth="1.1" />
    </svg>
  );
}

function FolderGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M2 4.5A1.5 1.5 0 0 1 3.5 3h3l1.4 1.5h4.6A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5v-7Z"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function ComputerGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="2" y="3" width="12" height="8" rx="1.2" stroke="currentColor" strokeWidth="1.2" />
      <path d="M5.5 13.5h5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function folderName(path: string | null): string {
  if (!path) return "Choose folder";
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

export function CodeHomeScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const taRef = useRef<HTMLTextAreaElement>(null);
  const sendingRef = useRef(false);
  const [policy, setPolicy] = useState<CodeDashboard["policy"] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<BridgeError | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<BridgeError | null>(null);
  const [draft, setDraft] = useState("");
  const [permission, setPermission] = useState<CodePermissionMode>("default");
  const [creating, setCreating] = useState(false);
  const [trustPrompt, setTrustPrompt] = useState<{ folder: string } | null>(null);
  const [trustBusy, setTrustBusy] = useState(false);
  const [trustError, setTrustError] = useState<string | null>(null);

  // The effective permission policy is real bridge data (codeDashboard's
  // policy view); Bypass stays visible but disabled unless it explicitly
  // allows it (fail-closed, the dashboard's rule).
  useEffect(() => {
    let cancelled = false;
    void chat({ type: "codeDashboard" }).then((response) => {
      if (cancelled) return;
      if (response.type === "codeDashboard") setPolicy(response.policy);
      else if (response.type === "error") setError(response.error);
      setLoading(false);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // A sidebar tree `+` lands here with the folder to start in — including
  // when the landing is already mounted, so the effect tracks the state
  // value; clearing the router state would blind the title-bar back/forward
  // tracker (report-bug-hunt A1/A2).
  const preset = (location.state as { folder?: string } | null)?.folder;
  const draftLoaded = useRef(false);
  useEffect(() => {
    if (preset) {
      setFolder(preset);
      taRef.current?.focus();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preset]);
  useEffect(() => {
    if (draftLoaded.current) return;
    draftLoaded.current = true;
    setDraft(loadDraft(codeDraftKey()));
  }, []);

  const chooseFolder = async () => {
    setFolderError(null);
    const result = await pickWorkspaceFolder();
    if (result.kind === "picked") {
      setFolder(result.path);
      taRef.current?.focus();
      return;
    }
    if (result.kind === "cancelled") return;
    setFolderError(result.error);
  };

  const createSessionAndOpen = useCallback(
    async (cwd: string, text: string, mode: CodePermissionMode) => {
      sendingRef.current = true;
      setCreating(true);
      try {
        const response = await chat({
          type: "createCodeSession",
          title: titleFromMessage(text) === "New chat" ? "New Code task" : titleFromMessage(text),
          cwd,
          permissionMode: mode,
        });
        if (response.type === "session") {
          emitSessionsChanged();
          navigate(`/epitaxy/${response.session.session.id}`, {
            state: { pendingText: text, originKey: codeDraftKey() },
          });
        } else if (response.type === "error") {
          setFolderError(response.error);
          saveDraft(codeDraftKey(), text);
        }
      } finally {
        sendingRef.current = false;
        setCreating(false);
      }
    },
    [navigate],
  );

  const send = async () => {
    if (!draft.trim() || !folder || sendingRef.current || creating) return;
    const text = draft;
    // The trust gate (UI-SCREEN-32): ask the bridge before the create. The
    // bridge refuses an untrusted folder regardless — the modal is how the
    // decision gets made, not the enforcement itself.
    const verdict = await checkWorkspaceTrust(folder);
    if (verdict.kind === "checked") {
      if (!verdict.trusted) {
        setTrustError(null);
        setTrustPrompt({ folder: verdict.folder });
        return;
      }
      await createSessionAndOpen(verdict.folder, text, permission);
      return;
    }
    // A preview's BRIDGE_UNAVAILABLE keeps flowing to the create below, whose
    // honest state is the documented behavior; a real failure surfaces.
    if (verdict.error.code !== "BRIDGE_UNAVAILABLE") {
      setFolderError(verdict.error);
      return;
    }
    await createSessionAndOpen(folder, text, permission);
  };

  const cancelTrust = () => {
    // Cancel aborts: no session, no folder access, nothing recorded.
    setTrustPrompt(null);
    setTrustError(null);
  };

  const confirmTrust = async () => {
    if (!trustPrompt || trustBusy) return;
    setTrustBusy(true);
    setTrustError(null);
    const result = await trustWorkspace(trustPrompt.folder);
    if (result.kind === "error") {
      setTrustBusy(false);
      setTrustError(
        result.error.code === "BRIDGE_UNAVAILABLE"
          ? "Trusting a workspace is available in the desktop app."
          : result.error.message,
      );
      return;
    }
    setTrustBusy(false);
    setTrustPrompt(null);
    await createSessionAndOpen(result.folder, draft, permission);
  };

  const updateDraft = (value: string) => {
    setDraft(value);
    setFolderError(null);
    saveDraft(codeDraftKey(), value);
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!(event.nativeEvent as KeyboardEvent).isComposing) void send();
    }
  };

  if (loading) return <LoadingShell label="Loading Code…" />;

  const bridgeUnavailable = error?.code === "BRIDGE_UNAVAILABLE";
  const codeDisabled = policy !== null && (!policy.codeEnabled || !policy.localSessionsEnabled);

  return (
    <div className="code-home">
      <header className="code-home-head">
        <h1 className="code-home-title">New session</h1>
        {/* The observed glyphs (docs/23 §3.6 item 31); their functions were
            never observed, so they render inert instead of pretending. */}
        <span className="code-home-glyphs" title="These controls' functions were not observed (docs/23 §3.6)">
          <TerminalGlyph />
          <GlobeGlyph />
        </span>
      </header>

      {/* The observed body below the header was not populated in the audit;
          the middle stays open rather than inventing content. */}
      <div className="code-home-body" />

      <div className="composer-card code-home-composer">
        <div className="code-context-row">
          {/* Local is the only placement in this build; a chip, not a control
              that would pretend to switch (docs/14 §9). */}
          <span className="code-placement-chip" title="Local placement (the only placement in this build)">
            <ComputerGlyph />
            <span>This computer</span>
          </span>
          {folder ? (
            <button
              type="button"
              className="code-folder-chip"
              title={folder}
              onClick={() => void chooseFolder()}
            >
              <FolderGlyph />
              <span>{folderName(folder)}</span>
            </button>
          ) : (
            <button type="button" className="text-btn code-choose-folder" onClick={() => void chooseFolder()}>
              Choose folder
            </button>
          )}
        </div>
        <textarea
          ref={taRef}
          className="composer-input"
          placeholder="Describe a task or ask a question"
          aria-label="Describe a task or ask a question"
          value={draft}
          rows={1}
          onChange={(event) => {
            updateDraft(event.target.value);
            event.target.style.height = "auto";
            event.target.style.height = `${Math.min(event.target.scrollHeight, 200)}px`;
          }}
          onKeyDown={onKeyDown}
        />
        <div className="code-home-toolbar">
          <select
            className="code-permission-select"
            aria-label="Permission mode"
            value={permission}
            onChange={(event) => setPermission(event.target.value as CodePermissionMode)}
          >
            <option value="default">Default</option>
            <option value="acceptEdits">Accept edits</option>
            <option value="plan">Plan</option>
            <option value="bypass" disabled>
              Bypass
            </option>
          </select>
          {/* The observed "Default model" placeholder (docs/23 §3.6 item 31);
              the picker itself arrives with the model phase. */}
          <button
            type="button"
            className="code-model-label"
            disabled
            title="The model picker arrives with the model phase"
          >
            Default model
          </button>
          <span className="composer-spacer" />
          <button
            type="button"
            className={`send-btn${draft.trim() && folder ? " primary" : ""}`}
            aria-label="Send"
            disabled={!draft.trim() || !folder || creating || codeDisabled}
            onClick={() => void send()}
          >
            <ArrowUpIcon />
          </button>
        </div>
      </div>

      {folderError ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{folderError.code}</span>
          <span>{folderError.message} Your draft and folder are kept.</span>
        </div>
      ) : null}
      {bridgeUnavailable ? (
        <div className="code-notice" role="status">
          <strong>Code is available in the desktop app.</strong>
          <span>This browser preview has no local session store or folder picker.</span>
        </div>
      ) : null}
      {policy && !policy.codeEnabled ? (
        <div className="code-notice" role="status">
          Code is disabled by the applied configuration.
        </div>
      ) : null}
      {policy && policy.codeEnabled && !policy.localSessionsEnabled ? (
        <div className="code-notice" role="status">
          Local Code sessions are disabled. Remote placement is not implemented in this build.
        </div>
      ) : null}
      {trustPrompt ? (
        <TrustWorkspaceModal
          folder={trustPrompt.folder}
          busy={trustBusy}
          error={trustError}
          onCancel={cancelTrust}
          onTrust={() => void confirmTrust()}
        />
      ) : null}
    </div>
  );
}

function ArrowUpIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M8 13V3.5M4 7.5 8 3.5l4 4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
