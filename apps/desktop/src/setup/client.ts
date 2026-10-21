/**
 * Bridge client for the `settings.Custom3pSetup` namespace (docs/18 §3.4).
 *
 * The setup window is a separate window (docs/09 §2.1) that reaches the
 * configuration library only through the typed bridge. Nothing here talks to the
 * filesystem: the renderer has no privileged surface of its own (RULES.md §4).
 *
 * Secrets cross this boundary as opaque handles, never as values (docs/18 §10).
 * A credential the user types travels on the request; the response carries the
 * handle, never the value.
 */
import { invoke } from "@tauri-apps/api/core";

/** The nine setup-window tabs (docs/09 §2.2–§2.10). */
export type SetupTab =
  | "connection"
  | "capabilities"
  | "connectors"
  | "telemetry"
  | "limits"
  | "appearance"
  | "plugins"
  | "egress"
  | "source";

/** The provider families (docs/12 §1). */
export type ProviderKind =
  | "anthropic"
  | "bedrock"
  | "mantle"
  | "vertex"
  | "foundry"
  | "gateway";

/** A profile as the renderer sees it — never a document. */
export interface ProfileSummary {
  id: string;
  name: string;
  provider: ProviderKind | null;
  applied: boolean;
}

/** A validation issue on a field (docs/09 §2.2, "Validation and error states"). */
export interface FormIssue {
  field: string;
  severity: "error" | "warning";
  message: string;
}

/** The control vocabulary of a form field. */
export type FieldKind =
  | "text"
  | "url"
  | "secret"
  | "select"
  | "number"
  | "switch"
  | "tag-list"
  | "key-value";

/** One form field (crates/config::forms::FieldDef). */
export interface FieldDef {
  key: string;
  label: string;
  kind: FieldKind;
  group: string;
  required?: boolean;
  requiredWhen?: string[];
  options?: string[];
  help?: string;
}

/** A form schema for one tab or provider. */
export interface Schema {
  tab?: SetupTab;
  title?: string;
  provider?: ProviderKind;
  fields: FieldDef[];
  readOnly?: boolean;
}

/** A profile document: nine sections of flat keys (docs/18 §10). */
export type ProfileDocument = Record<string, unknown> & {
  schemaVersion: number;
  id?: string;
  name?: string;
  provider?: ProviderKind;
  connection?: Record<string, unknown>;
  capabilities?: Record<string, unknown>;
  connectors?: Record<string, unknown>;
  telemetry?: Record<string, unknown>;
  limits?: Record<string, unknown>;
  appearance?: Record<string, unknown>;
  plugins?: Record<string, unknown>;
  egress?: Record<string, unknown>;
  source?: Record<string, unknown>;
};

/** The managed-configuration health (DATA-CONFIG-09). */
export interface ConfigHealth {
  status: "healthy" | "tainted";
  localFallbackAllowed: boolean;
  overlayPresent: boolean;
  lastChecked: number | null;
  detail: string | null;
}

/** One group of the derived egress allowlist (docs/09 §2.9). */
export interface EgressGroup {
  group: string;
  hosts: string[];
  hostsUnobserved: boolean;
}

export interface EgressAllowlist {
  groups: EgressGroup[];
}

/** The export bundle (docs/09 §4). */
export interface ExportBundle {
  contents: { name: string; description: string; mayContainPrivateHosts: boolean }[];
  files: Record<string, unknown>;
  warnings: string[];
  consentRequired: boolean;
}

export interface BridgeError {
  code: string;
  message: string;
  retryable: boolean;
}

export type SetupResponse =
  | { type: "profiles"; profiles: ProfileSummary[] }
  | { type: "profile"; document: ProfileDocument }
  | { type: "merged"; merged: Record<string, unknown> }
  | { type: "health"; health: ConfigHealth }
  | { type: "export"; bundle: ExportBundle }
  | { type: "egress"; allowlist: EgressAllowlist }
  | { type: "schema"; schema: Schema }
  | { type: "issues"; issues: FormIssue[] }
  | { type: "error"; error: BridgeError };

export type SetupCommand =
  | { type: "listProfiles" }
  | { type: "readProfile"; id: string }
  | { type: "writeProfile"; document: ProfileDocument; secrets: Record<string, string> }
  | { type: "applyProfile"; id: string }
  | { type: "duplicateProfile"; id: string; name: string }
  | { type: "renameProfile"; id: string; name: string }
  | { type: "deleteProfile"; id: string }
  | { type: "merged" }
  | { type: "health" }
  | { type: "export" }
  | { type: "egress" }
  | { type: "tabSchema"; tab: SetupTab }
  | { type: "connectionSchema"; provider: ProviderKind }
  | { type: "validate"; document: ProfileDocument };

/**
 * Send one command. The bridge rejects an unknown command and validates every
 * argument, so a failure comes back as an `error` response rather than an
 * exception (docs/18 §3.2: "reject unknown/invalid calls").
 */
export async function setup(command: SetupCommand): Promise<SetupResponse> {
  try {
    return await invoke<SetupResponse>("setup_command", { command });
  } catch (error) {
    return {
      type: "error",
      error: {
        code: "BRIDGE_UNAVAILABLE",
        message: typeof error === "string" ? error : "The configuration service is not available.",
        retryable: true,
      },
    };
  }
}

/** Open the separate setup window (docs/09 §2.1). */
export async function openSetupWindow(): Promise<void> {
  await invoke("open_setup_window");
}
