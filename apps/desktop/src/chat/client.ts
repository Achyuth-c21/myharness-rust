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
  /** Stored snake_case spelling (the row's wire contract). */
  permission_mode: string | null;
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
  role: "user" | "assistant";
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

export type ChatCommand =
  | { type: "listSessions"; archived?: boolean | null }
  | { type: "createSession"; title?: string | null }
  | { type: "openSession"; id: string }
  | { type: "sendTurn"; sessionId: string; text: string; maxTokens?: number | null }
  | { type: "stopTurn"; sessionId: string }
  | { type: "renameSession"; id: string; title: string }
  | { type: "archiveSession"; id: string; archived: boolean }
  | { type: "deleteSession"; id: string }
  | { type: "usageSummary"; days?: number | null; utcOffsetMinutes?: number | null };

export type ChatResponse =
  | { type: "sessions"; sessions: SessionRecord[] }
  | { type: "session"; session: SessionView }
  | {
      type: "turn";
      sessionId: string;
      userTurn: TurnSummary;
      assistantTurn: TurnSummary;
      stopReason: string | null;
      usage: Usage | null;
    }
  | { type: "stopped"; stopped: boolean }
  | { type: "ack" }
  | { type: "usage"; summary: UsageSummary }
  | { type: "error"; error: BridgeError; userTurnRecorded: boolean };

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
