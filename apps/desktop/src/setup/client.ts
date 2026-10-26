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

/** The provider families (docs/12 §1; the rebuilt set excludes the shipped Vertex
 * family — removed from rebuild scope, owner decision 2026-10-23, docs/04 §3.1).
 * The open model layer adds the two OpenAI-compatible families (owner decision
 * recorded 2026-10-23, docs/14 §9): first-party `openai` and the user-defined
 * `custom` OpenAI-compatible endpoint (base URL required, key optional). Phase 8
 * adds the Responses extension (`docs/13` Phase 8): first-party `openai-responses`
 * over its own native wall — a static key is required, this wire has no keyless
 * variant (docs/14 §9 item 71). Phase 8 also adds the local extension: `local`
 * — a locally running Ollama / llama.cpp server over its documented compat
 * surface, keyless by default, loopback-only by construction (docs/14 §9). */
export type ProviderKind =
  | "anthropic"
  | "bedrock"
  | "mantle"
  | "foundry"
  | "gateway"
  | "openai"
  | "custom"
  | "openai-responses"
  | "local";

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
  /** A select's options: the wire value stored in the profile, and its label. */
  options?: { value: string; label: string }[];
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
  | { type: "error"; error: BridgeError }
  | { type: "probe"; probe: ProbeStatus };

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
  | { type: "validate"; document: ProfileDocument }
  | {
      type: "testConnection";
      document: ProfileDocument;
      secrets: Record<string, string>;
    };

/**
 * The *Test connection* verdict: a real probe inference (docs/13 Phase 3).
 * `ok` means the provider answered for the requested model; exact usage,
 * latency, and the native stop reason ride along. The shape is shared by
 * every probe-capable family, so the setup window stays family-agnostic.
 */
export interface ProbeStatus {
  ok: boolean;
  /** The model the provider says it ran (== the requested id). */
  model: string;
  /** The native stop reason, preserved. */
  stopReason: string | null;
  /** Exact tokens as the provider reported. */
  inputTokens: number;
  outputTokens: number;
  /** Wall time around the native request, in milliseconds. */
  latencyMs: number;
}

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

/**
 * One discovered model (docs/12 §3.1).
 */
export interface DiscoveredModel {
  id: string;
  raw: unknown;
}

/** The outcome of a discovery run (docs/12 §3.1). */
export interface DiscoveryOutcome {
  models: DiscoveredModel[];
  pages: number;
  /** True when a later page failed and the rows already read were kept. */
  partial: boolean;
  warning: string | null;
}

/**
 * Run model discovery against an Anthropic-compatible gateway
 * (`settings.Custom3pHelperRun`, docs/18 §3.4; docs/12 §3.1).
 *
 * The credential travels on the request — the user typed it — and is never
 * stored by the probe.
 */
export async function discoverModels(request: {
  baseUrl: string;
  apiKey?: string;
  authScheme?: "bearer" | "x-api-key";
  customHeaders?: Record<string, string>;
}): Promise<DiscoveryOutcome | { error: BridgeError }> {
  try {
    return await invoke<DiscoveryOutcome>("setup_discover_models", {
      request: {
        baseUrl: request.baseUrl,
        apiKey: request.apiKey ?? null,
        authScheme: request.authScheme ?? "bearer",
        customHeaders: Object.entries(request.customHeaders ?? {}),
      },
    });
  } catch (error) {
    return {
      error: {
        code: "PROVIDER_UNREACHABLE",
        message:
          typeof error === "string" ? error : "The gateway could not be reached.",
        retryable: true,
      },
    };
  }
}

/**
 * Run model discovery against an OpenAI-compatible endpoint — the open
 * model layer's families (`docs/13` §C): first-party `openai` (base URL
 * optional, defaulting to the first-party host) and the user-defined
 * `custom` endpoint, whose credential is optional (`docs/14` §9 owner
 * decision).
 *
 * The credential travels on the request — the user typed it — and is never
 * stored by the probe.
 */
export async function discoverOpenAiModels(request: {
  kind: "openai" | "custom";
  baseUrl: string;
  apiKey?: string;
  customHeaders?: Record<string, string>;
}): Promise<DiscoveryOutcome | { error: BridgeError }> {
  try {
    return await invoke<DiscoveryOutcome>("setup_discover_openai_models", {
      request: {
        kind: request.kind,
        baseUrl: request.baseUrl,
        apiKey: request.apiKey ?? null,
        customHeaders: Object.entries(request.customHeaders ?? {}),
      },
    });
  } catch (error) {
    return {
      error: {
        code: "PROVIDER_UNREACHABLE",
        message:
          typeof error === "string" ? error : "The provider could not be reached.",
        retryable: true,
      },
    };
  }
}

/**
 * Run model discovery against the local wall's listings — Phase 8's local
 * extension (`docs/13` Phase 8; docs/14 §9): `/api/tags` first (Ollama),
 * `/v1/models` on a 404 (the llama.cpp posture). The destination gate is
 * the wall's own — a non-loopback base refuses here too. The credential is
 * optional (keyless is the documented default) and is never stored by the
 * probe.
 */
export async function discoverLocalModels(request: {
  baseUrl: string;
  apiKey?: string;
  customHeaders?: Record<string, string>;
}): Promise<DiscoveryOutcome | { error: BridgeError }> {
  try {
    return await invoke<DiscoveryOutcome>("setup_discover_local_models", {
      request: {
        baseUrl: request.baseUrl,
        apiKey: request.apiKey ?? null,
        customHeaders: Object.entries(request.customHeaders ?? {}),
      },
    });
  } catch (error) {
    return {
      error: {
        code: "PROVIDER_UNREACHABLE",
        message:
          typeof error === "string" ? error : "The local server could not be reached.",
        retryable: true,
      },
    };
  }
}

/**
 * Run model discovery against the Responses wall's listing — Phase 8's
 * extension (`docs/13` Phase 8; the readings are docs/14 §9 item 71):
 * first-party OpenAI only, base URL optional (empty = the first-party
 * host), and the credential the family's rule requires. The typed
 * credential travels on the request — the user typed it — and is never
 * stored by the probe.
 */
export async function discoverOpenAiResponsesModels(request: {
  baseUrl: string;
  apiKey?: string;
  customHeaders?: Record<string, string>;
}): Promise<DiscoveryOutcome | { error: BridgeError }> {
  try {
    return await invoke<DiscoveryOutcome>("setup_discover_openai_responses_models", {
      request: {
        baseUrl: request.baseUrl,
        apiKey: request.apiKey ?? null,
        customHeaders: Object.entries(request.customHeaders ?? {}),
      },
    });
  } catch (error) {
    return {
      error: {
        code: "PROVIDER_UNREACHABLE",
        message:
          typeof error === "string" ? error : "The provider could not be reached.",
        retryable: true,
      },
    };
  }
}

/**
 * Run the Connection tab's *Test connection* (docs/13 Phase 3): a real
 * one-token inference against the gateway through the native adapter. The
 * typed credential rides `secrets` exactly like the write path; the draft is
 * never persisted, and a stored credential is never substituted for a typed
 * one or vice versa.
 */
export function testConnection(
  document: ProfileDocument,
  secrets: Record<string, string>,
): Promise<SetupResponse> {
  return setup({ type: "testConnection", document, secrets });
}

/** Open the separate setup window (docs/09 §2.1). */
export async function openSetupWindow(): Promise<void> {
  await invoke("open_setup_window");
}
