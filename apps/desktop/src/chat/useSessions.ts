/**
 * Session list state shared by the sidebar and the New-task Active list.
 *
 * Each surface shows only its own rows; both refresh when a command changes the
 * list (`SESSIONS_CHANGED`) and when the window regains focus (a turn finished
 * while another window was in front). A browser preview has no core, so the
 * hook reports `unavailable` and the surfaces keep their documented empty
 * state rather than inventing rows.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import {
  chat,
  emitSessionsChanged,
  SESSIONS_CHANGED,
  type SessionRecord,
} from "./client";
import { reportBackgroundFailure } from "../components/Notice";

export interface SessionList {
  sessions: SessionRecord[];
  unavailable: boolean;
  refresh: () => Promise<void>;
}

export function useChatSessions(
  archived: boolean | null = false,
  surface: "chat" | "code" = "chat",
): SessionList {
  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [unavailable, setUnavailable] = useState(false);
  const request = useRef(0);

  const refresh = useCallback(async () => {
    const requestId = ++request.current;
    const response =
      surface === "code"
        ? await chat({ type: "listCodeSessions", archived })
        : await chat({ type: "listSessions", archived });
    // A surface/archive switch or a newer focus refresh can resolve first.
    // Never let an older response put another surface's rows in this list.
    if (requestId !== request.current) return;
    if (response.type === "sessions") {
      // The Code command already filters on the stored surface; the client
      // filter keeps the two lists honest if a future response is reused.
      setSessions(response.sessions.filter((session) => session.surface === surface));
      setUnavailable(false);
    } else if (response.type === "error" && response.error.code === "BRIDGE_UNAVAILABLE") {
      setSessions([]);
      setUnavailable(true);
    } else if (response.type === "error") {
      // A real bridge failure must not leave the list silently empty
      // (UI-SCREEN-33; docs/23 §3.8 item 34). BRIDGE_UNAVAILABLE above is the
      // browser preview's honest state and never reaches the toast surface.
      reportBackgroundFailure({
        code: response.error.code,
        message: `The session list could not load. ${response.error.message}`,
        action: { label: "Check again", run: () => void refresh() },
      });
    }
  }, [archived, surface]);

  useEffect(() => {
    // The old surface's rows are wrong the moment the route changes; clear
    // them before the new request resolves. The request id still guards the
    // response race.
    setSessions([]);
    setUnavailable(false);
    void refresh();
    const onChange = () => void refresh();
    const onFocus = () => void refresh();
    window.addEventListener(SESSIONS_CHANGED, onChange);
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener(SESSIONS_CHANGED, onChange);
      window.removeEventListener("focus", onFocus);
    };
  }, [refresh]);

  return { sessions, unavailable, refresh };
}

/** Refresh every mounted session list after a mutation. */
export { emitSessionsChanged };
