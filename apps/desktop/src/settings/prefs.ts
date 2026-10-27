/**
 * General-pane preferences that live entirely in the renderer (docs/screens/
 * settings.md §General): the chat typeface, the motion preference, and the
 * response-completion notification switch. Keys are our own (the original's
 * storage key types are recorded in evidence/localstorage-key-types.json; no
 * chat-font/motion/notify key was captured), so the spellings here are
 * rebuild-defined and documented.
 */

export type ChatFont = "sans" | "serif";
export type MotionPref = "system" | "reduced";

const CHAT_FONT_KEY = "ui.chatFont";
const MOTION_KEY = "ui.motion";
const NOTIFY_KEY = "ui.notifyOnDone";

export function loadChatFont(): ChatFont {
  try {
    return localStorage.getItem(CHAT_FONT_KEY) === "serif" ? "serif" : "sans";
  } catch {
    return "sans";
  }
}

export function applyChatFont(font: ChatFont): void {
  try {
    localStorage.setItem(CHAT_FONT_KEY, font);
  } catch {
    // Storage unavailable (private mode): the choice applies for the session.
  }
  document.documentElement.dataset.chatFont = font;
}

export function loadMotion(): MotionPref {
  try {
    return localStorage.getItem(MOTION_KEY) === "reduced" ? "reduced" : "system";
  } catch {
    return "system";
  }
}

export function applyMotion(pref: MotionPref): void {
  try {
    localStorage.setItem(MOTION_KEY, pref);
  } catch {
    // Session-only, as above.
  }
  document.documentElement.dataset.motion = pref;
}

/** Restore both at boot; called once from main.tsx next to the theme restore. */
export function restoreInterfacePrefs(): void {
  applyChatFont(loadChatFont());
  applyMotion(loadMotion());
}

export function loadNotifyOnDone(): boolean {
  try {
    return localStorage.getItem(NOTIFY_KEY) === "1";
  } catch {
    return false;
  }
}

export function saveNotifyOnDone(enabled: boolean): void {
  try {
    localStorage.setItem(NOTIFY_KEY, enabled ? "1" : "0");
  } catch {
    // The switch still reflects this session's choice.
  }
}

/** Fire the documented "response completed" notification when allowed. */
export function notifyTurnDone(title: string): void {
  if (!loadNotifyOnDone()) return;
  if (typeof Notification === "undefined" || Notification.permission !== "granted") return;
  try {
    new Notification("Claude", { body: `${title} — response completed.` });
  } catch {
    // Notification construction can throw on some platforms; never break the send.
  }
}
