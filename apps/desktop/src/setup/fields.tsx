/**
 * Schema-driven form controls for the setup window (docs/screens/setup-desktop-3p.md
 * "Connection tab": two-column label/control layout, help text below labels,
 * required fields marked with a red `*`, password fields with reveal and lock
 * icons, native-looking selects, blue switches).
 *
 * The control vocabulary comes from the bridge's schema (`crates/config::forms`
 * and `crates/config::tabs`), so the renderer holds no per-provider knowledge.
 */
import { useState } from "react";
import type { FieldDef, FormIssue } from "./client";

export interface FieldProps {
  field: FieldDef;
  value: unknown;
  issues: FormIssue[];
  /** True when the field is managed and therefore read-only (DATA-CONFIG-04). */
  locked?: boolean;
  /** The source of a locked field, shown next to it. */
  source?: string;
  /** Called for every edit of a non-secret field. */
  onChange: (value: unknown) => void;
  /** Called when a secret is typed. The value never travels back to the renderer. */
  onSecret?: (value: string) => void;
}

/** The issues that apply to one field. */
function issuesFor(issues: FormIssue[], field: FieldDef): FormIssue[] {
  return issues.filter((issue) => issue.field === field.key);
}

export function Field({ field, value, issues, locked, source, onChange, onSecret }: FieldProps) {
  const [revealed, setRevealed] = useState(false);
  const fieldIssues = issuesFor(issues, field);
  const hasError = fieldIssues.some((issue) => issue.severity === "error");
  const describedBy = fieldIssues.length ? `${field.key}-help` : undefined;

  const label = (
    <label className="setup-label" htmlFor={`field-${field.key}`}>
      <span className="setup-label-text">
        {field.label}
        {field.required ? <span className="setup-required" aria-hidden="true">*</span> : null}
      </span>
      {field.help ? <span className="setup-help-text">{field.help}</span> : null}
    </label>
  );

  const control = (() => {
    switch (field.kind) {
      case "switch":
        return (
          <input
            id={`field-${field.key}`}
            className="setup-switch"
            type="checkbox"
            checked={value === true}
            disabled={locked}
            aria-describedby={describedBy}
            onChange={(event) => onChange(event.target.checked)}
          />
        );
      case "select":
        return (
          <select
            id={`field-${field.key}`}
            className="setup-select"
            value={typeof value === "string" ? value : ""}
            disabled={locked}
            aria-describedby={describedBy}
            onChange={(event) => onChange(event.target.value)}
          >
            {/* A blank option is the "unset" state: the shipped form distinguishes
                "blank means default" from an explicit zero (docs/09 §3.4). */}
            <option value=""></option>
            {(field.options ?? []).map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        );
      case "number":
        return (
          <input
            id={`field-${field.key}`}
            className="setup-input"
            type="number"
            value={typeof value === "number" || typeof value === "string" ? String(value) : ""}
            disabled={locked}
            aria-describedby={describedBy}
            onChange={(event) =>
              onChange(event.target.value === "" ? "" : Number(event.target.value))
            }
          />
        );
      case "url":
        return (
          <input
            id={`field-${field.key}`}
            className="setup-input"
            type="url"
            value={typeof value === "string" ? value : ""}
            disabled={locked}
            aria-describedby={describedBy}
            onChange={(event) => onChange(event.target.value)}
          />
        );
      case "secret":
        // A stored credential reaches the renderer as a HANDLE, never a value
        // (docs/18 §10), so the field is empty and the placeholder says a
        // credential is already there. Typing replaces it.
        const hasStored = value !== undefined && value !== null && value !== "";
        return (
          <div className="setup-secret">
            <input
              id={`field-${field.key}`}
              className="setup-input"
              type={revealed ? "text" : "password"}
              value={typeof value === "string" ? value : ""}
              disabled={locked}
              placeholder={hasStored ? "Stored — type to replace" : ""}
              aria-describedby={describedBy}
              onChange={(event) => onSecret?.(event.target.value)}
            />
            <button
              type="button"
              className="setup-secret-toggle"
              aria-label={revealed ? "Hide the credential" : "Reveal the credential"}
              aria-pressed={revealed}
              disabled={locked}
              onClick={() => setRevealed((shown) => !shown)}
            >
              {revealed ? "Hide" : "Reveal"}
            </button>
          </div>
        );
      case "tag-list":
        return (
          <TagList
            id={`field-${field.key}`}
            values={Array.isArray(value) ? (value as string[]) : []}
            disabled={locked}
            describedBy={describedBy}
            onChange={onChange}
          />
        );
      case "key-value":
        return (
          <KeyValue
            id={`field-${field.key}`}
            value={value && typeof value === "object" ? (value as Record<string, string>) : {}}
            disabled={locked}
            describedBy={describedBy}
            onChange={onChange}
          />
        );
      default:
        return (
          <input
            id={`field-${field.key}`}
            className="setup-input"
            type="text"
            value={typeof value === "string" ? value : ""}
            disabled={locked}
            aria-describedby={describedBy}
            onChange={(event) => onChange(event.target.value)}
          />
        );
    }
  })();

  return (
    <div className={`setup-field${hasError ? " setup-field-error" : ""}${locked ? " setup-field-locked" : ""}`}>
      {label}
      <div className="setup-control">
        {control}
        {locked ? (
          <span className="setup-locked-note">
            Managed{source ? ` by ${source}` : ""} — read-only
          </span>
        ) : null}
        {fieldIssues.length ? (
          <ul className="setup-issues" id={describedBy}>
            {fieldIssues.map((issue) => (
              <li key={`${issue.severity}-${issue.message}`} className={`setup-issue-${issue.severity}`}>
                {issue.message}
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </div>
  );
}

/** A tag-style list input (egress hosts, allowed domains, folders). */
function TagList({
  id,
  values,
  disabled,
  describedBy,
  onChange,
}: {
  id: string;
  values: string[];
  disabled?: boolean;
  describedBy?: string;
  onChange: (value: unknown) => void;
}) {
  const [text, setText] = useState("");
  const add = () => {
    const next = text.trim();
    if (!next || values.includes(next)) return;
    onChange([...values, next]);
    setText("");
  };
  return (
    <div className="setup-tags">
      {values.length ? (
        <ul className="setup-tag-list">
          {values.map((value) => (
            <li key={value} className="setup-tag">
              <span>{value}</span>
              <button
                type="button"
                className="setup-tag-remove"
                aria-label={`Remove ${value}`}
                disabled={disabled}
                onClick={() => onChange(values.filter((entry) => entry !== value))}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      <input
        id={id}
        className="setup-input"
        type="text"
        value={text}
        disabled={disabled}
        aria-describedby={describedBy}
        placeholder="Add and press Enter"
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === ",") {
            event.preventDefault();
            add();
          }
        }}
      />
    </div>
  );
}

/** A key/value editor (custom inference headers, OIDC blobs). */
function KeyValue({
  id,
  value,
  disabled,
  describedBy,
  onChange,
}: {
  id: string;
  value: Record<string, string>;
  disabled?: boolean;
  describedBy?: string;
  onChange: (value: unknown) => void;
}) {
  const entries = Object.entries(value);
  const update = (index: number, key: string, next: string) => {
    const copy = entries.map((entry, at) => (at === index ? [key, next] as const : entry));
    onChange(Object.fromEntries(copy));
  };
  return (
    <div className="setup-kv" id={id} aria-describedby={describedBy}>
      {entries.map(([key, entryValue], index) => (
        <div className="setup-kv-row" key={`${key}-${index}`}>
          <input
            className="setup-input"
            type="text"
            value={key}
            disabled={disabled}
            aria-label="Name"
            onChange={(event) => update(index, event.target.value, entryValue)}
          />
          <input
            className="setup-input"
            type="text"
            value={entryValue}
            disabled={disabled}
            aria-label="Value"
            onChange={(event) => update(index, key, event.target.value)}
          />
          <button
            type="button"
            className="setup-tag-remove"
            aria-label={`Remove ${key}`}
            disabled={disabled}
            onClick={() => onChange(Object.fromEntries(entries.filter((_, at) => at !== index)))}
          >
            ×
          </button>
        </div>
      ))}
      <button
        type="button"
        className="setup-secondary-button"
        disabled={disabled}
        onClick={() => onChange({ ...value, "": "" })}
      >
        Add
      </button>
    </div>
  );
}
