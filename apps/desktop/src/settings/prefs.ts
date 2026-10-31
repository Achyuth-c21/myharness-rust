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
  // The live original's default is Anthropic Serif (docs/23 §3.4 item 18);
  // the rebuild converges to it (IMP-45).
  try {
    return localStorage.getItem(CHAT_FONT_KEY) === "sans" ? "sans" : "serif";
  } catch {
    return "serif";
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

// ─── Claude Code pane preferences (UI-SCREEN-18 §Claude Code; docs/23 §3.4
// item 21). Renderer-local until a bridge phase consumes them; every key is
// rebuild-defined and the pane says so. The option lists below carry the
// observed defaults ("Default · 12 px", Small/Medium/Large, Narrow/Medium/
// Wide, Normal/Thinking/Verbose); the unobserved alternative options are
// rebuild-defined additions, marked in the pane.

export type CodeTheme = "claude-light" | "claude-dark";
export type CodeFontSize = "default" | "10" | "11" | "13" | "14";
export type TranscriptSize = "small" | "medium" | "large";
export type TranscriptWidth = "narrow" | "medium" | "wide";
export type TranscriptView = "normal" | "thinking" | "verbose";

const CODE_FONT_KEY = "ui.codeFont";
const CODE_FONT_SIZE_KEY = "ui.codeFontSize";
const CODE_THEME_KEY = "ui.codeTheme";
const TRANSCRIPT_SIZE_KEY = "ui.transcriptSize";
const TRANSCRIPT_WIDTH_KEY = "ui.transcriptWidth";
const TRANSCRIPT_VIEW_KEY = "ui.transcriptView";

export function loadCodeFont(): string {
  try {
    return localStorage.getItem(CODE_FONT_KEY) ?? "";
  } catch {
    return "";
  }
}

export function saveCodeFont(font: string): void {
  try {
    localStorage.setItem(CODE_FONT_KEY, font);
  } catch {
    // Session-only, as above.
  }
}

export function loadCodeFontSize(): CodeFontSize {
  try {
    const raw = localStorage.getItem(CODE_FONT_SIZE_KEY);
    return (["default", "10", "11", "13", "14"] as const).includes(raw as CodeFontSize)
      ? (raw as CodeFontSize)
      : "default";
  } catch {
    return "default";
  }
}

export function saveCodeFontSize(size: CodeFontSize): void {
  try {
    localStorage.setItem(CODE_FONT_SIZE_KEY, size);
  } catch {
    // Session-only, as above.
  }
}

export function loadCodeTheme(): CodeTheme {
  try {
    return localStorage.getItem(CODE_THEME_KEY) === "claude-dark" ? "claude-dark" : "claude-light";
  } catch {
    return "claude-light";
  }
}

export function saveCodeTheme(theme: CodeTheme): void {
  try {
    localStorage.setItem(CODE_THEME_KEY, theme);
  } catch {
    // Session-only, as above.
  }
}

export function loadTranscriptSize(): TranscriptSize {
  try {
    const raw = localStorage.getItem(TRANSCRIPT_SIZE_KEY);
    return raw === "small" || raw === "large" ? raw : "medium";
  } catch {
    return "medium";
  }
}

export function saveTranscriptSize(size: TranscriptSize): void {
  try {
    localStorage.setItem(TRANSCRIPT_SIZE_KEY, size);
  } catch {
    // Session-only, as above.
  }
}

export function loadTranscriptWidth(): TranscriptWidth {
  try {
    const raw = localStorage.getItem(TRANSCRIPT_WIDTH_KEY);
    return raw === "narrow" || raw === "wide" ? raw : "medium";
  } catch {
    return "medium";
  }
}

export function saveTranscriptWidth(width: TranscriptWidth): void {
  try {
    localStorage.setItem(TRANSCRIPT_WIDTH_KEY, width);
  } catch {
    // Session-only, as above.
  }
}

export function loadTranscriptView(): TranscriptView {
  try {
    const raw = localStorage.getItem(TRANSCRIPT_VIEW_KEY);
    return raw === "thinking" || raw === "verbose" ? raw : "normal";
  } catch {
    return "normal";
  }
}

export function saveTranscriptView(view: TranscriptView): void {
  try {
    localStorage.setItem(TRANSCRIPT_VIEW_KEY, view);
  } catch {
    // Session-only, as above.
  }
}

// ─── The language choice (UI-SCREEN-20; IMP-47). Renderer-side until the
// bridge/identity phase consumes it; the dialog states that honestly.

const LOCALE_KEY = "ui.locale";

export function loadLocale(): string {
  try {
    return localStorage.getItem(LOCALE_KEY) ?? "en-US";
  } catch {
    return "en-US";
  }
}

export function saveLocale(locale: string): void {
  try {
    localStorage.setItem(LOCALE_KEY, locale);
  } catch {
    // Session-only, as above.
  }
}
