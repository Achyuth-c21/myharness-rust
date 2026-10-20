/**
 * Focus trap for modal/dialog surfaces — docs/07 §5 (UI-DS-05: "Menus and dialogs
 * trap/restore focus and support Escape") and docs/06 §5 ("Settings traps focus
 * behind the scrim", "The search palette traps focus and returns it to the trigger
 * on close"). Cycles Tab/Shift+Tab among focusable descendants and restores focus
 * to the invoking element on unmount.
 */
import { useEffect, type RefObject } from "react";

const FOCUSABLE =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function useFocusTrap(container: RefObject<HTMLElement | null>, active: boolean) {
  useEffect(() => {
    if (!active) return;
    const el = container.current;
    if (!el) return;
    // Capture the invoker BEFORE any programmatic focus inside this layer — autofocus
    // must not overwrite it, or restore-on-close would target a removed node.
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;

    // Scope to the container itself (not :scope descendants of the document) so the
    // shell beneath the scrim can never join this layer's cycle (docs/06 §5).
    const focusables = () =>
      Array.from(el.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (node) => node instanceof HTMLElement && node.offsetParent !== null,
      );

    // Within-layer cycle (docs/06 §5), seeded by focusing the layer's first control
    // (settings: its search field; palette: the query input) after capturing the trigger.
    focusables()[0]?.focus();

    // docs/07 §3.1: focus is a ring shadow, never a browser outline — enforced in CSS.
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const list = focusables();
      if (list.length === 0) return;
      const index = list.indexOf(document.activeElement as HTMLElement);
      // The invoker (e.g. the palette trigger) lives OUTSIDE the layer: treat focus
      // outside the container the same as index -1 and pull focus back inside.
      const inside = index !== -1 && document.activeElement instanceof Node && el.contains(document.activeElement);
      if (event.shiftKey) {
        if (!inside || index === 0) {
          event.preventDefault();
          list[list.length - 1].focus();
        }
      } else if (!inside || index === list.length - 1) {
        event.preventDefault();
        list[0].focus();
      }
    };
    el.addEventListener("keydown", onKey, true); // capture: fires even when focus sits outside the layer
    return () => {
      el.removeEventListener("keydown", onKey, true);
      previous?.focus();
    };
  }, [container, active]);
}
