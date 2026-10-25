/**
 * Bridge client for the chat subset of `web.LocalAgentModeSessions`
 * (docs/18 §3.4/§4; crates/bridge/src/chat.rs).
 *
 * The renderer never talks to a provider, a database, or a file: every call is
 * one typed command through `invoke`, and streaming arrives as
 * `chat://turn-event` keyed by session. A failure is a typed `error` response,
 * not an exception (docs/18 §3.2), except when the window has no Tauri host at
 * all — the browser preview — which maps to `BRIDGE_UNAVAILABLE` so the UI can
 * say so honestly instead of pretending.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface BridgeError {
  code: string;
  message: string;
  retryable: boolean;
}

/** A stored session row (crates/store/src/repos.rs `SessionRow`). */
export interface SessionRecord {
  id: string;
  surface: string;
  title: string;
  cwd: string | null;
  model: string | null;
  effort: string | null;
  /** Stored permission spelling (e.g. `default`, `acceptEdits`). */
  permission_mode: string | null;
  /** Soft project association; a deleted project id resolves to "missing". */
  projectId: string | null;
  archived: boolean;
  /** Stored snake_case spelling (the row's wire contract). */
  legal_hold: boolean;
  createdAt: number;
  lastActivityAt: number;
  extra: unknown;
}

/** One transcript content block (`provider_sdk::ContentPart`). */
export interface ContentPart {
  type: "text" | "image" | "document" | "tool_use" | "tool_result" | "reasoning" | "redacted_thinking";
  text?: string;
  thinking?: string;
  signature?: string | null;
  name?: string;
  [key: string]: unknown;
}

/** Stored turn content (`agent::TurnContent`). */
export interface TurnContent {
  parts: ContentPart[];
  citations?: unknown[];
}

export interface TurnSummary {
  id: string;
  seq: number;
  role: "user" | "assistant" | "system" | "tool";
  status: "complete" | "partial" | "error";
  stopReason: string | null;
  error: string | null;
  createdAt: number;
  content: TurnContent;
}

export interface SessionView {
  session: SessionRecord;
  turns: TurnSummary[];
}

export interface Usage {
  input: number;
  output: number;
  cacheRead?: number | null;
  cacheWrite?: number | null;
  webSearch?: number | null;
}

/** One event out of a turn's stream (`provider_sdk::TurnEvent`). */
export type TurnEvent =
  | { type: "text_delta"; text: string }
  | { type: "reasoning_delta"; text: string; signature?: string | null }
  | { type: "reasoning_redacted"; data: unknown }
  | { type: "citation"; citation: unknown }
  | {
      type: "tool_call";
      id: string;
      name: string;
      arguments_delta?: string | null;
      arguments?: unknown;
    }
  | { type: "tool_result"; id: string; parts: ContentPart[] }
  | { type: "usage"; usage: Usage }
  | { type: "stop"; reason: unknown }
  // Provider errors on the stream carry the provider's class/status/message;
  // the terminal command response carries the bridge envelope.
  | { type: "error"; error: { class: string; status?: number | null; message: string } }
  | { type: "done" };

export interface UsageTotals {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  searchRequests: number;
  events: number;
  errorEvents: number;
}

export interface DailyUsage {
  date: string;
  totals: UsageTotals;
}

export interface UsageSummary {
  days: number;
  timezoneOffsetMinutes: number;
  from: string;
  to: string;
  totals: UsageTotals;
  daily: DailyUsage[];
}

export type CodeRange = "7d" | "30d" | "all";
export type CodePermissionMode = "default" | "acceptEdits" | "plan" | "bypass";

/**
 * A stored project row (`crates/store/src/repos_projects.rs` `ProjectRow`)
 * plus the bridge-computed folder-availability fact. Field spellings follow
 * the stored row's wire contract: `memory_enabled` is snake_case, the
 * timestamps are camelCase (`docs/18` row-shape convention).
 */
export interface ProjectRecord {
  id: string;
  name: string;
  instructions: string | null;
  cwd: string;
  memory_enabled: boolean;
  createdAt: number;
  lastActivityAt: number;
  extra: unknown;
  /** False when the folder no longer resolves to the recorded spelling. */
  folderAvailable: boolean;
}

/**
 * A stored scheduled task (`crates/store/src/repos_scheduler.rs`
 * `ScheduledTaskRow`) plus the bridge-derived summary and last run.
 * `armedAt` is the single pending occurrence; `null` while disabled.
 */
export interface ScheduledTaskRecord {
  id: string;
  name: string;
  prompt: string;
  cron: string;
  enabled: boolean;
  projectId: string | null;
  cwd: string | null;
  createdAt: number;
  armedAt: number | null;
  lastRunAt: number | null;
  extra: unknown;
  scheduleSummary: string;
  lastRun: ScheduledRunRecord | null;
  contextUnavailable: boolean;
}

/** One recorded run (or skip) of a scheduled task. */
export interface ScheduledRunRecord {
  id: string;
  taskId: string;
  startedAt: number;
  status: "running" | "completed" | "failed" | "skipped";
  reason: string | null;
  sessionId: string | null;
  finishedAt: number | null;
}

/**
 * One stored memory entry (`crates/memory` `MemoryEntry`): Markdown content
 * with its timestamps. Content is personal and local-only.
 */
export interface MemoryEntryRecord {
  id: string;
  content: string;
  createdAt: number;
  updatedAt: number;
}

/** One captured template card (name + schedule label are the captured copy). */
export interface ScheduledTemplateCard {
  name: string;
  prompt: string;
  cron: string;
  scheduleLabel: string;
}

/** The effective Code policy; folder roots never cross the bridge. */
export interface CodePolicyView {
  codeEnabled: boolean;
  localSessionsEnabled: boolean;
  bypassAllowed: boolean;
  workspaceRestricted: boolean;
}

export interface CodeModelUsage {
  model: string;
  totals: UsageTotals;
  sessions: number;
  sharePercent: number;
}

/** Code-only dashboard data (`sessions::CodeUsageSummary`). */
export interface CodeUsageSummary {
  range: CodeRange;
  days: number;
  truncated: boolean;
  timezoneOffsetMinutes: number;
  from: string;
  to: string;
  totals: UsageTotals;
  daily: DailyUsage[];
  messages: number;
  sessions: number;
  activeDays: number;
  peakHour: number | null;
  favoriteModel: string | null;
  models: CodeModelUsage[];
}

export interface CodeRepositoryView {
  cwd: string;
  sessionCount: number;
  lastActivityAt: number;
}

export type ChatCommand =
  | { type: "listSessions"; archived?: boolean | null }
  | {
      type: "createSession";
      title?: string | null;
      /** Bind a project's working context at create time (REQ-PROJ-05). */
      projectId?: string | null;
      /** Bind a one-off folder; validated and canonicalized by the bridge. */
      cwd?: string | null;
    }
  | { type: "listProjects" }
  | {
      type: "createProject";
      name: string;
      instructions?: string | null;
      cwd: string;
      memoryEnabled?: boolean | null;
    }
  | { type: "openProject"; id: string; archived?: boolean | null }
  | {
      type: "updateProject";
      id: string;
      name: string;
      instructions?: string | null;
      memoryEnabled?: boolean | null;
    }
  | { type: "deleteProject"; id: string }
  | { type: "createArtifactTask" }
  | { type: "listScheduledTasks" }
  | { type: "scheduledTemplates" }
  | {
      type: "createScheduledTask";
      name: string;
      prompt: string;
      cron: string;
      projectId?: string | null;
      cwd?: string | null;
      enabled?: boolean | null;
    }
  | { type: "updateScheduledTask"; id: string; name: string; prompt: string; cron: string }
  | { type: "setScheduledTaskEnabled"; id: string; enabled: boolean }
  | { type: "deleteScheduledTask"; id: string }
  | { type: "runScheduledTaskNow"; id: string }
  | { type: "previewSchedule"; cron: string; count?: number | null }
  | { type: "memoryOverview" }
  | { type: "listMemoryEntries" }
  | { type: "createMemoryEntry"; content: string }
  | { type: "updateMemoryEntry"; id: string; content: string }
  | { type: "deleteMemoryEntry"; id: string }
  | { type: "resetMemory" }
  | { type: "listCodeSessions"; archived?: boolean | null }
  | {
      type: "createCodeSession";
      title?: string | null;
      cwd: string;
      permissionMode: CodePermissionMode;
    }
  | { type: "openCodeSession"; id: string }
  | { type: "sendCodeTurn"; sessionId: string; text: string; maxTokens?: number | null }
  | {
      type: "codeDashboard";
      range?: CodeRange | null;
      utcOffsetMinutes?: number | null;
    }
  | { type: "openSession"; id: string }
  | { type: "sendTurn"; sessionId: string; text: string; maxTokens?: number | null }
  | { type: "stopTurn"; sessionId: string }
  | { type: "renameSession"; id: string; title: string }
  | { type: "archiveSession"; id: string; archived: boolean }
  | { type: "deleteSession"; id: string }
  | { type: "usageSummary"; days?: number | null; utcOffsetMinutes?: number | null }
  | { type: "mcpStatus" }
  | { type: "mcpTools" }
  | { type: "mcpSearchTools"; query: string; limit?: number | null }
  | { type: "setSessionTools"; sessionId: string; tools: string[] }
  | {
      type: "respondToolApproval";
      approvalId: string;
      decision: "allow" | "deny";
      scope?: ApprovalScope | null;
    }
  | { type: "retryMcpServer"; server: string }
  | { type: "forgetMcpOAuth"; server: string }
  | { type: "listSkills" }
  | { type: "selectSkill"; name: string }
  | { type: "setSkillEnabled"; name: string; enabled: boolean }
  | { type: "setSkillCreationEnabled"; enabled: boolean }
  | { type: "listPlugins" }
  | { type: "installPlugin"; path: string }
  | { type: "uninstallPlugin"; name: string }
  | { type: "setPluginEnabled"; name: string; enabled: boolean }
  | { type: "setPluginSignaturePolicy"; policy: SignaturePolicy };

export type ChatResponse =
  | { type: "sessions"; sessions: SessionRecord[] }
  | { type: "session"; session: SessionView }
  | { type: "projects"; projects: ProjectRecord[] }
  | { type: "project"; project: ProjectRecord }
  | { type: "projectDetail"; project: ProjectRecord; sessions: SessionRecord[] }
  | { type: "projectDeleted"; orphanedSessions: number }
  | {
      type: "scheduledTasks";
      tasks: ScheduledTaskRecord[];
    }
  | { type: "scheduledTask"; task: ScheduledTaskRecord }
  | { type: "scheduledRun"; run: ScheduledRunRecord }
  | { type: "templates"; templates: ScheduledTemplateCard[] }
  | { type: "schedulePreview"; times: number[] }
  | {
      type: "memoryOverview";
      policy: { memoryEnabled: boolean };
      baselineHash: string | null;
      baselineChanged: boolean;
      count: number;
    }
  | { type: "memoryEntries"; entries: MemoryEntryRecord[] }
  | { type: "memoryEntry"; entry: MemoryEntryRecord }
  | { type: "memoryReset"; deleted: number }
  | { type: "codeDashboard"; summary: CodeUsageSummary; sessions: SessionRecord[]; repositories: CodeRepositoryView[]; policy: CodePolicyView }
  | {
      type: "turn";
      sessionId: string;
      userTurn: TurnSummary;
      assistantTurn: TurnSummary;
      stopReason: string | null;
      usage: Usage | null;
    }
  | {
      type: "toolTurn";
      sessionId: string;
      userTurn: TurnSummary;
      assistantTurns: TurnSummary[];
      toolTurns: TurnSummary[];
      stopReason: string | null;
      usage: Usage | null;
      warnings: string[];
    }
  | { type: "stopped"; stopped: boolean }
  | { type: "ack" }
  | { type: "usage"; summary: UsageSummary }
  | { type: "mcpStatus"; servers: McpServerView[]; warnings: string[] }
  | { type: "toolSearch"; hits: ToolSearchHit[]; enabled: boolean }
  | { type: "mcpTools"; tools: ToolDescriptor[] }
  | { type: "skills"; view: SkillsView }
  | { type: "skillSelection"; selection: { name: string; instructions: string } }
  | { type: "plugins"; view: PluginsView }
  | { type: "pluginInstalled"; plugin: PluginView }
  | { type: "error"; error: BridgeError; userTurnRecorded: boolean };


/** Approval lifetime offered by the core (`agent::tool_policy::ApprovalScope`). */
export type ApprovalScope = "once" | "session" | "persistent";

/** One `chat://tool-approval` payload. Arguments are a redacted preview. */
export interface ToolApprovalEvent {
  id: string;
  sessionId: string;
  callId: string;
  toolName: string;
  displayName: string;
  argumentsPreview: unknown;
  allowedScopes: ApprovalScope[];
}

/** One MCP server state (`mcp::ServerStatus`). */
export type ServerStatus =
  | { state: "stopped" }
  | { state: "starting" }
  | { state: "connected"; server: unknown; toolCount: number }
  | { state: "failed"; message: string }
  | { state: "needsAuth"; message: string };

export interface McpServerView {
  name: string;
  status: ServerStatus;
  error?: string | null;
}

/** One namespaced tool (`mcp::ToolDescriptor`). Schemas are untrusted. */
export interface ToolDescriptor {
  name: string;
  server: string;
  tool: string;
  title?: string | null;
  description?: string | null;
  inputSchema: unknown;
  outputSchema?: unknown;
}

export interface ToolSearchHit {
  tool: ToolDescriptor;
  score: number;
  cached: boolean;
}

export type SkillSource = "org" | "user" | "plugin" | "bundled";

export interface SkillView {
  name: string;
  displayName: string;
  description: string;
  version: string;
  source: SkillSource;
  runnable: boolean;
  hasScript: boolean;
}

export interface SkillsView {
  skills: SkillView[];
  disabled: string[];
  creationEnabled: boolean;
  warnings: string[];
}

export type PluginSource = "marketplace" | "org" | "local";
export type SignatureStatus = "unsigned" | "unverified";
export type SignaturePolicy = "optional" | "required";

export interface PluginView {
  name: string;
  displayName: string;
  version: string;
  description: string;
  source: PluginSource;
  readOnly: boolean;
  signature: SignatureStatus;
  permissions: string[];
  servers: string[];
  skills: string[];
  commands: string[];
}

export interface PluginsView {
  plugins: PluginView[];
  disabled: string[];
  signaturePolicy: SignaturePolicy;
  warnings: string[];
}

/** Send one command. A missing Tauri host is reported, never faked. */
export async function chat(command: ChatCommand): Promise<ChatResponse> {
  try {
    return await invoke<ChatResponse>("chat_command", { command });
  } catch (error) {
    // A missing host (the browser preview) is a different fact from a command
    // that failed inside the app. Do not report a real failure as "not
    // available" — the message still travels verbatim either way.
    const hostPresent =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    return {
      type: "error",
      error: {
        code: hostPresent ? "BRIDGE_ERROR" : "BRIDGE_UNAVAILABLE",
        message:
          typeof error === "string"
            ? error
            : error instanceof Error
              ? error.message
              : hostPresent
                ? "The chat command failed."
                : "The chat service is not available in this window.",
        retryable: true,
      },
      userTurnRecorded: false,
    };
  }
}

export type FolderPickResult =
  | { kind: "picked"; path: string }
  | { kind: "cancelled" }
  | { kind: "error"; error: BridgeError };

/**
 * Native folder selection. Cancellation is `cancelled`; an invocation failure
 * is an error, and a browser preview reports `BRIDGE_UNAVAILABLE` instead of
 * inventing a folder. `rfd` does not expose a platform-picker failure
 * separately from cancellation, so the bridge always revalidates the path it
 * receives as untrusted input.
 */
export async function pickWorkspaceFolder(): Promise<FolderPickResult> {
  try {
    const path = await invoke<string | null>("pick_workspace_folder");
    return path ? { kind: "picked", path } : { kind: "cancelled" };
  } catch (error) {
    const hostPresent =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!hostPresent) {
      return {
        kind: "error",
        error: {
          code: "BRIDGE_UNAVAILABLE",
          message: "Folder selection is available in the desktop app.",
          retryable: true,
        },
      };
    }
    const record =
      typeof error === "object" && error !== null
        ? (error as { code?: unknown; message?: unknown })
        : null;
    return {
      kind: "error",
      error: {
        code:
          typeof record?.code === "string" ? record.code : "FOLDER_PICKER_FAILED",
        message:
          typeof record?.message === "string"
            ? record.message
            : typeof error === "string"
              ? error
              : error instanceof Error
                ? error.message
                : "The folder picker could not be opened.",
        retryable: true,
      },
    };
  }
}

/**
 * Open a project/session folder in the OS file manager. The path is
 * revalidated by the shell (absolute, existing, still canonical) before
 * anything opens; a browser preview reports `BRIDGE_UNAVAILABLE`.
 */
export async function revealWorkspaceFolder(path: string): Promise<BridgeError | null> {
  try {
    await invoke("reveal_workspace_folder", { path });
    return null;
  } catch (error) {
    const hostPresent =
      typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    const record =
      typeof error === "object" && error !== null ? (error as { code?: unknown; message?: unknown }) : null;
    return {
      code: typeof record?.code === "string" ? record.code : hostPresent ? "FOLDER_UNAVAILABLE" : "BRIDGE_UNAVAILABLE",
      message:
        typeof record?.message === "string"
          ? record.message
          : typeof error === "string"
            ? error
            : "The folder could not be opened.",
      retryable: false,
    };
  }
}

/**
 * Subscribe to this app's stream events. In a browser preview the event
 * bridge is absent; the returned unlistener is then a no-op.
 */
export async function onTurnEvent(
  handler: (sessionId: string, event: TurnEvent) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<{ sessionId: string; event: TurnEvent }>(
      "chat://turn-event",
      (payload) => handler(payload.payload.sessionId, payload.payload.event),
    );
  } catch {
    return () => {};
  }
}

/** Approval requests from the core. No-op without a Tauri host. */
export async function onToolApproval(
  handler: (event: ToolApprovalEvent) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<ToolApprovalEvent>("chat://tool-approval", (payload) =>
      handler(payload.payload),
    );
  } catch {
    return () => {};
  }
}

/** Background MCP boot/retry status. No-op without a Tauri host. */
export async function onMcpStatus(
  handler: (payload: {
    summary?: unknown;
    servers?: McpServerView[];
    warnings?: string[];
  }) => void,
): Promise<UnlistenFn> {
  try {
    return await listen("chat://mcp-status", (payload) => handler(payload.payload as never));
  } catch {
    return () => {};
  }
}

/** Window event fired after any command that changes the session list. */
export const SESSIONS_CHANGED = "myharness:sessions-changed";

export function emitSessionsChanged(): void {
  window.dispatchEvent(new Event(SESSIONS_CHANGED));
}

/** The concatenated text of a turn, for the first transcript surface. */
export function plainText(content: TurnContent): string {
  return content.parts
    .filter((part): part is ContentPart & { type: "text"; text: string } => part.type === "text")
    .map((part) => part.text)
    .join("");
}

/** Tool calls in stored order (`ContentPart::ToolUse`). */
export function toolUses(
  content: TurnContent,
): { id: string; name: string; input: unknown }[] {
  return content.parts
    .filter(
      (part): part is ContentPart & { type: "tool_use"; id: string; name: string; input: unknown } =>
        part.type === "tool_use" && typeof part.id === "string" && typeof part.name === "string",
    )
    .map((part) => ({ id: part.id, name: part.name, input: part.input }));
}

/** Tool results in stored order (`ContentPart::ToolResult`). */
export function toolResults(
  content: TurnContent,
): { toolUseId: string; parts: ContentPart[]; isError: boolean }[] {
  return content.parts
    .filter(
      (part): part is ContentPart & {
        type: "tool_result";
        tool_use_id: string;
        content?: ContentPart[];
        is_error?: boolean;
      } => part.type === "tool_result" && typeof part.tool_use_id === "string",
    )
    .map((part) => ({
      toolUseId: part.tool_use_id,
      parts: Array.isArray(part.content) ? part.content : [],
      isError: part.is_error === true,
    }));
}

/** Reasoning blocks in stored order (rendered behind a disclosure). */
export function reasoningText(content: TurnContent): string {
  return content.parts
    .filter(
      (part): part is ContentPart & { type: "reasoning"; thinking: string } =>
        part.type === "reasoning" && typeof part.thinking === "string",
    )
    .map((part) => part.thinking)
    .join("\n\n");
}
