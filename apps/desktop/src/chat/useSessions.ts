/**
 * Session list state shared by the sidebar and the New-task Active list.
 *
 * Both surfaces show the same rows; both refresh when a command changes the
 * list (`SESSIONS_CHANGED`) and when the window regains focus (a turn finished
 * while another window was in front). A browser preview has no core, so the
 * hook reports `unavailable` and the surfaces keep their documented empty
 * state rather than inventing rows.
 */
import { useCallback, useEffect, useState } from "react";
import {
  chat,
  emitSessionsChanged,
  SESSIONS_CHANGED,
  type SessionRecord,
} from "./client";

export interface SessionList {
  sessions: SessionRecord[];
  unavailable: boolean;
  refresh: () => Promise<void>;
}

export function useChatSessions(archived: boolean | null = false): SessionList {
  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [unavailable, setUnavailable] = useState(false);

  const refresh = useCallback(async () => {
    const response = await chat({ type: "listSessions", archived });
    if (response.type === "sessions") {
      // Phase 4 renders the chat surface; other surfaces arrive with their
      // own screens (Code is a separate roadmap phase).
      setSessions(response.sessions.filter((session) => session.surface === "chat"));
      setUnavailable(false);
    } else if (response.type === "error" && response.error.code === "BRIDGE_UNAVAILABLE") {
      setSessions([]);
      setUnavailable(true);
    }
  }, [archived]);

  useEffect(() => {
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
