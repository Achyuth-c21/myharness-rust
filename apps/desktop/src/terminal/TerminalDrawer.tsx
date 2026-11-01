/**
 * The terminal drawer (UI: docs/23 §7 addendum item 8; IMP-55). The observed
 * header is `Terminal ˅ | + | ⋮ | expand | ✕` over a live shell prompt; the
 * ⋮ menu carries Cursor style (Block), Blink cursor, Copy on select, and
 * Scrollback (10,000 lines).
 *
 * Honest rebuild definitions (recorded in docs/19 IMP-55): the drawer renders
 * the PTY's raw output with a line input — a full VT100 emulator is not
 * invented; the cursor settings style the input caret, not an emulated cell
 * cursor. The scrollback cap follows the observed default (10,000 lines).
 * The `+` affordance (additional terminals) was not observed in use and stays
 * inert with a reason.
 */
import { useEffect, useRef, useState } from "react";
import { closeTerminal, onTerminalOutput, openTerminal, sendTerminalInput } from "./client";

const SCROLLBACK_OPTIONS = [1_000, 10_000, 100_000] as const;

interface Props {
  /** The bound workspace folder, or null for the home directory. */
  cwd: string | null;
  onClose: () => void;
}

function TerminalDrawer({ cwd, onClose }: Props) {
  const [session, setSession] = useState<number | null>(null);
  const [status, setStatus] = useState<"opening" | "live" | "unavailable" | "error" | "closed">("opening");
  const [message, setMessage] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [expanded, setExpanded] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [cursorStyle, setCursorStyle] = useState<"block" | "bar" | "underline">("block");
  const [blink, setBlink] = useState(true);
  const [copyOnSelect, setCopyOnSelect] = useState(false);
  const [scrollback, setScrollback] = useState<number>(10_000);
  const bodyRef = useRef<HTMLPreElement>(null);
  const outputRef = useRef("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void (async () => {
      const opened = await openTerminal(cwd);
      if (cancelled) {
        if (opened.kind === "opened") void closeTerminal(opened.id);
        return;
      }
      if (opened.kind === "opened") {
        setSession(opened.id);
        setStatus("live");
        unlisten = await onTerminalOutput((id, data) => {
          if (id !== opened.id) return;
          outputRef.current += data;
          setText(outputRef.current);
          if (bodyRef.current) bodyRef.current.scrollTop = bodyRef.current.scrollHeight;
        });
      } else if (opened.kind === "unavailable") {
        setStatus("unavailable");
      } else {
        setStatus("error");
        setMessage(opened.message);
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
      if (session !== null) void closeTerminal(session);
    };
    // The drawer opens once per mount; the session id is captured above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const submit = () => {
    if (session === null || !text.trim()) return;
    void sendTerminalInput(session, `${text}\n`);
    inputRef.current?.focus();
  };

  // The scrollback cap follows the observed default; older lines drop.
  const renderText =
    text.split("\n").length > scrollback
      ? text.split("\n").slice(-scrollback).join("\n")
      : text;

  return (
    <aside
      className={`terminal-drawer${expanded ? " expanded" : ""}`}
      aria-label="Terminal"
      data-cursor-style={cursorStyle}
      data-blink={blink ? "true" : "false"}
    >
      <div className="terminal-head">
        <span className="terminal-title">
          Terminal <span className="terminal-caret-glyph">˅</span>
        </span>
        <button
          type="button"
          className="icon-btn icon-btn-sm"
          aria-label="New terminal"
          disabled
          title="Additional terminals were not observed in use (docs/23 §7 item 8)"
        >
          +
        </button>
        <span className="terminal-menu-anchor">
          <button
            type="button"
            className="icon-btn icon-btn-sm"
            aria-label="Terminal settings"
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            onClick={() => setMenuOpen((open) => !open)}
          >
            ⋮
          </button>
          {menuOpen ? (
            <div className="add-menu terminal-menu" role="menu" aria-label="Terminal settings">
              <div className="permission-menu-head" role="presentation">
                Cursor style
              </div>
              {(["block", "bar", "underline"] as const).map((style) => (
                <button
                  key={style}
                  type="button"
                  role="menuitemradio"
                  aria-checked={cursorStyle === style}
                  className="add-menu-item"
                  onClick={() => setCursorStyle(style)}
                >
                  <span style={{ textTransform: "capitalize" }}>{style}</span>
                  {cursorStyle === style ? <span className="permission-check">✓</span> : null}
                </button>
              ))}
              <button
                type="button"
                role="menuitemcheckbox"
                aria-checked={blink}
                className="add-menu-item"
                onClick={() => setBlink((value) => !value)}
              >
                <span>Blink cursor</span>
                {blink ? <span className="permission-check">✓</span> : null}
              </button>
              <button
                type="button"
                role="menuitemcheckbox"
                aria-checked={copyOnSelect}
                className="add-menu-item"
                onClick={() => setCopyOnSelect((value) => !value)}
                title="Selecting text copies it to the clipboard"
              >
                <span>Copy on select</span>
                {copyOnSelect ? <span className="permission-check">✓</span> : null}
              </button>
              <div className="permission-menu-head" role="presentation">
                Scrollback
              </div>
              {SCROLLBACK_OPTIONS.map((option) => (
                <button
                  key={option}
                  type="button"
                  role="menuitemradio"
                  aria-checked={scrollback === option}
                  className="add-menu-item"
                  onClick={() => setScrollback(option)}
                >
                  <span>{option.toLocaleString()} lines</span>
                  {scrollback === option ? <span className="permission-check">✓</span> : null}
                </button>
              ))}
            </div>
          ) : null}
        </span>
        <button
          type="button"
          className="icon-btn icon-btn-sm"
          aria-label={expanded ? "Shrink terminal" : "Expand terminal"}
          onClick={() => setExpanded((value) => !value)}
        >
          {expanded ? "⇤" : "⇥"}
        </button>
        <button
          type="button"
          className="icon-btn icon-btn-sm"
          aria-label="Close terminal"
          onClick={() => {
            if (session !== null) void closeTerminal(session);
            onClose();
          }}
        >
          ✕
        </button>
      </div>
      {status === "live" ? (
        <>
          <pre className="terminal-body" ref={bodyRef} data-copy-on-select={copyOnSelect ? "true" : "false"}>
            {renderText}
          </pre>
          <div className="terminal-input-row">
            <span className="terminal-input-caret" aria-hidden="true">
              {cursorStyle === "block" ? "█" : cursorStyle === "bar" ? "▏" : "▁"}
            </span>
            <input
              ref={inputRef}
              className="terminal-input"
              aria-label="Terminal input"
              value={text}
              onChange={(event) => setText(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  submit();
                }
              }}
            />
          </div>
        </>
      ) : (
        <div className="terminal-body terminal-note" role="status">
          {status === "opening" ? "Opening the terminal…" : null}
          {status === "unavailable"
            ? "The terminal is available in the desktop app. This browser preview has no shell to attach."
            : null}
          {status === "error" ? message : null}
        </div>
      )}
    </aside>
  );
}

export { TerminalDrawer };
