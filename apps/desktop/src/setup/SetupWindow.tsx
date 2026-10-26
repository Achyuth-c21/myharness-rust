/**
 * The third-party inference setup window (docs/screens/setup-desktop-3p.md;
 * docs/09 §2.1–§2.10).
 *
 * A separate window, not a Settings tab and not a modal. The layout follows the
 * captured arrangement: a header with the title, the profile selector and
 * **Export**; a left navigation with a search box and nine entries; a scrolling
 * content pane; and a fixed footer with **Apply Changes**.
 *
 * The draft model is the shipped one: edits live in memory and reach the disk
 * only on Apply, which is the only save action (docs/09 §3.2;
 * docs/screens/setup-desktop-3p.md "Apply is the only save action"). Discard
 * restores the applied values.
 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { Field } from "./fields";
import {
  discoverModels,
  discoverLocalModels,
  discoverOpenAiModels,
  discoverOpenAiResponsesModels,
  testConnection,
  setup,
  type ConfigHealth,
  type DiscoveryOutcome,
  type FamilyDescriptor,
  type EgressAllowlist,
  type ExportBundle,
  type FieldDef,
  type FormIssue,
  type ProfileDocument,
  type ProfileSummary,
  type Schema,
  type SetupTab,
  type ProviderKind,
} from "./client";
import "./setup.css";

/** The nine tabs, in the captured navigation order. */
const TABS: { id: SetupTab; label: string }[] = [
  { id: "connection", label: "Connection" },
  { id: "capabilities", label: "Capabilities" },
  { id: "connectors", label: "Connectors" },
  { id: "telemetry", label: "Telemetry & updates" },
  { id: "limits", label: "Limits" },
  { id: "appearance", label: "Appearance" },
  { id: "plugins", label: "Plugins" },
  { id: "egress", label: "Egress" },
  { id: "source", label: "Source" },
];

/** The human label of an egress group (docs/09 §2.9 names the groups). */
const EGRESS_GROUP_LABELS: Record<string, string> = {
  "core-downloads": "Core downloads",
  "auto-updates": "Auto-updates",
  "essential-telemetry": "Essential telemetry",
  "nonessential-services": "Nonessential services",
  "provider-endpoint": "Provider endpoint",
  "plugin-marketplaces": "Plugin marketplaces",
  "managed-mcp-servers": "Managed MCP servers",
  "user-mcp-runtime": "User MCP & Python runtime",
  "ssh-identity-endpoints": "SSH & identity endpoints",
};

/**
 * The family picker's options come from the BRIDGE's family catalog
 * (`TEST-EXT-007`'s data half; crates/config forms::family_catalog) — the
 * UI never hard-codes a family id. Until the catalog arrives the picker is
 * empty; a failed catalog load is a banner, not a fake list.
 */


/** The nine profile sections (docs/18 §10). */
const SECTIONS = [
  "connection",
  "capabilities",
  "connectors",
  "telemetry",
  "limits",
  "appearance",
  "plugins",
  "egress",
  "source",
] as const;

type Section = (typeof SECTIONS)[number];

/** The merged view's per-field lock state (DATA-CONFIG-04). */
type MergedFields = Record<string, { lock: "writable" | "readOnly"; source: string }>;

function sectionOf(tab: SetupTab): Section {
  return tab as Section;
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

export default function SetupWindow() {
  const [phase, setPhase] = useState<"loading" | "ready" | "applying" | "error">("loading");
  const [banner, setBanner] = useState<string | null>(null);
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [baseline, setBaseline] = useState<ProfileDocument | null>(null);
  const [draft, setDraft] = useState<ProfileDocument | null>(null);
  const [pendingSecrets, setPendingSecrets] = useState<Record<string, string>>({});
  const [issues, setIssues] = useState<FormIssue[]>([]);
  const [tab, setTab] = useState<SetupTab>("connection");
  const [query, setQuery] = useState("");
  const [schemas, setSchemas] = useState<Partial<Record<SetupTab, Schema>>>({});
  const [merged, setMerged] = useState<MergedFields>({});
  const [health, setHealth] = useState<ConfigHealth | null>(null);
  const [egress, setEgress] = useState<EgressAllowlist | null>(null);
  const [exportBundle, setExportBundle] = useState<ExportBundle | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [discovering, setDiscovering] = useState(false);
  const [discoveryNote, setDiscoveryNote] = useState<string | null>(null);
  const [probing, setProbing] = useState(false);
  const [probeNote, setProbeNote] = useState<string | null>(null);
  const [familyCatalog, setFamilyCatalog] = useState<FamilyDescriptor[]>([]);

  const loadProfiles = useCallback(async () => {
    const response = await setup({ type: "listProfiles" });
    if (response.type === "error") {
      setBanner(response.error.message);
      return [];
    }
    if (response.type !== "profiles") return [];
    setProfiles(response.profiles);
    return response.profiles;
  }, []);

  const loadProfile = useCallback(async (id: string) => {
    const response = await setup({ type: "readProfile", id });
    if (response.type === "error") {
      setBanner(response.error.message);
      return;
    }
    if (response.type !== "profile") return;
    setBaseline(clone(response.document));
    setDraft(clone(response.document));
    setPendingSecrets({});
    setIssues([]);
  }, []);

  const loadSchemas = useCallback(async (provider: ProviderKind | null) => {
    const tabs = await Promise.all(
      TABS.map(async (entry) => {
        const response = await setup({ type: "tabSchema", tab: entry.id });
        return [entry.id, response.type === "schema" ? response.schema : null] as const;
      }),
    );
    const next: Partial<Record<SetupTab, Schema>> = {};
    for (const [id, schema] of tabs) {
      if (schema) next[id] = schema;
    }
    if (provider) {
      const response = await setup({
        type: "connectionSchema",
        provider,
      });
      if (response.type === "schema") next.connection = response.schema;
    }
    setSchemas(next);
  }, []);

  const loadFamilyCatalog = useCallback(async () => {
    const response = await setup({ type: "familyCatalog" });
    if (response.type === "familyCatalog") setFamilyCatalog(response.families);
  }, []);

  const loadMerged = useCallback(async () => {
    const response = await setup({ type: "merged" });
    if (response.type !== "merged") return;
    const fields = (response.merged.fields ?? {}) as Record<
      string,
      { lock: "writable" | "readOnly"; source: string }
    >;
    setMerged(fields);
  }, []);

  // Boot: profiles, then the applied one, then the schemas and the health.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const list = await loadProfiles();
      if (cancelled) return;
      const applied = list.find((profile) => profile.applied) ?? list[0];
      if (!applied) {
        setPhase("ready");
        setBanner("No configuration profile exists yet. Create one to configure inference.");
        return;
      }
      setSelectedId(applied.id);
      await loadProfile(applied.id);
      await loadFamilyCatalog();
      await loadSchemas(applied.provider);
      await loadMerged();
      const healthResponse = await setup({ type: "health" });
      if (healthResponse.type === "health") setHealth(healthResponse.health);
      if (!cancelled) setPhase("ready");
    })();
    return () => {
      cancelled = true;
    };
  }, [loadProfiles, loadProfile, loadSchemas, loadMerged, loadFamilyCatalog]);

  // The Egress tab is derived, so it is fetched when the tab is opened.
  useEffect(() => {
    if (tab !== "egress") return;
    (async () => {
      const response = await setup({ type: "egress" });
      if (response.type === "egress") setEgress(response.allowlist);
    })();
  }, [tab]);

  const dirty = useMemo(() => {
    if (!baseline || !draft) return false;
    // A typed credential is a change too: it never lands in the document, so
    // without this the draft would read as clean and Apply would stay disabled
    // with the credential unsavable.
    if (Object.keys(pendingSecrets).length > 0) return true;
    return JSON.stringify(baseline) !== JSON.stringify(draft);
  }, [baseline, draft, pendingSecrets]);

  const hasError = issues.some((issue) => issue.severity === "error");

  // The probe runs only where the catalog says a native adapter exists
  // (`docs/13` Phase 3; TEST-EXT-007): the catalog is the registration
  // record — pending families keep the button disabled rather than faked.
  const probeSupported =
    familyCatalog.find((entry) => entry.id === draft?.provider)?.probeSupported ?? false;
  const familyDescriptor = (provider: ProviderKind | null | undefined) =>
    familyCatalog.find((entry) => entry.id === provider) ?? null;

  const section = draft ? ((draft[sectionOf(tab)] as Record<string, unknown>) ?? {}) : {};
  const schema = schemas[tab];
  const visibleFields: FieldDef[] = useMemo(() => {
    if (!schema) return [];
    if (tab !== "connection") return schema.fields;
    return schema.fields;
  }, [schema, tab]);

  const filteredTabs = TABS.filter((entry) =>
    entry.label.toLowerCase().includes(query.trim().toLowerCase()),
  );

  const setValue = (key: string, value: unknown) => {
    setDraft((current) => {
      if (!current) return current;
      const next = clone(current);
      const target = (next[sectionOf(tab)] as Record<string, unknown>) ?? {};
      target[key] = value;
      (next as Record<string, unknown>)[sectionOf(tab)] = target;
      return next;
    });
  };

  const setSecret = (key: string, value: string) => {
    const path = `${sectionOf(tab)}/${key}`;
    setPendingSecrets((current) => ({ ...current, [path]: value }));
  };

  /**
   * The family picker (docs/13 §C open model layer; owner decision recorded
   * 2026-10-23, docs/14 §9): the draft's family decides the connection
   * schema, the probe lane, and the applied wire. Switching it keeps the
   * shared fields (model list, idle window, custom headers, credential-kind
   * selection — the resolver re-checks the kind against the new family's
   * offered set) and resets the family-specific ones, so nothing stale
   * leaks across a wire. Nothing applies until Apply.
   */
  const changeFamily = async (provider: string) => {
    if (!draft) return;
    const next = clone(draft) as ProfileDocument;
    next.provider = provider as ProviderKind;
    const shared = new Set([
      "inferenceModels",
      "inferenceStreamIdleTimeoutSec",
      "inferenceCustomHeaders",
      "inferenceCredentialKind",
      "inferenceCredentialHelper",
      "inferenceCredentialHelperArgs",
    ]);
    next.connection = Object.fromEntries(
      Object.entries(next.connection ?? {}).filter(([key]) => shared.has(key)),
    );
    setDraft(next);
    setIssues([]);
    await loadSchemas(next.provider ?? null);
  };

  /**
   * Discover the endpoint's model list and offer it as the explicit model
   * list. Family-aware (`docs/12` §3.1 the Gateway family; `docs/13` §C the
   * open model layer): the gateway family walks its cursor-paginated
   * `/v1/models`, the OpenAI-compatible families read the endpoint's own
   * `/models` listing — the `custom` one keyless when no credential
   * exists (`docs/14` §9 owner decision). The discovered ids are
   * de-duplicated by the core; the first entry is the default
   * (`docs/12` §3.2).
   */
  const runDiscovery = async () => {
    const provider = draft?.provider ?? null;
    const descriptor = familyDescriptor(provider);
    const headers = (section["inferenceCustomHeaders"] ?? {}) as Record<string, string>;
    setDiscovering(true);
    setDiscoveryNote(null);
    // The discovery dispatch keys off the CATALOG's style + keys, not on
    // family ids (`TEST-EXT-007`): the renderer asks the bridge what a
    // family's base URL and credential slot are called.
    const style = descriptor?.discovery ?? "none";
    const fieldString = (key: string | null): string => {
      if (!key) return "";
      const value = section[key];
      return typeof value === "string" ? value : "";
    };
    if (style === "gateway") {
      const baseKey = descriptor?.baseKey ?? "";
      const baseUrl = fieldString(baseKey);
      if (!baseUrl.trim()) {
        setDiscovering(false);
        setDiscoveryNote("A gateway base URL is required before models can be discovered.");
        return;
      }
      const response = await discoverModels({
        baseUrl,
        apiKey: pendingSecrets["connection/inferenceGatewayApiKey"],
        authScheme: section["inferenceGatewayAuthScheme"] === "x-api-key" ? "x-api-key" : "bearer",
        customHeaders: headers,
      });
      setDiscovering(false);
      if ("error" in response) {
        setDiscoveryNote(response.error.message);
        return;
      }
      offerDiscoveredModels(response);
      return;
    }
    if (style === "openai-compat" && (provider === "openai" || provider === "custom")) {
      const baseUrl = fieldString(descriptor?.baseKey ?? null);
      // The custom family's base URL is required (the catalog says so);
      // the openai family's is optional (empty = the first-party host).
      if (descriptor?.baseRequired && !baseUrl.trim()) {
        setDiscovering(false);
        setDiscoveryNote("A base URL is required before models can be discovered.");
        return;
      }
      const response = await discoverOpenAiModels({
        kind: provider === "custom" ? "custom" : "openai",
        baseUrl,
        apiKey: pendingSecrets[`connection/${descriptor?.credentialSlot ?? ""}`],
        customHeaders: headers,
      });
      setDiscovering(false);
      if ("error" in response) {
        setDiscoveryNote(response.error.message);
        return;
      }
      offerDiscoveredModels(response);
      return;
    }
    if (style === "responses") {
      // The Responses wall's listing (Phase 8; docs/14 §9 item 71): the
      // base URL is optional (empty = the first-party host) and the
      // credential is the family's required static key.
      const response = await discoverOpenAiResponsesModels({
        baseUrl: fieldString(descriptor?.baseKey ?? null),
        apiKey: pendingSecrets[`connection/${descriptor?.credentialSlot ?? ""}`],
        customHeaders: headers,
      });
      setDiscovering(false);
      if ("error" in response) {
        setDiscoveryNote(response.error.message);
        return;
      }
      offerDiscoveredModels(response);
      return;
    }
    if (style === "local") {
      // The local wall's listings (Phase 8; docs/14 §9): the base URL is
      // optional (empty = the family's recorded Ollama default) and the
      // credential is optional (keyless is the documented default).
      const response = await discoverLocalModels({
        baseUrl: fieldString(descriptor?.baseKey ?? null),
        apiKey: pendingSecrets[`connection/${descriptor?.credentialSlot ?? ""}`],
        customHeaders: headers,
      });
      setDiscovering(false);
      if ("error" in response) {
        setDiscoveryNote(response.error.message);
        return;
      }
      offerDiscoveredModels(response);
      return;
    }
    setDiscovering(false);
    setDiscoveryNote("Model discovery is not available for this provider family.");
  };

  const offerDiscoveredModels = (response: DiscoveryOutcome) => {
    const ids = response.models.map((model) => model.id);
    if (!ids.length) {
      setDiscoveryNote("The provider advertised no models.");
      return;
    }
    setValue("inferenceModels", ids);
    setDiscoveryNote(
      response.partial
        ? `Discovered ${ids.length} models; the list is partial${response.warning ? ` (${response.warning})` : ""}.`
        : `Discovered ${ids.length} models.`,
    );
  };

  /**
   * The *Test connection*: a real one-token inference through the native
   * adapter of the draft's family (docs/13 Phase 3 — gateway, the Claude API
   * family, then Bedrock). The typed credential rides the
   * secrets map exactly like the apply path; nothing here persists. The
   * adapter owns every header the family's contract defines.
   */
  const runProbe = async () => {
    if (!draft) return;
    setProbing(true);
    setProbeNote(null);
    const response = await testConnection(draft, pendingSecrets);
    setProbing(false);
    if (response.type === "error") {
      setProbeNote(response.error.message);
      return;
    }
    if (response.type !== "probe") return;
    const { probe } = response;
    // The note names the family whose adapter ran this probe; the verdict
    // itself is the shared typed shape.
    const familyLabel =
      draft.provider === "anthropic"
        ? "Claude API"
        : draft.provider === "bedrock"
          ? "Bedrock"
          : draft.provider === "gateway"
            ? "Gateway"
            : draft.provider === "openai"
              ? "OpenAI"
              : draft.provider === "custom"
                ? "OpenAI-compatible endpoint"
                : draft.provider === "openai-responses"
                  ? "OpenAI Responses"
                  : draft.provider === "local"
                    ? "Local server"
                    : /* family-agnostic for anything else */ "provider";
    setProbeNote(
      probe.ok
        ? `Connected. The ${familyLabel} adapter ran ${probe.model} (${probe.latencyMs} ms, ` +
          `${probe.inputTokens} in / ${probe.outputTokens} out` +
          `${probe.stopReason ? `, stop ${probe.stopReason}` : ""}).`
        : `The ${familyLabel} adapter answered but the probe did not complete.`,
    );
  };

  const discard = () => {
    if (!baseline) return;
    setDraft(clone(baseline));
    setPendingSecrets({});
    setIssues([]);
  };

  const apply = async () => {
    if (!draft || !selectedId) return;
    setPhase("applying");
    setBanner(null);
    // Validate before writing: the shipped form blocks Apply while invalid
    // (docs/09 §2.2, "Required fields marked `*`; Apply blocks/shows errors").
    const validation = await setup({ type: "validate", document: draft });
    if (validation.type === "issues" && validation.issues.some((i) => i.severity === "error")) {
      setIssues(validation.issues);
      setPhase("ready");
      setBanner("The profile has invalid fields. Apply was not attempted.");
      return;
    }
    if (validation.type === "issues") setIssues(validation.issues);
    const write = await setup({
      type: "writeProfile",
      document: draft,
      secrets: pendingSecrets,
    });
    if (write.type === "error") {
      setBanner(write.error.message);
      setPhase("ready");
      return;
    }
    const applied = await setup({ type: "applyProfile", id: selectedId });
    if (applied.type === "error") {
      // A failed apply leaves the previous profile active (docs/18 §10); the draft
      // is kept so the user does not lose their work.
      setBanner(applied.error.message);
      setPhase("ready");
      return;
    }
    await loadProfile(selectedId);
    await loadMerged();
    setPhase("ready");
  };

  const switchProfile = async (id: string) => {
    if (dirty && !window.confirm("Discard the unsaved changes?")) return;
    setSelectedId(id);
    await loadProfile(id);
    const profile = profiles.find((entry) => entry.id === id);
    await loadSchemas(profile?.provider ?? null);
    await loadMerged();
  };

  const runExport = async () => {
    const response = await setup({ type: "export" });
    if (response.type === "export") {
      setExportBundle(response.bundle);
      setExportOpen(true);
    } else if (response.type === "error") {
      setBanner(response.error.message);
    }
  };

  const copyEgress = async () => {
    if (!egress) return;
    const hosts = egress.groups.flatMap((group) => group.hosts);
    try {
      await navigator.clipboard.writeText(hosts.join("\n"));
    } catch {
      setBanner("The host list could not be copied to the clipboard.");
    }
  };

  return (
    <div className="setup-window">
      <header className="setup-header">
        <h1 className="setup-title">Configure third-party inference</h1>
        <div className="setup-header-actions">
          <label className="setup-profile-picker">
            <span className="setup-visually-hidden">Profile</span>
            <select
              className="setup-select"
              value={selectedId ?? ""}
              onChange={(event) => void switchProfile(event.target.value)}
            >
              {profiles.length === 0 ? <option value="">No profile</option> : null}
              {profiles.map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.name}
                  {profile.applied ? " (applied)" : ""}
                </option>
              ))}
            </select>
          </label>
          <button type="button" className="setup-secondary-button" onClick={() => void runExport()}>
            Export
          </button>
        </div>
      </header>

      {banner ? (
        <div className="setup-banner setup-banner-error" role="alert">
          {banner}
        </div>
      ) : null}

      {health && health.status === "tainted" ? (
        <div className="setup-banner setup-banner-warn" role="status">
          The managed configuration source could not be reached
          {health.detail ? `: ${health.detail}` : "."} The last known managed values are still in
          effect.
        </div>
      ) : null}

      <div className="setup-body">
        <nav className="setup-nav" aria-label="Setup sections">
          <input
            className="setup-search"
            type="search"
            placeholder="Search settings"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <ul className="setup-nav-list">
            {filteredTabs.map((entry) => {
              const tabIssues = issues.filter((issue) => issue.field.includes(entry.id));
              return (
                <li key={entry.id}>
                  <button
                    type="button"
                    className={`setup-nav-item${tab === entry.id ? " setup-nav-item-active" : ""}`}
                    aria-current={tab === entry.id ? "page" : undefined}
                    onClick={() => setTab(entry.id)}
                  >
                    <span>{entry.label}</span>
                    {tabIssues.length ? <span className="setup-nav-dot" aria-hidden="true" /> : null}
                  </button>
                </li>
              );
            })}
          </ul>
        </nav>

        <main className="setup-content">
          {phase === "loading" ? <div className="setup-loading">Loading configuration…</div> : null}

          {tab === "egress" ? (
            <EgressTab allowlist={egress} onCopy={() => void copyEgress()} />
          ) : (
            <section className="setup-section" aria-label={schema?.title ?? tab}>
              <div className="setup-section-head">
                <h2 className="setup-section-title">
                  {TABS.find((entry) => entry.id === tab)?.label ?? tab}
                </h2>
                {tab === "connection" ? (
                  <div className="setup-section-actions">
                    {/* A real one-token inference through the native adapter
                        of the selected family (docs/13 Phase 3, slice per
                        adapter: gateway, the Claude API family, then
                        Bedrock; docs/13 §C the open model layer's families).
                        Other families keep the button disabled until their
                        adapters land rather than faked. */}
                    <button
                      type="button"
                      className="setup-secondary-button"
                      disabled={probing || !probeSupported}
                      onClick={() => void runProbe()}
                      title={
                        probeSupported
                          ? undefined
                          : "The probe verifies the gateway, Claude API, Bedrock, and OpenAI-compatible families; other families unlock with their adapters"
                      }
                    >
                      {probing ? "Testing…" : "Test connection"}
                    </button>
                    <button
                      type="button"
                      className="setup-secondary-button"
                      disabled={discovering}
                      onClick={() => void runDiscovery()}
                    >
                      {discovering ? "Discovering…" : "Discover models"}
                    </button>
                  </div>
                ) : null}
              </div>
              {tab === "connection" ? (
                <div className="setup-field">
                  <label className="setup-label" htmlFor="family-picker">
                    <span className="setup-label-text">Provider family</span>
                    <span className="setup-help-text">
                      Decides the wire this profile speaks (docs/12 §1;
                      docs/13 §C open model layer). Mantle and Foundry keep
                      their probe and send locked until their adapters land
                      (docs/13 Phase 3).
                    </span>
                  </label>
                  <div className="setup-control">
                    <select
                      id="family-picker"
                      className="setup-select"
                      value={(draft?.provider as string | undefined) ?? ""}
                      onChange={(event) => void changeFamily(event.target.value)}
                    >
                      {/* Blank stays the unset state, exactly like the schema
                          fields' blank option (docs/09 §3.4). */}
                      <option value=""></option>
                      {familyCatalog.map((family) => (
                        <option key={family.id} value={family.id}>
                          {family.label}
                        </option>
                      ))}
                    </select>
                  </div>
                </div>
              ) : null}
              {discoveryNote ? (
                <p className="setup-note" role="status">
                  {discoveryNote}
                </p>
              ) : null}
              {probeNote ? (
                <p className="setup-note" role="status">
                  {probeNote}
                </p>
              ) : null}
              {visibleFields.map((field) => {
                const locked = merged[`${sectionOf(tab)}/${field.key}`]?.lock === "readOnly";
                return (
                  <Field
                    key={`${sectionOf(tab)}/${field.key}`}
                    field={field}
                    value={section[field.key]}
                    issues={issues}
                    locked={locked}
                    source={merged[`${sectionOf(tab)}/${field.key}`]?.source}
                    onChange={(value) => setValue(field.key, value)}
                    onSecret={(value) => setSecret(field.key, value)}
                  />
                );
              })}
            </section>
          )}
        </main>
      </div>

      <footer className="setup-footer">
        {dirty ? (
          <button type="button" className="setup-secondary-button" onClick={discard}>
            Discard Changes
          </button>
        ) : null}
        <button
          type="button"
          className="setup-primary-button"
          disabled={!dirty || hasError || phase === "applying"}
          onClick={() => void apply()}
        >
          {phase === "applying" ? "Applying…" : "Apply Changes"}
        </button>
      </footer>

      {exportOpen && exportBundle ? (
        <div className="setup-export-scrim">
          <div className="setup-export" role="dialog" aria-modal="true" aria-label="Export bundle">
          <h2>Export bundle</h2>
          <ul>
            {exportBundle.warnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
          <ul>
            {exportBundle.contents.map((entry) => (
              <li key={entry.name}>
                <strong>{entry.name}</strong> — {entry.description}
              </li>
            ))}
          </ul>
          <button type="button" className="setup-secondary-button" onClick={() => setExportOpen(false)}>
            Close
          </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}

/**
 * The Egress tab: a read-only derived allowlist (docs/09 §2.9). "Test
 * connectivity" is a network probe that has not been implemented; it is shown
 * disabled rather than faked.
 */
function EgressTab({
  allowlist,
  onCopy,
}: {
  allowlist: EgressAllowlist | null;
  onCopy: () => void;
}) {
  if (!allowlist) return <div className="setup-loading">Deriving the allowlist…</div>;
  return (
    <section className="setup-section" aria-label="Egress">
      <h2 className="setup-section-title">Egress</h2>
      <p className="setup-note">
        Derived from the active configuration. This list can reveal private infrastructure; it must
        not be published.
      </p>
      <div className="setup-egress-actions">
        <button type="button" className="setup-secondary-button" disabled title="Not implemented yet">
          Test connectivity
        </button>
        <button type="button" className="setup-secondary-button" onClick={onCopy}>
          Copy hostnames
        </button>
        <button
          type="button"
          className="setup-secondary-button"
          onClick={() => {
            const text = allowlist.groups.flatMap((group) => group.hosts).join("\n");
            const blob = new Blob([text], { type: "text/plain" });
            const url = URL.createObjectURL(blob);
            const anchor = document.createElement("a");
            anchor.href = url;
            anchor.download = "egress.txt";
            anchor.click();
            URL.revokeObjectURL(url);
          }}
        >
          Download .txt
        </button>
      </div>
      {allowlist.groups.map((group) => (
        <div className="setup-egress-group" key={group.group}>
          <h3>{EGRESS_GROUP_LABELS[group.group] ?? group.group}</h3>
          {group.hosts.length ? (
            <ul className="setup-egress-hosts">
              {group.hosts.map((host) => (
                <li key={host}>{host}</li>
              ))}
            </ul>
          ) : null}
          {group.hostsUnobserved ? (
            <p className="setup-note">
              Some hosts in this group were never captured; they are not listed rather than
              guessed.
            </p>
          ) : null}
        </div>
      ))}
    </section>
  );
}
