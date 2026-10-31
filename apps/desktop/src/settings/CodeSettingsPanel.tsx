/**
 * Settings → Claude Code (UI-SCREEN-18 §Claude Code; docs/23 §3.4 item 21,
 * captures x13; IMP-43).
 *
 * The live original's pane: "Code appearance" with two live theme preview
 * cards (Claude Light / Claude Dark) showing a syntax-highlighted sample;
 * Code font (placeholder "e.g. JetBrains Mono"); Code font size
 * ("Default · 12 px"); Transcript text size (Small/Medium/Large); Transcript
 * width (Narrow/Medium/Wide); Default transcript view (Normal/Thinking/
 * Verbose); plus a Sessions group below the fold whose contents were not
 * captured — stated unavailable, not invented.
 *
 * Persistence is renderer-local (`src/settings/prefs.ts`) until a bridge
 * phase consumes these preferences; the pane says so. Only the observed
 * defaults are evidence — the alternative option values are rebuild-defined
 * additions a select needs to be a control.
 */
import { useState } from "react";
import {
  loadCodeFont,
  loadCodeFontSize,
  loadCodeTheme,
  loadTranscriptSize,
  loadTranscriptWidth,
  loadTranscriptView,
  saveCodeFont,
  saveCodeFontSize,
  saveCodeTheme,
  saveTranscriptSize,
  saveTranscriptWidth,
  saveTranscriptView,
  type CodeFontSize,
  type CodeTheme,
  type TranscriptSize,
  type TranscriptView,
  type TranscriptWidth,
} from "./prefs";

/** The syntax-highlighted sample both preview cards render. */
const SAMPLE_LINES: { text: string; kind: "keyword" | "fn" | "string" | "comment" | "plain" }[] = [
  { text: "// Greet the workspace", kind: "comment" },
  { text: "fn greet(name: &str) {", kind: "keyword" },
  { text: '  println!("hello, {name}");', kind: "plain" },
  { text: "}", kind: "plain" },
];

function CodeSample({ theme }: { theme: CodeTheme }) {
  return (
    <pre className={`code-theme-sample code-theme-${theme}`} aria-hidden="true">
      {SAMPLE_LINES.map((line, index) => (
        <span key={index} className={`code-token-${line.kind}`}>
          {line.text}
          {"\n"}
        </span>
      ))}
    </pre>
  );
}

function CodeSettingsPanel() {
  const [codeFont, setCodeFont] = useState(loadCodeFont);
  const [codeFontSize, setCodeFontSize] = useState<CodeFontSize>(loadCodeFontSize);
  const [theme, setTheme] = useState<CodeTheme>(loadCodeTheme);
  const [transcriptSize, setTranscriptSize] = useState<TranscriptSize>(loadTranscriptSize);
  const [transcriptWidth, setTranscriptWidth] = useState<TranscriptWidth>(loadTranscriptWidth);
  const [transcriptView, setTranscriptView] = useState<TranscriptView>(loadTranscriptView);

  return (
    <div className="settings-panel code-settings">
      <p className="settings-note">
        These preferences are stored renderer-side until a bridge phase consumes
        them; they shape the Code surface when it reads them. Only the default
        values are observed evidence — the other option values are
        rebuild-defined (docs/23 §3.4 item 21; IMP-43).
      </p>
      <h3>Code appearance</h3>
      <div className="code-theme-cards" role="radiogroup" aria-label="Code theme">
        {(["claude-light", "claude-dark"] as CodeTheme[]).map((value) => (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={theme === value}
            className={`code-theme-card${theme === value ? " active" : ""}`}
            onClick={() => {
              setTheme(value);
              saveCodeTheme(value);
            }}
          >
            <CodeSample theme={value} />
            <span className="code-theme-card-label">
              {value === "claude-light" ? "Claude Light" : "Claude Dark"}
            </span>
          </button>
        ))}
      </div>
      <div className="settings-pref-row">
        <label className="settings-pref-label" htmlFor="code-font">
          Code font
        </label>
        <input
          id="code-font"
          type="text"
          placeholder="e.g. JetBrains Mono"
          value={codeFont}
          onChange={(event) => {
            setCodeFont(event.target.value);
            saveCodeFont(event.target.value);
          }}
        />
      </div>
      <div className="settings-pref-row">
        <label className="settings-pref-label" htmlFor="code-font-size">
          Code font size
        </label>
        <select
          id="code-font-size"
          value={codeFontSize}
          onChange={(event) => {
            const next = event.target.value as CodeFontSize;
            setCodeFontSize(next);
            saveCodeFontSize(next);
          }}
        >
          <option value="default">Default · 12 px</option>
          <option value="10">10 px</option>
          <option value="11">11 px</option>
          <option value="13">13 px</option>
          <option value="14">14 px</option>
        </select>
      </div>
      <div className="settings-pref-row">
        <label className="settings-pref-label" htmlFor="transcript-size">
          Transcript text size
        </label>
        <select
          id="transcript-size"
          value={transcriptSize}
          onChange={(event) => {
            const next = event.target.value as TranscriptSize;
            setTranscriptSize(next);
            saveTranscriptSize(next);
          }}
        >
          <option value="small">Small</option>
          <option value="medium">Medium</option>
          <option value="large">Large</option>
        </select>
      </div>
      <div className="settings-pref-row">
        <label className="settings-pref-label" htmlFor="transcript-width">
          Transcript width
        </label>
        <select
          id="transcript-width"
          value={transcriptWidth}
          onChange={(event) => {
            const next = event.target.value as TranscriptWidth;
            setTranscriptWidth(next);
            saveTranscriptWidth(next);
          }}
        >
          <option value="narrow">Narrow</option>
          <option value="medium">Medium</option>
          <option value="wide">Wide</option>
        </select>
      </div>
      <div className="settings-pref-row">
        <label className="settings-pref-label" htmlFor="transcript-view">
          Default transcript view
        </label>
        <select
          id="transcript-view"
          value={transcriptView}
          onChange={(event) => {
            const next = event.target.value as TranscriptView;
            setTranscriptView(next);
            saveTranscriptView(next);
          }}
        >
          <option value="normal">Normal</option>
          <option value="thinking">Thinking</option>
          <option value="verbose">Verbose</option>
        </select>
      </div>
      <h3>Sessions</h3>
      <p className="settings-note">
        The original&apos;s pane carries a Sessions group below the fold whose
        contents were not captured (docs/23 §3.4 item 21); it stays unavailable
        here rather than invented.
      </p>
    </div>
  );
}

export { CodeSettingsPanel };
