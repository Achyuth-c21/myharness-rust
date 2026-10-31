/**
 * Language dialog (UI-SCREEN-20; docs/screens/account-menu-and-language.md;
 * docs/23 §3.3 item 15, capture x30; IMP-47).
 *
 * The observed modal: "Choose your language", a grid of the 11 offered
 * locales, each showing the autonym over the English label, a checkmark on
 * the active one, no Apply button — selection applies immediately and
 * persists. The rebuild stores the choice renderer-side (`ui.locale`) until
 * the bridge/identity phase consumes it; until localization lands the UI
 * stays English, and the dialog says so rather than pretending the switch
 * took effect (docs/14 §9 — an unavailable capability is stated, never
 * emulated).
 */
import { useEffect, useRef, useState } from "react";
import { useFocusTrap } from "./focusTrap";
import { loadLocale, saveLocale } from "../settings/prefs";

/** The observed 11-locale catalog, in the dialog's order (docs/23 §3.3 item 15). */
const LANGUAGES: { code: string; native: string; english: string }[] = [
  { code: "en-US", native: "English", english: "English (United States)" },
  { code: "fr-FR", native: "Français", english: "French (France)" },
  { code: "de-DE", native: "Deutsch", english: "German (Germany)" },
  { code: "hi-IN", native: "हिन्दी", english: "Hindi (India)" },
  { code: "id-ID", native: "Indonesia", english: "Indonesian" },
  { code: "it-IT", native: "Italiano", english: "Italian (Italy)" },
  { code: "ja-JP", native: "日本語", english: "Japanese (Japan)" },
  { code: "ko-KR", native: "한국어", english: "Korean (South Korea)" },
  { code: "pt-BR", native: "Português (Brasil)", english: "Portuguese (Brazil)" },
  { code: "es-419", native: "Español (LatAm)", english: "Spanish (Latin America)" },
  { code: "es-ES", native: "Español (España)", english: "Spanish (Spain)" },
];

function CheckGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m3.5 8.5 3 3 6-6.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

interface Props {
  onClose: () => void;
}

export function LanguageDialog({ onClose }: Props) {
  const [selected, setSelected] = useState<string>(loadLocale);
  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef, true);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="trust-scrim"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="language-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Choose your language"
        ref={dialogRef}
      >
        <div className="language-head">
          <h2 className="trust-title">Choose your language</h2>
          <button type="button" className="notice-close" aria-label="Close language dialog" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="language-grid" role="listbox" aria-label="Languages">
          {LANGUAGES.map((language) => (
            <button
              key={language.code}
              type="button"
              role="option"
              aria-selected={selected === language.code}
              lang={language.code}
              className={`language-option${selected === language.code ? " active" : ""}`}
              onClick={() => {
                setSelected(language.code);
                saveLocale(language.code);
              }}
            >
              <span className="language-native">{language.native}</span>
              <span className="language-english">{language.english}</span>
              {selected === language.code ? (
                <span className="language-check">
                  <CheckGlyph />
                </span>
              ) : null}
            </button>
          ))}
        </div>
        <p className="settings-note">
          The choice is stored on this device. The interface applies it when
          localization lands; until then the interface stays English.
        </p>
      </div>
    </div>
  );
}
