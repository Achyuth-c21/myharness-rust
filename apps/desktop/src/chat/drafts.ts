/**
 * Composer drafts (`TEST-SESSION-009`, `REQ-COMPOSER-04`).
 *
 * The shipped app keeps unsent text in renderer local storage; the exact scope
 * was never observed (docs/features/chat-and-composer.md: "Closing the window
 * preserves drafts where local storage supports them; exact scope is
 * **unknown**"). The rebuild therefore keeps drafts in the renderer, keyed by
 * the composer's identity:
 *
 * - `composer-draft:new` — the New task composer;
 * - `composer-draft:<sessionId>` — each task's follow-up composer.
 *
 * A draft is cleared only when a turn completes. Any failure — before the
 * user turn is persisted or after it (a provider error) — keeps the text
 * exactly where the user left it (`docs/features/chat-and-composer.md`
 * §Errors: "A provider failure preserves the draft"). Nothing here is sent to
 * the core; the store is the transcript truth, never the draft.
 */

const NEW_DRAFT_KEY = "composer-draft:new";

export function newDraftKey(): string {
  return NEW_DRAFT_KEY;
}

export function sessionDraftKey(sessionId: string): string {
  return `composer-draft:${sessionId}`;
}

export function loadDraft(key: string): string {
  try {
    return localStorage.getItem(key) ?? "";
  } catch {
    return "";
  }
}

export function saveDraft(key: string, value: string): void {
  try {
    if (value.length > 0) localStorage.setItem(key, value);
    else localStorage.removeItem(key);
  } catch {
    // Storage can be unavailable (private mode, quota). The draft then lives
    // only for this mount; nothing is fabricated in its place.
  }
}

export function clearDraft(key: string): void {
  try {
    localStorage.removeItem(key);
  } catch {
    // See saveDraft.
  }
}
