//! The **local provider** extension — Phase 8's second reference extension
//! (`docs/13` §"Phase 8"; `docs/12` §8 adapter contract): a native wall for
//! locally running inference servers, scoped to the runtimes' documented
//! **OpenAI-compatible surfaces** — `POST {base}/v1/chat/completions` and
//! the listing endpoints below. Ollama (default port 11434) and the
//! llama.cpp `llama-server` (default port 8080) both document these paths;
//! the wall does not assume the two runtimes are identical anywhere else.
//!
//! The wall shares a *design* with the sibling Chat Completions wall
//! (`myharness-provider-openai`) and deliberately no code (`AI-PROVIDER-02`,
//! `AI-EXT-02`): the app-internal Anthropic-shaped model maps onto this wire
//! only here.
//!
//! Wire readings pinned for v1 — this wire was never observed in the shipped
//! app (Phase 8 extensions are rebuild additions), so every reading is a
//! rebuild-defined choice recorded in `docs/14` §9:
//!
//! - **Destination security is the wall's core** (`AI-LOCAL-01..04`). The
//!   base URL's host must be a **loopback IP literal or `localhost`**;
//!   anything else refuses before any bytes (`AI-LOCAL-01`). A hostname is
//!   resolved through an injectable resolver and **every** resolved address
//!   must be loopback — a name that resolves to a public address (DNS/IP
//!   rebinding) is refused (`AI-LOCAL-02`) — and requests are pinned to the
//!   resolved loopback address for the adapter's lifetime, so a later
//!   re-resolution cannot silently move the destination (`AI-LOCAL-03`).
//!   The wall builds its own per-instance `reqwest::Client` with
//!   **redirects disabled**: a local endpoint that answers with a redirect
//!   to any other origin produces an error, never a follow (`AI-LOCAL-04`).
//!   This is the one deliberate deviation from the injected-client posture —
//!   it is per-instance, nothing process-global (`AI-ADAPTER-13`), and it is
//!   what makes redirect refusal enforceable inside the wall rather than a
//!   promise about the caller.
//! - **Keyless is the default, never a bypass** (`AI-LOCAL-05`): an absent
//!   or blank credential sends **no `authorization` header at all**; a
//!   present one sends `Bearer`. The destination gate above is
//!   transport-level and applies identically to both — keyless never
//!   widens where the wall may connect.
//! - **Discovery reads both runtimes' listing shapes** (`AI-LOCAL-06`):
//!   `GET {base}/api/tags` first (Ollama's keyed `models` array, ids under
//!   `name`/`model`), and `GET {base}/v1/models` (the OpenAI `data` array)
//!   when the first endpoint is absent. A 404 on `/api/tags` is not an
//!   error — it is the llama.cpp posture. Only a failure on BOTH is one.
//! - The chat grammar follows the Chat Completions readings the sibling
//!   wall pinned (system joins `\n\n` into one `role:"system"` message,
//!   `max_tokens`, flat drop of `top_k`/thinking/betas/`cache_control`,
//!   images as `image_url` data URIs, documents and tool-result media
//!   refused, assistant reasoning omitted from replay, streamed reasoning
//!   via `delta.reasoning_content` (fallback `delta.reasoning`), tool
//!   fragments via `delta.tool_calls` with the recorded synthetic ids and
//!   the close at `finish_reason`, the stop map with unknown spellings
//!   verbatim, prefix model echo, exact usage with cached tokens, `data: {}`
//!   keep-alives that never reset the idle window, and the `data: [DONE]`
//!   sentinel required) — pinned here by this crate's own fixtures, never
//!   by importing the sibling's.
//! - **Token counting is an explicit refusal** — neither runtime documents
//!   a compatible count surface on this path, and an estimate is forbidden
//!   (`AI-ADAPTER-09`).
//! - A base URL that ends in `/v1` is normalized (the suffix is stripped)
//!   so both the server-root spelling (`http://127.0.0.1:11434`) and the
//!   OpenAI-style spelling (`.../v1`) address the same paths.
//!
//! Boundaries (`docs/12` §11): credentials exist only as request-header
//! material (`AI-PROVIDER-03`); nothing here is process-global
//! (`AI-ADAPTER-13`); model ids travel verbatim, ungated — the only
//! identity checks are structural (`docs/14` §9 owner decision, 2026-10-23).
//!
//! Traceability: `TEST-EXT-003/005/006` adjacency, `AI-LOCAL-01..06`,
//! `AI-PROVIDER-01..05`, `AI-MODEL-01`, `AI-STREAM-01/02`, `AI-TOOL-01`,
//! `AI-MM-01/02`, `AI-USAGE-01`, `TEST-AI-013/014/016/021/023`.

pub mod adapter;
pub mod chat;
pub mod destination;
pub mod discovery;

pub use adapter::LocalAdapter;
pub use chat::{LocalClientConfig, ProbeStatus, DEFAULT_IDLE_TIMEOUT};
pub use destination::{Destination, LOOPBACK_PORTS};

/// The default local base URL: the Ollama server root. The llama.cpp
/// server's own default port (8080) is reached by overriding the base URL;
/// neither runtime's port is invented here beyond the one documented
/// default the family carries (`docs/14` §9).
pub const DEFAULT_LOCAL_BASE: &str = "http://127.0.0.1:11434";
