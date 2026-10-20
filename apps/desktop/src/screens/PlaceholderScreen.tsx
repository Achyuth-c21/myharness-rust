/**
 * Generic placeholder screen — Phase 1 ships the shell + route table only (docs/13 §4).
 * Every real surface arrives in its own phase with the docs/05-screen-inventory.md
 * spec, docs/08-interaction-catalog.md controls, and docs/07 tokens.
 */
import type { ScreenDef } from "../routes";
import { EmptyShell, LoadingShell } from "../components/shells";

export function PlaceholderScreen({ screen, detail }: { screen: ScreenDef; detail?: boolean }) {
  // Boot shells render through the same primitives so empty/loading/error states get
  // exercised from day one (TEST-UX-012: states are announced accessibly).
  if (detail) {
    return <LoadingShell label={`Opening ${screen.label.toLowerCase()} task…`} />;
  }
  return (
    <EmptyShell
      title={screen.label}
      hint="This surface arrives in a later roadmap phase — route, shell, and theme are live."
    />
  );
}
