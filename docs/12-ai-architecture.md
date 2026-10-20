# 12 — AI, Provider, Model, and Tool Architecture

This is the core architectural specification for inference. It separates:

1. **Shipped 3P behavior** — provider-native clients and an Anthropic-compatible gateway contract.
2. **The current local adapter** — a noncompliant in-process translation shim that must not be rebuilt.
3. **The extension architecture** — how to add providers, SDKs, and wire protocols without violating
   the standing constraints.

The controlling constraints are:

- No routing, translation, or gateway layer between the app and the selected provider.
- Provider-native integration using the provider’s official SDK/protocol where possible.
- No silent model substitution.
- The current in-process adapter is **noncompliant** and must never be described as “native.”

## 1. Shipped provider model

The setup window supports six provider families:

| Provider ID | UI label | Default host shape | Native protocol | Credential kinds |
|---|---|---|---|---|
| `anthropic` | Claude API | `api.anthropic.com` | Anthropic Messages | static key, interactive OAuth |
| `bedrock` | Bedrock | `bedrock-runtime[ -fips].<region>.amazonaws.com` | Anthropic Messages via Bedrock Runtime | static bearer, profile/SSO, web identity, helper, vendor profile |
| `mantle` | Bedrock Mantle | `bedrock-mantle.<region>.api.aws` | Bedrock Mantle | static/helper/vendor |
| `vertex` | Vertex AI | `<region>-aiplatform.googleapis.com` (or `aiplatform.googleapis.com`, regional rep host) | Anthropic Messages via Vertex | Google interactive/Workforce/helper/vendor |
| `foundry` | Foundry | `<resource>.services.ai.azure.com` | Anthropic Messages via Azure AI Foundry | API key, interactive, helper |
| `gateway` | Gateway | configured URL | Anthropic-compatible `/v1/messages` + `/v1/models` | static key, interactive, helper |

Control-plane hosts for Bedrock include `bedrock.<region>.amazonaws.com`,
`sts.amazonaws.com`, `sts.<region>.amazonaws.com`, `portal.sso.<region>.amazonaws.com`, and
`oidc.<region>.amazonaws.com`. The egress page derives these from the active provider/region.

**Verified** from provider tables, host-construction functions, and credential enums. The exact
request path for a Bedrock/Vertex/Foundry call is **strong inference** from the provider families and
the app’s Anthropic-compatible payloads; the bundled implementation is minified and no source maps
exist.

The shipped design is **provider-native** in the relevant sense: each provider has an explicit
endpoint/auth/model policy, and the Gateway is a first-class provider whose contract is Anthropic
Messages. There is no universal “translate Anthropic to OpenAI” layer in the pristine app.

## 2. Provider capabilities and validation

### 2.1 Credential rules

- “When set, only that source is used (no fallback).” This is explicit UI copy and must be preserved.
- Credential kinds: `static`, `helper-script`, `interactive`, `vendor-profile`, `workforce`,
  `external-idp`.
- Helper scripts support args, timeout, TTL, and silent refresh.
- Interactive OAuth/device-code flows cache tokens in main; tokens never enter renderer storage.
- `sk-ant-oat...` tokens are recognized as Anthropic OAuth access tokens and receive the OAuth beta
  header.
- Bedrock/Vertex use cloud identity and optional base URLs/VPC endpoints.
- Foundry accepts a resource name/base URL and deployment name; the deployment must resolve to an
  Anthropic model.

### 2.2 Shipped model-ID policy

The pristine app deliberately restricts models. The predicate is equivalent to:

```text
reject if the lowercased id matches the non-Anthropic family list
    (ark-code, astron, command-r, deepseek, doubao, gemini, gemma, glm, gpt,
     grok, hermes, hy3, kimi, lfm, ling, llama, longcat, mimo, minimax,
     mistral, mixtral, moonshot, nemotron, openai, phi-, qianfan, qwen,
     tc-code, unic, yi-, stepfun, step-3, seed-, bytedance, hunyuan,
     granite, amazon.nova, nova-, devstral, ministral, ernie, codex,
     arcee, trinity, abab, phi\d, k2., m2., jamba, arctic, solar,
     mercury, zamba, kat-coder, ds-, dpsk/)
otherwise accept if it matches an Anthropic family tier
    (sonnet, opus, haiku, fable, mythos), optionally with a numeric version,
    or contains claude/anthropic.
```

Provider-specific validators:

| Provider | Valid model ID |
|---|---|
| Bedrock | `anthropic.*` vendor prefix or an `arn:aws...:bedrock:` inference-profile/provisioned-model ARN; ARN/profile IDs produce an “underlying model not verified” warning |
| Vertex | starts with `claude-`; no `/`, `\`, `?`, `#`, `%`, whitespace |
| Foundry | deployment name passes the Anthropic model predicate |
| Gateway | route/id passes the Anthropic model predicate |
| Claude API | Anthropic model predicate |

This policy is a **shipped product constraint**, not a bug. The local adapter changes
`function So(e){return!0}` to bypass it. A compliant extension for non-Anthropic providers must
introduce a new provider family and its own policy — never globally weaken the Anthropic policy.

### 2.3 Capability metadata

Model capability metadata comes from:

- provider-native model listing (where supported);
- the signed published model catalog;
- static `inferenceModels` entries;
- gateway `/v1/models` metadata;
- explicit configuration overrides.

Shipped evidence includes model fields `capabilities`, `thinking`, `fast_mode`, `hard_limit`,
`quick_select`, `runtime`, and `offered_on` in the catalog, plus `anthropic_family_tier`,
`supports_1m`, `prefer_1m`, and `max_effort_level` for gateway models. The exact set of capability
flags is provider/catalog-dependent and was not exhaustively enumerated.

**Do not assume a capability because a model family usually has it.** A rebuild must treat missing
capability data as unsupported and show why.

## 3. Model discovery and catalog

### 3.1 Gateway discovery

`GET <gateway-base>/v1/models?limit=1000`

- Headers: provider auth, `anthropic-version: 2023-06-01`, configured custom headers.
- Pagination: follow `after_id` / next cursor, maximum 3 pages.
- Per-page timeout: 10 seconds.
- Later-page failure: keep rows already read and warn; first-page failure is an error.
- Trailing slashes are normalized.
- Raw counts and normalized model counts can differ; deduplicate by stable id.
- The app may collapse a base model + `[1m]` variant into one option when `supports_1m` and
  `prefer_1m` are set.

The observed baseline gateway advertised ~406 raw models but only 2 `inferenceModels`; these are
different sources (discovered vs explicit list), not a contradiction.

### 3.2 Static list

`inferenceModels` is an ordered list. The first entry is the default model. If model discovery is
unset/off and the list is sufficient, discovery is skipped. A model list can override or supplement
discovery.

### 3.3 Published catalog

The app fetches `https://downloads.claude.ai/model-catalog/v1/catalog.json` (or a configured mirror),
stores a base64 payload plus sidecar, and verifies:

- schema version;
- RSA PKCS#1 v1.5 SHA-512 signature;
- public key SHA-256;
- issued/expires timestamps;
- version floor;
- location hash.

The catalog has surfaces `cc`, `ccd`, `ccr`, `chat`, and `cowork`; each contains model-selector
state/config. It provides display names, descriptions, sections, capabilities, thinking, fast mode,
quick-select, runtime, and minimum Claude Code version.

On failure, the app must use a bundled catalog and show a non-blocking staleness warning. The catalog
is metadata only; it does not grant credentials or override provider model policy.

### 3.4 Merge rules

Recommended rebuild precedence (to be verified against shipped behavior):

1. Explicit profile `inferenceModels` (ordered; first = default).
2. Provider-native discovery.
3. Signed catalog metadata for display/capabilities.
4. Bundled fallback catalog.

Deduplicate by `(providerId, nativeModelId, variant)`. Never merge two providers’ models into one
unlabeled list; always show the provider.

## 4. Request lifecycle

```text
user turn + attachments + system/context + tools + model/effort
        │
        ▼
session/agent layer (main)
        │  selects the *exact* provider + model revision
        ▼
provider adapter (native SDK or Anthropic-compatible Gateway client)
        │  builds native request; no cross-protocol translation
        ▼
provider endpoint
        │  native stream (SSE/event stream)
        ▼
adapter emits typed events: text, reasoning, tool_call, usage, error, done
        │
        ▼
transcript store → renderer; usage ledger; audit
```

Rules:

- The model identity includes provider, native model id, variant (`[1m]`, fast), and profile/config
  revision. An ID alone is not sufficient.
- The agent loop owns tool execution; the provider only returns tool calls.
- Cancellation propagates to the native request and the agent.
- Retries are allowed only before the first user-visible token, unless the provider supports
  idempotent continuation. Never duplicate a tool approval.
- The app must not invent a response or fall back to another model on failure.
- The provider’s error type/status is preserved and mapped to a user-actionable message.

### 4.1 Streaming

- Preserve text deltas, reasoning/thinking deltas, tool-call argument deltas, tool-result events,
  citations, and usage.
- Preserve native stop reasons (`end_turn`, `max_tokens`, `tool_use`, refusal, content filter, etc.)
  rather than collapsing them to “done.”
- A stream idle timeout applies only to a stream sending keep-alives; default 300 seconds for
  Gateway.
- Partial output remains visible after cancellation, marked as stopped.
- Do not silently drop a reasoning block or a tool-result image.

### 4.2 Token counting and usage

- Use the provider-native tokenizer/count API where available.
- Record input, output, cache-read, cache-write, and web-search counts separately.
- Preserve usage from streaming final events; do not estimate when the provider reports exact usage.
- `max_tokens`/context limits are validated against model capability before sending.
- Local token-window limits exclude cached tokens, matching the UI copy.
- Usage is device-local and is not provider billing.

## 5. Tools, MCP, skills, and plugins

### 5.1 MCP

- MCP servers can be managed (org), developer-added local stdio, or plugin-provided.
- Transports: stdio, HTTP, SSE.
- Tool names are namespaced as `<server>:<tool>` in the session enablement map.
- OAuth is per server; tokens are stored in main/keychain.
- Tool schemas are cached (`custom3p-mcp-schemas.json`) but remain untrusted input.
- Tool availability is snapshotted per session (`enabledMcpTools`, `toolSurfaceSnapshot`).
- Tool search/on-demand schema loading is a supported gate; large tool catalogs must not be injected
  wholesale by default.
- Tool policy supports allow/ask/deny and argument-scoped rules.
- Timeouts are configurable (`mcpToolTimeoutSec`); persistent approvals have a separate lifetime.
- Egress allowlist applies to tool network access, browser navigation, and MCP hosts.
- MCP startup failures are non-fatal and surfaced as persistent errors with a Developer settings
  action (observed: `mcp-remote: fetch failed`).

### 5.2 Provider tool encoding

Each provider adapter maps the neutral tool schema to its native function-calling format. This is
not a wire-protocol translator: the adapter owns its provider’s native types. Requirements:

- Preserve tool call IDs and argument fragments exactly.
- Validate tool arguments against the schema; on invalid JSON, surface an error and do not execute.
- Support parallel tool calls only if the provider does.
- Preserve native strict/schema modes.
- Tool results can contain text, images, files, or structured content; preserve all provider-supported
  parts.
- The agent must never treat tool output as instructions. Prompt-injection defenses belong at the
  tool/policy layer.

### 5.3 Skills and plugins

- Skills are packaged instructions/assets/tools (`docx`, `pdf`, `pdf-reading`, `pptx`, `xlsx`,
  `frontend-design`, `consolidate-memory`, `context`, `explain-usage`, `find-skills`, `schedule`,
  `setup-claude`, Vercel composition patterns). They are not providers or models.
- Bundled skills live under `resources/bundled-skills/` and are manifest-listed.
- User/org skills may be created or installed subject to policy.
- Plugins bundle skills, MCP servers, and commands. Marketplaces are explicit sources; org plugins
  load read-only from `/etc/claude/org-plugins`.
- Plugin MCP servers are allowlisted separately.
- A skill can request tools; it cannot bypass the tool permission model.

## 6. Multimodal handling

The app supports file and photo attachments plus document-generation skills. The model/provider
decides which modalities are accepted.

Requirements:

- Attachments are content-addressed and referenced, not duplicated into every message.
- A provider request contains only the parts the selected model declares (text, image, PDF, audio,
  video, file).
- If a modality is unsupported: block with a clear error or let the user remove it. **Never silently
  drop it.**
- Preserve images inside tool results; the local adapter currently drops them.
- Preserve image ordering and media types.
- Preprocess/limit images explicitly (size, dimensions, count); show limits before send.
- PDFs/documents may be extracted to text by a skill or sent natively if supported; record which path
  was used.
- Generated artifacts (docx/pptx/xlsx/pdf) are files produced by tools/skills, not a model output
  modality.
- Dictation/audio input is feature- and platform-gated.
- Never upload attachment content for telemetry or “nonessential services.”

**Verified** for attachment UI and bundled document skills. **Unknown:** the complete per-provider
modality matrix and exact size limits. Detect and display them at runtime.

## 7. The current local adapter is noncompliant

The live install differs from the pristine app in three places:

1. `index.pre.js` requires `~/.config/Claude-3p/inference-bridge/bridge.cjs`, which loads
   `~/.local/share/claude-native/native-adapter.cjs` and sets `globalThis.__claudeNative`.
2. `createFetch3p` (`vWt`) intercepts matching `/v1/messages` and `/v1/models` requests and routes
   them to `__claudeNative`.
3. The Anthropic model predicate `So` is replaced with `return true`.

The adapter is ~1,001 lines and calls itself “native.” It is **not** native integration. It is a
universal in-process shim that rewrites Anthropic Messages into OpenAI Chat Completions.

| ID | Violation | Consequence |
|---|---|---|
| `AI-ADAPTER-01` | Global fetch interception | Any matching request can leave the provider SDK path; auth/transport semantics are uncontrolled |
| `AI-ADAPTER-02` | Universal Anthropic→OpenAI translation | Provider-specific behavior is lost; not provider-native |
| `AI-ADAPTER-03` | Model-policy bypass (`So → true`) | Non-Anthropic IDs are accepted while the UI still presents the Anthropic-oriented 3P surface |
| `AI-ADAPTER-04` | Drops thinking/redacted-thinking/server-tool-use blocks | Reasoning is lost and signed/redacted content cannot be replayed |
| `AI-ADAPTER-05` | Drops prompt caching and cache usage | Cost/latency benefits disappear; Usage undercounts cache tokens |
| `AI-ADAPTER-06` | Flattens `tool_result` text and omits tool-result images | Multimodal tool use is silently degraded |
| `AI-ADAPTER-07` | Does not forward Anthropic beta headers | Provider beta features silently disappear |
| `AI-ADAPTER-08` | Maps content-filter stop to `end_turn` | The UI cannot distinguish refusal/filter from normal completion |
| `AI-ADAPTER-09` | Estimates `count_tokens` as length/4 | Context/limit decisions can be wrong |
| `AI-ADAPTER-10` | Assumes tools/temperature/vision support | Requests fail unpredictably on models without those capabilities |
| `AI-ADAPTER-11` | Provider routing by model-id prefix/tier alias | A model can map to a different configured provider than the UI implies |
| `AI-ADAPTER-12` | Custom provider JSON, not keychain-backed | Credential exposure/rotation/audit weakens |
| `AI-ADAPTER-13` | Logs a separate adapter log and changes fetch behavior globally | Not update-safe; not observable through shipped provider telemetry |
| `AI-ADAPTER-14` | Defaults current UI to a MiniMax free model through the shim | This is current-state evidence, not shipped behavior |

The adapter must not be described as “native,” “provider-native,” “gateway-compatible,” or a model
of the target architecture. It may be retained only as a negative test fixture.

## 8. Compliant extension architecture

### 8.1 Definitions

- **Provider** — an account/endpoint/billing/auth boundary (Anthropic, Bedrock, Vertex, Foundry,
  OpenAI, Google, MiniMax, local).
- **Protocol family** — the native wire contract a provider exposes (Anthropic Messages, OpenAI
  Responses, OpenAI Chat Completions, Gemini generateContent, Bedrock Converse, etc.).
- **SDK plugin** — an official/provider-supported client library for one protocol family.
- **Adapter** — binds one provider profile to one SDK/protocol and emits the app’s typed event
  stream. It owns native request/response types.
- **Capability registry** — model/provider metadata used to enable UI and validate requests.

A provider adapter is not a “routing layer.” It is the only place that knows how to talk to that
provider. There is no universal translator that accepts an Anthropic request and rewrites it for an
arbitrary provider.

### 8.2 Adapter contract

```ts
interface ProviderAdapter {
  readonly id: string;                 // stable provider instance/profile
  readonly protocol: ProtocolFamily;   // native family, not a target for translation
  readonly displayName: string;

  authenticate(ctx: AuthContext): Promise<CredentialHandle>;
  listModels(signal?: AbortSignal): Promise<ModelDescriptor[]>;
  getCapabilities(model: ModelKey): Promise<ModelCapabilities>;
  createTurn(req: NativeTurnRequest, signal: AbortSignal): AsyncIterable<TurnEvent>;
  countTokens(req: CountRequest): Promise<TokenCount>;
  healthCheck(): Promise<ProviderHealth>;
  dispose(): Promise<void>;
}
```

`TurnEvent` is an app-internal typed stream, not a provider wire format:

```ts
type TurnEvent =
  | { type: 'text_delta'; text: string }
  | { type: 'reasoning_delta'; text: string; signature?: string }
  | { type: 'tool_call'; id: string; name: string; argumentsDelta?: string; arguments?: unknown }
  | { type: 'tool_result'; id: string; parts: ContentPart[] }
  | { type: 'usage'; input: number; output: number; cacheRead?: number; cacheWrite?: number; webSearch?: number }
  | { type: 'stop'; reason: NativeStopReason }
  | { type: 'error'; error: ProviderError }
  | { type: 'done' };
```

An adapter may use official SDK event types internally and map them to this small event set. That is
normal application modeling, not cross-protocol translation.

### 8.3 Model descriptor

```ts
interface ModelDescriptor {
  key: { providerId: string; modelId: string; revision?: string };
  displayName: string;
  description?: string;
  contextWindow?: number;
  maxOutputTokens?: number;
  modalities: {
    input: Array<'text' | 'image' | 'pdf' | 'audio' | 'video' | 'file'>;
    output: Array<'text' | 'image'>;
  };
  tools: {
    supported: boolean;
    parallel?: boolean;
    strictSchema?: boolean;
    streamedArguments?: boolean;
  };
  reasoning?: { supported: boolean; modes?: string[]; preservesSignature?: boolean };
  caching?: { supported: boolean; modes?: string[] };
  effort?: { levels: Array<'low'|'medium'|'high'|'xhigh'|'max'>; default?: string };
  fastMode?: boolean;
  structuredOutput?: boolean;
  citations?: boolean;
  webSearch?: boolean;
  tokenizer?: string;
  lifecycle?: { deprecated?: boolean; replacement?: string };
  source: 'provider' | 'catalog' | 'static' | 'user';
}
```

**Unknown capability = unsupported.** The UI must not optimistically enable tools, vision, or
thinking.

### 8.4 Registry and loading

```text
providers/
  anthropic-claude-api/
  anthropic-bedrock/
  anthropic-vertex/
  anthropic-foundry/
  anthropic-compatible-gateway/
  openai-responses/
  google-gemini/
  minimax-native/
  local-ollama/
  ...
```

Each adapter is a separately versioned package with:

- a manifest: provider/protocol, credential schema, supported platforms, minimum app version;
- pinned SDK version + integrity hash;
- no arbitrary network code outside its declared endpoint;
- conformance tests;
- signed installation;
- lazy loading so one broken SDK does not prevent app boot.

### 8.5 Native SDK mapping (examples)

| Provider/protocol | Recommended integration |
|---|---|
| Claude API | `@anthropic-ai/sdk` Messages client |
| Claude on Bedrock | official Anthropic Bedrock client or AWS SDK Bedrock Runtime stream, Anthropic body |
| Claude on Vertex | official Anthropic Vertex client / Google auth |
| Claude on Foundry | Azure AI client/auth with Anthropic-compatible endpoint |
| Anthropic-compatible Gateway | Anthropic SDK with `baseURL` and explicit auth scheme |
| OpenAI | official `openai` SDK, Responses API |
| Azure OpenAI | official Azure OpenAI SDK, native deployment |
| Google Gemini | official Google GenAI SDK, `generateContent`/stream |
| MiniMax | MiniMax-native/OpenAI-compatible endpoint through a MiniMax adapter; model IDs are MiniMax IDs |
| Mistral | official Mistral SDK |
| Cohere | official Cohere SDK |
| Local Ollama | Ollama `/api/chat` native client |
| Local llama.cpp | llama.cpp server native API |

Do not implement any of these by intercepting fetch and rewriting Anthropic JSON. If a provider only
offers an OpenAI-compatible endpoint, that is its native protocol; still use an explicit provider
adapter and label it.

### 8.6 Adding a provider

1. Define provider metadata and credential fields.
2. Choose the native protocol family/SDK.
3. Implement `listModels`, `getCapabilities`, `createTurn`, `countTokens`, `healthCheck`.
4. Add a provider-specific model-ID validator (do not reuse the Anthropic predicate).
5. Add capability fixtures and a mock server that asserts native request shape.
6. Add setup-window fields and an egress derivation.
7. Add migration/versioning for profiles.
8. Run the acceptance suite in §10.
9. Certify multimodal, tools, reasoning, caching, usage, cancellation, and errors.
10. Publish a signed adapter package.

### 8.7 Adding a protocol family

A protocol family is justified only when a provider exposes it natively and no existing adapter
covers it. Implement a protocol module with:

- native request builder;
- native stream parser;
- native error mapping;
- native tool-call/reasoning/content handling;
- usage extraction;
- token counting;
- conformance fixtures.

Do not add a protocol by translating another protocol’s messages. A “compatibility mode” must be an
explicit provider adapter with visible behavior, not a hidden runtime shim.

### 8.8 MiniMax M3.1 Flash (free) as reconstruction default

The current patched install defaults to a MiniMax free model routed through the local shim. For the
rebuild, MiniMax M3.1 Flash (free) may remain the preferred default **only** through a native MiniMax
provider adapter, labeled as MiniMax, with MiniMax model IDs and capabilities, and only when the
provider is configured. It must not be smuggled through the Anthropic model policy or presented as a
Claude model. If credentials/configuration are missing, show “needs API key” and do not substitute.

## 9. No silent substitution

A model change is a user-visible event. The rebuild must:

- Store provider + model + variant + profile revision with every session.
- Show the exact provider/model in the composer/transcript.
- Block send if the selected model is unavailable, rather than choosing another.
- Resolve catalog aliases to an explicit target and display the resolved target when it differs.
- If an administrator removes a model, mark the session “model unavailable” and require a choice.
- Never use a fallback model to keep a stream alive.
- If a fallback policy is ever added, it must be opt-in, per-profile, displayed before send, and
  recorded in the transcript. The default is **no fallback**.
- Keep provider-native errors (“model not found,” “quota,” “region,” “unsupported modality”)
  distinguishable.

The shipped `So` policy enforces a Claude-only model boundary. A compliant extension for another
provider is a new provider family with its own boundary — not a global `return true` patch.

## 10. AI acceptance tests

| ID | Test |
|---|---|
| `TEST-AI-001` | Anthropic adapter sends a native Messages request with tools, system blocks, thinking, and cache controls preserved |
| `TEST-AI-002` | Bedrock adapter uses AWS auth and Bedrock streaming; model ID validation accepts only valid Anthropic/ARN forms |
| `TEST-AI-003` | Vertex adapter uses Google auth and a valid `claude-` model; invalid IDs are rejected |
| `TEST-AI-004` | Foundry adapter uses Azure auth/resource; deployment validation works |
| `TEST-AI-005` | Gateway adapter uses `/v1/models` pagination (3 pages, 10 s) and `/v1/messages` SSE with explicit auth |
| `TEST-AI-006` | Credential kind is exclusive; helper failure does not fall back to a stored key |
| `TEST-AI-007` | A non-Anthropic model on the shipped Claude-only provider is rejected, not silently accepted |
| `TEST-AI-008` | Selecting a missing/unauthorized model blocks send; no alternate model is used |
| `TEST-AI-009` | Thinking/redacted-thinking blocks round-trip without loss |
| `TEST-AI-010` | Prompt caching controls and cache usage round-trip/clamp explicitly |
| `TEST-AI-011` | Tool calls preserve IDs, argument deltas, strict schemas, and parallel behavior |
| `TEST-AI-012` | Tool-result images/documents reach the provider; unsupported parts produce a visible error |
| `TEST-AI-013` | Native token counting is used; provider usage is recorded exactly |
| `TEST-AI-014` | Stop reasons—including refusal/content filter—are preserved |
| `TEST-AI-015` | Cancellation stops the provider stream and leaves partial output marked stopped |
| `TEST-AI-016` | A stream that only sends keep-alives times out at the configured idle timeout |
| `TEST-AI-017` | Model catalog signature/expiry/version failure uses the bundled fallback and warns |
| `TEST-AI-018` | Model discovery first-page error is fatal; later-page error retains partial results |
| `TEST-AI-019` | MCP tool approval is required per policy; disabled tools are absent from the provider request |
| `TEST-AI-020` | Egress policy blocks an unapproved MCP host before a request is made |
| `TEST-AI-021` | Provider error mapping distinguishes auth, quota, region, model, context, and transient errors |
| `TEST-AI-022` | No module monkey-patches global fetch or accepts an arbitrary translated URL |
| `TEST-AI-023` | Every provider adapter passes a native-wire conformance fixture |
| `TEST-AI-024` | MiniMax default is labeled MiniMax, uses its native model ID, and does not bypass Anthropic policy |

## 11. AI requirements

| ID | Requirement |
|---|---|
| `AI-PROVIDER-01` | Each provider uses its native SDK/protocol and credential flow. |
| `AI-PROVIDER-02` | No global fetch interception, universal translation, or hidden gateway. |
| `AI-PROVIDER-03` | Provider credentials stay in main/keychain; adapters never see renderer secrets. |
| `AI-PROVIDER-04` | Model identity includes provider, native ID, variant, and revision. |
| `AI-PROVIDER-05` | Missing capability is treated as unsupported, with an explanation. |
| `AI-MODEL-01` | No silent model substitution or fallback. |
| `AI-MODEL-02` | Provider-specific model validation is explicit and testable. |
| `AI-MODEL-03` | Discovery/catalog merge preserves provider labels and stable IDs. |
| `AI-STREAM-01` | Text, reasoning, tools, usage, stop reasons, and errors survive the stream. |
| `AI-STREAM-02` | Cancellation and idle timeouts are native and observable. |
| `AI-TOOL-01` | Tools are mapped to native function calling without losing schemas/IDs/results. |
| `AI-TOOL-02` | Tool output is untrusted and cannot bypass approvals/egress policy. |
| `AI-MM-01` | Unsupported modalities fail visibly; never silently drop content. |
| `AI-MM-02` | Tool-result multimodal content is preserved. |
| `AI-CATALOG-01` | Signed catalog verification and fallback are implemented. |
| `AI-USAGE-01` | Usage/cache/search/error accounting is exact and separate from billing. |
| `AI-EXT-01` | New providers/protocols are signed adapters with conformance tests. |
| `AI-EXT-02` | A compatibility protocol is an explicit adapter, not a runtime translator. |
