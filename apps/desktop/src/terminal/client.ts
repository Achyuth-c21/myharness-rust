/**
 * The terminal drawer client (docs/23 §7 addendum item 8; IMP-55). The
 * renderer never touches the PTY: open/input/close cross the bridge, and the
 * output arrives on the `terminal://output` event. A browser preview has no
 * Tauri host — the open reports that honestly and the drawer says so.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type TerminalOpen =
  | { kind: "opened"; id: number }
  | { kind: "unavailable" }
  | { kind: "error"; message: string };

export async function openTerminal(cwd: string | null): Promise<TerminalOpen> {
  const hostPresent = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  if (!hostPresent) {
    return { kind: "unavailable" };
  }
  try {
    const opened = await invokeTerminalOpen(cwd);
    return { kind: "opened", id: opened.id };
  } catch (error) {
    return {
      kind: "error",
      message: typeof error === "string" ? error : error instanceof Error ? error.message : "The terminal could not be opened.",
    };
  }
}

async function invokeTerminalOpen(cwd: string | null): Promise<{ id: number }> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<{ id: number }>("terminal_open", { cwd });
}

export async function sendTerminalInput(id: number, data: string): Promise<void> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("terminal_input", { id, data });
  } catch {
    // A failed input on a dying session surfaces through the closed output
    // stream; the drawer's error state covers it.
  }
}

export async function closeTerminal(id: number): Promise<void> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("terminal_close", { id });
  } catch {
    // The drain is the bridge's; a failed close notification is harmless.
  }
}

export function onTerminalOutput(handler: (id: number, data: string) => void): Promise<UnlistenFn> {
  return listen<{ id: number; data: string }>("terminal://output", (event) => {
    handler(event.payload.id, event.payload.data);
  });
}
