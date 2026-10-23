//! The [`GatewayAdapter`]: the one place in the app that talks the gateway's
//! native protocol (`docs/12` §8.1; `docs/10` §9). It implements the
//! [`myharness_provider_sdk::ProviderAdapter`] contract and adds the one-shot
//! inference probe the setup window's *Test connection* runs
//! (`docs/screens/setup-desktop-3p.md`, Phase 3 slice 1).
//!
//! Boundaries this crate holds (`docs/12` §11 requirements):
//!
//! - **Credentials** arrive resolved in an [`AuthContext`], re-resolved by the
//!   caller per request — rotation without a second credential source
//!   (`AI-PROVIDER-03`; the `AI-ADAPTER-12` regression). The adapter neither
//!   reads the keychain nor writes provider credentials anywhere.
//! - **The model policy** gates every send: an id that fails the Anthropic
//!   predicate (`myharness_config::forms::validate_anthropic_model_id`,
//!   `docs/12` §2.2) is refused before any bytes go out (`TEST-AI-007`
//!   gateway half; the `AI-ADAPTER-03` regression is that policy, enforced).
//! - **No substitution** at any layer: the model id travels verbatim on the
//!   wire (`AI-ADAPTER-11`), the performed model in the stream's opening
//!   frame is compared against it, and a mismatch ends the stream with a
//!   visible error (`AI-MODEL-01`).
//! - **Nothing global** (`AI-ADAPTER-13`): a concrete [`reqwest::Client`] is
//!   injected per adapter; there is no fetch interception, no module state,
//!   no side-channel log file.

use std::collections::{BTreeMap, VecDeque};
use std::convert::TryFrom;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures::Stream;
use myharness_provider_sdk::{
    AuthContext, ContentDelta, ContentPart, CountRequest, ErrorClass, ModelCapabilities,
    ModelDescriptor, ModelKey, NativeStopReason, ProtocolFamily, ProviderAdapter, ProviderError,
    ProviderHealth, TokenCount, TurnEvent, TurnStream, Usage,
};
use secrecy::ExposeSecret;

use crate::discovery::{descriptor_from_row, DiscoveryOutcome, ANTHROPIC_VERSION};
use crate::messages::{
    count_tokens_request_body, error_from_envelope, error_from_status, messages_request_body,
    parse_usage, SseParser, StreamEvent,
};
use crate::{GatewayClientConfig, ProbeStatus};

/// The gateway adapter. Constructed per profile by the caller (the bridge);
/// [`reqwest::Client`] is injected so connection pooling is the caller's
/// decision and nothing here is process-global (`AI-ADAPTER-13`).
pub struct GatewayAdapter {
    config: GatewayClientConfig,
    client: reqwest::Client,
}

impl GatewayAdapter {
    pub fn new(config: GatewayClientConfig, client: reqwest::Client) -> Self {
        let mut config = config;
        // Trailing slashes are normalized (`docs/12` §3.1).
        config.base_url = config.base_url.trim().trim_end_matches('/').to_string();
        GatewayAdapter { config, client }
    }

    /// The normalized base URL.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// The shared request header set: the version header, the credential per
    /// the configured scheme, forwarded custom headers, and the beta list
    /// (`AI-ADAPTER-07`). The credential exists only as request-header
    /// material — it is never logged, never persisted, and never copied into
    /// any other store (`AI-PROVIDER-03`).
    fn request_builder(
        &self,
        method: reqwest::Method,
        url: &str,
        auth: &AuthContext,
        extra_betas: &[String],
    ) -> reqwest::RequestBuilder {
        let mut builder = self
            .client
            .request(method, url)
            .header("anthropic-version", ANTHROPIC_VERSION);
        let credential = auth.credential().expose_secret();
        builder = match self.config.auth_scheme {
            crate::discovery::AuthScheme::Bearer => {
                builder.header("authorization", format!("Bearer {credential}"))
            }
            crate::discovery::AuthScheme::XApiKey => builder.header("x-api-key", credential),
        };
        for (name, value) in &self.config.custom_headers {
            builder = builder.header(name, value);
        }
        // Configured profile betas plus this turn's betas, in first-seen
        // order (headers are a list; duplicates collapse, nothing invented).
        let mut betas: Vec<&String> = self.config.betas.iter().collect();
        for beta in extra_betas {
            if !betas.contains(&beta) {
                betas.push(beta);
            }
        }
        if !betas.is_empty() {
            let joined: Vec<&str> = betas.iter().map(|beta| beta.as_str()).collect();
            builder = builder.header("anthropic-beta", joined.join(","));
        }
        builder
    }

    /// The `{base}/v1/messages` URL. The path is fixed — a request only ever
    /// goes to the configured endpoint (`TEST-AI-022`: no arbitrary URLs).
    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.config.base_url)
    }

    /// A URL under this adapter's configured endpoint only.
    fn endpoint_url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    /// Read a response's JSON, or turn a failure into the mapped provider
    /// error. A non-success status is parsed for the native error envelope
    /// (its `type` and `message` only — no raw bodies, `docs/18` §11).
    async fn response_json(
        response: reqwest::Response,
    ) -> Result<serde_json::Value, ProviderError> {
        let status = response.status();
        if !status.is_success() {
            let status = status.as_u16();
            let body = response.json::<serde_json::Value>().await.ok();
            return Err(match body {
                Some(body) => error_from_envelope(Some(status), &body),
                None => error_from_status(status),
            });
        }
        response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ProviderError {
                class: ErrorClass::InvalidRequest,
                status: Some(status.as_u16()),
                message: format!("the gateway answer could not be read: {error}"),
            })
    }

    /// Open a request; unreachable transports map to `Unreachable`.
    async fn send(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, ProviderError> {
        builder.send().await.map_err(|error| ProviderError {
            class: ErrorClass::Unreachable,
            status: None,
            message: format!("the gateway could not be reached: {error}"),
        })
    }

    /// A non-success response, mapped: envelope first, bare status otherwise.
    async fn response_error(response: reqwest::Response) -> ProviderError {
        let status = response.status().as_u16();
        let body = response.json::<serde_json::Value>().await.ok();
        match body {
            Some(body) => error_from_envelope(Some(status), &body),
            None => error_from_status(status),
        }
    }

    /// Send non-streaming `/v1/messages`.
    async fn send_message(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        let builder = self
            .request_builder(
                reqwest::Method::POST,
                &self.messages_url(),
                auth,
                &request.betas,
            )
            .json(&messages_request_body(request)?);
        let response = Self::response_json(self.send(builder).await?).await?;
        Ok(response)
    }

    /// The unsigned policy gate every send passes first (`TEST-AI-007/008`).
    fn gate_model(&self, model: &ModelKey) -> Result<(), ProviderError> {
        if self.config.base_url.is_empty() {
            return Err(ProviderError {
                class: ErrorClass::Configuration,
                status: None,
                message: "the gateway base URL is required before sending anything".to_string(),
            });
        }
        match myharness_config::forms::validate_anthropic_model_id(&model.model_id) {
            Ok(()) => Ok(()),
            Err(reason) => Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!(
                    "the requested model {:?} does not pass the Anthropic model policy and was not sent: {reason}",
                    model.model_id
                ),
            }),
        }
    }

    /// Translate a complete (non-streaming) Messages response into the typed
    /// event set (`docs/12` §8.2): content, usage, stop reason, done. Content
    /// blocks are preserved part-for-part; an unknown block kind is an error,
    /// never a silent drop (`AI-MM-01`, `AI-STREAM-01`).
    fn events_from_response(
        response: &serde_json::Value,
        requested_model: &ModelKey,
    ) -> Result<Vec<TurnEvent>, ProviderError> {
        // The response names the model it performed: substitution is a
        // user-visible event, never something to ride through (`AI-MODEL-01`).
        if let Some(ran) = response.get("model").and_then(serde_json::Value::as_str) {
            if ran != requested_model.model_id {
                return Err(ProviderError {
                    class: ErrorClass::ModelNotFound,
                    status: None,
                    message: format!(
                        "the gateway performed model {ran:?} while the request named {:?}; the turn was refused",
                        requested_model.model_id
                    ),
                });
            }
        }
        let mut events = Vec::new();
        for block in response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let kind = block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            match kind {
                "text" => events.push(TurnEvent::TextDelta {
                    text: block
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                }),
                "tool_use" => events.push(TurnEvent::ToolCall {
                    id: block
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    name: block
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    arguments_delta: None,
                    arguments: Some(block.get("input").cloned().unwrap_or_default()),
                }),
                "thinking" => events.push(TurnEvent::ReasoningDelta {
                    text: block
                        .get("thinking")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    signature: block
                        .get("signature")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                }),
                "redacted_thinking" => events.push(TurnEvent::ReasoningRedacted {
                    data: block.clone(),
                }),
                other => {
                    return Err(ProviderError {
                        class: ErrorClass::UnsupportedModality,
                        status: None,
                        message: format!(
                            "the response carried a content block of an unhandled kind: {other:?}"
                        ),
                    });
                }
            }
        }
        let usage = response
            .get("usage")
            .and_then(parse_usage)
            .unwrap_or_default();
        if usage != Usage::default() {
            events.push(TurnEvent::Usage { usage });
        }
        match response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str)
            .map(NativeStopReason::parse)
        {
            Some(reason) => events.push(TurnEvent::Stop { reason }),
            None => {
                // A response without a stop reason is contract-malformed:
                // refusing an invented "end of turn" is the point.
                return Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: "the response carried no stop reason".to_string(),
                });
            }
        }
        events.push(TurnEvent::Done);
        Ok(events)
    }

    /// The one-shot inference probe (the setup window's *Test connection*;
    /// `docs/screens/setup-desktop-3p.md` — a real request, nothing persisted,
    /// no draft changed by the answer).
    ///
    /// Sends a 1-token turn to the requested model. Reports exact usage and
    /// the native stop reason (`TEST-AI-013/014` analog for probes), returns
    /// provider error classes verbatim (`TEST-AI-021`), and refuses a model
    /// swap (`AI-MODEL-01`). Latency is measured around the native request.
    pub async fn probe_inference(
        &self,
        auth: &AuthContext,
        model_id: &str,
    ) -> Result<ProbeStatus, ProviderError> {
        let model = ModelKey {
            provider_id: self.id().to_string(),
            model_id: model_id.to_string(),
            variant: None,
            revision: None,
        };
        self.gate_model(&model)?;
        self.authenticate(auth).await?;
        let request = myharness_provider_sdk::NativeTurnRequest {
            model,
            system: Vec::new(),
            messages: vec![(
                myharness_provider_sdk::Role::User,
                vec![ContentPart::Text {
                    text: "ping".to_string(),
                }],
            )],
            tools: Vec::new(),
            max_tokens: 1,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: Vec::new(),
            thinking: None,
            betas: Vec::new(),
            stream: false,
        };
        let started = Instant::now();
        let response = self.send_message(auth, &request).await?;
        let latency = started.elapsed();
        // The model echo gate applies to probes too: the answer must name the
        // model that was requested (`AI-MODEL-01`).
        let ran = response
            .get("model")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(model_id);
        if ran != model_id {
            return Err(ProviderError {
                class: ErrorClass::ModelNotFound,
                status: None,
                message: format!(
                    "the gateway performed model {ran:?} while the probe named {model_id:?}"
                ),
            });
        }
        let stop_reason = response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str)
            .map(NativeStopReason::parse)
            .map(|reason| reason.wire());
        let usage = response
            .get("usage")
            .and_then(parse_usage)
            .unwrap_or_default();
        Ok(ProbeStatus {
            ok: true,
            model: ran.to_string(),
            stop_reason,
            output_tokens: usage.output,
            input_tokens: usage.input,
            latency_ms: u64::try_from(latency.as_millis()).unwrap_or(u64::MAX),
        })
    }
}

/// Turn a successful streaming response into the typed [`TurnStream`]: the
/// bytes stream feeds the SSE parser, the state machine guards the model
/// echo and the idle window, and dropping the returned stream cancels the
/// native request (`TEST-AI-015`).
fn finish_stream_into_turn_stream(
    response: reqwest::Response,
    requested_model: String,
    idle_timeout: Duration,
) -> TurnStream {
    // A zero-sized window means "no idle timeout" — the profile documents a
    // nonzero default (`docs/20`: `inferenceStreamIdleTimeoutSec`); this is
    // the one deliberate off-switch.
    let idle_armed = !idle_timeout.is_zero();
    Box::pin(TurnStreamImpl {
        inner: Box::pin(response.bytes_stream()),
        parser: SseParser::new(),
        requested_model,
        idle_timeout,
        idle: Box::pin(tokio::time::sleep(idle_timeout)),
        idle_armed,
        tools: BTreeMap::new(),
        usage: Usage::default(),
        stop: None,
        outbox: VecDeque::new(),
        finished: false,
        saw_message_stop: false,
    })
}

#[async_trait::async_trait]
impl ProviderAdapter for GatewayAdapter {
    fn id(&self) -> &str {
        "gateway"
    }

    fn protocol(&self) -> ProtocolFamily {
        ProtocolFamily::AnthropicMessages
    }

    fn display_name(&self) -> &str {
        "Gateway"
    }

    /// A shape check with no network: non-empty credential, no OAuth-token
    /// material this rebuild cannot inform correctly.
    async fn authenticate(&self, auth: &AuthContext) -> Result<(), ProviderError> {
        let credential = auth.credential().expose_secret();
        if credential.trim().is_empty() {
            return Err(ProviderError {
                class: ErrorClass::Auth,
                status: None,
                message: "the credential is empty; nothing was sent".to_string(),
            });
        }
        // `sk-ant-oat…` is an Anthropic OAuth access token that trades on a
        // beta header whose exact value was never captured (`docs/12` §2.1;
        // the deferral is recorded in `docs/14` §9). Sending it uninformed
        // would be the `AI-ADAPTER-07` violation in new clothing, so the
        // adapter refuses — visibly, with the reason.
        if credential.starts_with("sk-ant-oat") {
            return Err(ProviderError {
                class: ErrorClass::Auth,
                status: None,
                message: "this credential is an Anthropic OAuth access token; its beta header value was never captured, so this rebuild refuses to guess it".to_string(),
            });
        }
        Ok(())
    }

    /// The native listing: `/v1/models` discovery, rows mapped to
    /// descriptors (`TEST-AI-005` discovery half; `TEST-AI-018`;
    /// `docs/12` §3.1; `docs/10` §9 assigns discovery to `providers`).
    async fn list_models(&self, auth: &AuthContext) -> Result<Vec<ModelDescriptor>, ProviderError> {
        if self.config.base_url.is_empty() {
            return Err(ProviderError {
                class: ErrorClass::Configuration,
                status: None,
                message: "the gateway base URL is required before listing models".to_string(),
            });
        }
        let outcome: DiscoveryOutcome = crate::discovery::discover_pages(
            &self.config.base_url,
            self.config.auth_scheme,
            // Header material for this listing's requests only; the value
            // came from the caller's per-request resolution (rotation) —
            // the adapter keeps no cached copy (`AI-PROVIDER-03`).
            Some(auth.credential().expose_secret().to_string()),
            &self.config.custom_headers,
            &self.client,
            crate::discovery::DISCOVERY_PAGE_TIMEOUT.as_secs(),
            crate::discovery::DISCOVERY_MAX_PAGES,
        )
        .await?;
        let mut descriptors: Vec<ModelDescriptor> = outcome
            .models
            .iter()
            .filter_map(|model| descriptor_from_row(&model.raw))
            .collect();
        for descriptor in &mut descriptors {
            descriptor.key.provider_id = self.id().to_string();
        }
        Ok(descriptors)
    }

    /// Capability lookup over the resolved descriptor list; absent metadata
    /// stays unsupported (`AI-PROVIDER-05`).
    fn capabilities(&self, model: &ModelKey, descriptors: &[ModelDescriptor]) -> ModelCapabilities {
        descriptors
            .iter()
            .find(|descriptor| descriptor.key.model_id == model.model_id)
            .map(|descriptor| descriptor.capabilities.clone())
            .unwrap_or_default()
    }

    /// Build the native request and open the SSE stream (`docs/12` §4.1).
    /// A non-streaming request becomes a buffered event list from the same
    /// contract.
    async fn create_turn(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<TurnStream, ProviderError> {
        self.gate_model(&request.model)?;
        self.authenticate(auth).await?;
        if request.stream {
            let builder = self
                .request_builder(
                    reqwest::Method::POST,
                    &self.messages_url(),
                    auth,
                    &request.betas,
                )
                .json(&messages_request_body(request)?);
            let response = self.send(builder).await?;
            if !response.status().is_success() {
                return Err(Self::response_error(response).await);
            }
            Ok(finish_stream_into_turn_stream(
                response,
                request.model.model_id.clone(),
                self.config.idle_timeout,
            ))
        } else {
            let response = self.send_message(auth, request).await?;
            let events = Self::events_from_response(&response, &request.model)?;
            Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))))
        }
    }

    /// The native counting endpoint. When the gateway declines the endpoint,
    /// the answer is an error — never an estimate (`AI-ADAPTER-09`).
    async fn count_tokens(
        &self,
        auth: &AuthContext,
        request: &CountRequest,
    ) -> Result<TokenCount, ProviderError> {
        self.gate_model(&request.model)?;
        let builder = self
            .request_builder(
                reqwest::Method::POST,
                &self.endpoint_url("/v1/messages/count_tokens"),
                auth,
                &[],
            )
            .json(&count_tokens_request_body(request)?);
        let response = self.send(builder).await?;
        if !response.status().is_success() {
            return Err(Self::response_error(response).await);
        }
        let status = response.status().as_u16();
        let body = Self::response_json(response).await?;
        let input_tokens = body
            .get("input_tokens")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| ProviderError {
                class: ErrorClass::InvalidRequest,
                status: Some(status),
                message: "the count answer carried no input_tokens".to_string(),
            })?;
        Ok(TokenCount {
            input_tokens,
            native: true,
        })
    }

    /// `/v1/models?limit=1` as the reachability check (`docs/12` §8.2
    /// `healthCheck`). No inference happens here.
    async fn health_check(&self, auth: &AuthContext) -> Result<ProviderHealth, ProviderError> {
        if self.config.base_url.is_empty() {
            return Ok(ProviderHealth {
                reachable: false,
                authenticated: None,
                detail: Some("the gateway base URL is not configured".to_string()),
            });
        }
        let builder = self.request_builder(
            reqwest::Method::GET,
            &self.endpoint_url("/v1/models?limit=1"),
            auth,
            &[],
        );
        let Ok(response) = self.send(builder).await else {
            return Ok(ProviderHealth {
                reachable: false,
                authenticated: None,
                detail: None,
            });
        };
        let status = response.status().as_u16();
        if (401..=403).contains(&status) {
            Ok(ProviderHealth {
                reachable: true,
                authenticated: Some(false),
                detail: Some(format!("status {status}")),
            })
        } else if (200..299).contains(&status) {
            Ok(ProviderHealth {
                reachable: true,
                authenticated: Some(true),
                detail: None,
            })
        } else {
            Ok(ProviderHealth {
                reachable: true,
                authenticated: None,
                detail: Some(format!("status {status}")),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// The streaming state machine
// ---------------------------------------------------------------------------

/// One in-flight tool call whose arguments stream in as
/// `input_json_delta` fragments (`AI-TOOL-01`: ids, names, and argument
/// deltas preserved; assembled arguments validated at close).
#[derive(Debug, Clone)]
struct ToolBlockState {
    id: String,
    name: String,
    buffer: String,
}

/// The `TurnStream` implementation for streamed turns: parses the gateway's
/// native SSE frames into the app's typed events, guards the model echo,
/// preserves usage and stop reasons, and enforces the idle window
/// (`TEST-AI-014/015/016`; the §4.1 reading in `messages.rs` module docs).
///
/// Dropping the stream cancels the native request: the `reqwest` response
/// (and its body task) drops with it, which closes the connection — the
/// caller keeps whatever was already emitted and marks it stopped
/// (`docs/12` §4.1).
pub(crate) struct TurnStreamImpl {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    parser: SseParser,
    requested_model: String,
    idle_timeout: Duration,
    /// The idle-window timer (re-armed on content progress; `ping` is not
    /// progress — the §4.1 reading).
    idle: Pin<Box<tokio::time::Sleep>>,
    /// Whether the idle window governs this stream at all (a `0` window
    /// means "no idle timeout"; the profile documents a nonzero default).
    idle_armed: bool,
    tools: BTreeMap<usize, ToolBlockState>,
    usage: Usage,
    stop: Option<NativeStopReason>,
    outbox: VecDeque<Result<TurnEvent, ProviderError>>,
    finished: bool,
    saw_message_stop: bool,
}

impl TurnStreamImpl {
    fn rearm_idle(&mut self) {
        if self.idle_armed {
            self.idle = Box::pin(tokio::time::sleep(self.idle_timeout));
        }
    }

    /// Absorb one byte chunk; append zero or more typed events to the
    /// outbox, or terminate the stream on a malformed/error frame.
    fn consume(&mut self, chunk: &[u8]) -> Result<(), ProviderError> {
        for parsed in self.parser.feed(chunk) {
            match parsed {
                Err(error) => {
                    // Malformed bytes/frames surface — nothing is silently
                    // skipped (`AI-MM-01`; `docs/12` §4.1).
                    self.finished = true;
                    return Err(error);
                }
                Ok(event) => self.step(event)?,
            }
        }
        Ok(())
    }

    /// One parsed stream frame. Progress events re-arm the idle window;
    /// `ping` never does (`TEST-AI-016`).
    fn step(&mut self, event: StreamEvent) -> Result<(), ProviderError> {
        match event {
            StreamEvent::Ping => Ok(()),
            StreamEvent::MessageStart { model, usage } => {
                if let Some(ran) = model {
                    if ran != self.requested_model {
                        self.finished = true;
                        return Err(ProviderError {
                            class: ErrorClass::ModelNotFound,
                            status: None,
                            message: format!(
                                "the gateway began model {ran:?} while the request named {:?}; the stream was stopped",
                                self.requested_model
                            ),
                        });
                    }
                }
                if let Some(usage) = usage {
                    self.usage.input = usage.input;
                    if usage.cache_read.is_some() {
                        self.usage.cache_read = usage.cache_read;
                    }
                    if usage.cache_write.is_some() {
                        self.usage.cache_write = usage.cache_write;
                    }
                }
                Ok(())
            }
            StreamEvent::BlockStart { index, block } => {
                self.rearm_idle();
                let kind = block
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                match kind {
                    "text" | "thinking" => Ok(()), // the deltas carry the content
                    "tool_use" => {
                        let id = block
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        self.tools.insert(
                            index,
                            ToolBlockState {
                                id: id.clone(),
                                name: name.clone(),
                                buffer: String::new(),
                            },
                        );
                        self.outbox.push_back(Ok(TurnEvent::ToolCall {
                            id,
                            name,
                            arguments_delta: None,
                            arguments: None,
                        }));
                        Ok(())
                    }
                    "redacted_thinking" => {
                        // The opaque block is preserved whole — dropped bytes
                        // cannot be replayed (`AI-ADAPTER-04`).
                        self.outbox
                            .push_back(Ok(TurnEvent::ReasoningRedacted { data: block }));
                        Ok(())
                    }
                    other => {
                        self.finished = true;
                        Err(ProviderError {
                            class: ErrorClass::UnsupportedModality,
                            status: None,
                            message: format!(
                                "the stream carried an assistant block of an unhandled kind: {other:?}"
                            ),
                        })
                    }
                }
            }
            StreamEvent::Delta { index, delta } => {
                self.rearm_idle();
                match delta {
                    ContentDelta::Text { text } => {
                        self.outbox.push_back(Ok(TurnEvent::TextDelta { text }));
                        Ok(())
                    }
                    ContentDelta::InputJson { partial_json } => {
                        if let Some(state) = self.tools.get_mut(&index) {
                            state.buffer.push_str(&partial_json);
                            self.outbox.push_back(Ok(TurnEvent::ToolCall {
                                id: state.id.clone(),
                                name: state.name.clone(),
                                arguments_delta: Some(partial_json),
                                arguments: None,
                            }));
                        }
                        Ok(())
                    }
                    ContentDelta::Thinking { thinking } => {
                        self.outbox.push_back(Ok(TurnEvent::ReasoningDelta {
                            text: thinking,
                            signature: None,
                        }));
                        Ok(())
                    }
                    ContentDelta::Signature { signature } => {
                        self.outbox.push_back(Ok(TurnEvent::ReasoningDelta {
                            text: String::new(),
                            signature: Some(signature),
                        }));
                        Ok(())
                    }
                    ContentDelta::Citation { citation } => {
                        self.outbox.push_back(Ok(TurnEvent::Citation { citation }));
                        Ok(())
                    }
                }
            }
            StreamEvent::BlockStop { index } => {
                if let Some(state) = self.tools.remove(&index) {
                    // Assembled arguments: a valid JSON payload (including an
                    // empty one) completes the call; anything else surfaces
                    // and nothing is executed (`docs/12` §5.2.
                    let arguments = if state.buffer.trim().is_empty() {
                        serde_json::Value::Object(serde_json::Map::new())
                    } else {
                        serde_json::from_str(&state.buffer).map_err(|error| {
                            self.finished = true;
                            ProviderError {
                                class: ErrorClass::InvalidRequest,
                                status: None,
                                message: format!(
                                    "the tool arguments for {:?} were not valid JSON; nothing was executed: {error}",
                                    state.name
                                ),
                            }
                        })?
                    };
                    self.outbox.push_back(Ok(TurnEvent::ToolCall {
                        id: state.id,
                        name: state.name,
                        arguments_delta: None,
                        arguments: Some(arguments),
                    }));
                }
                Ok(())
            }
            StreamEvent::MessageDelta {
                stop_reason, usage, ..
            } => {
                self.rearm_idle();
                if let Some(reason) = stop_reason {
                    self.stop = Some(reason);
                }
                // The final usage travels on message_delta: exact numbers,
                // never estimated (`AI-USAGE-01`; `TEST-AI-013`).
                if let Some(usage) = usage {
                    self.usage.output = usage.output;
                    if usage.cache_read.is_some() {
                        self.usage.cache_read = usage.cache_read;
                    }
                    if usage.cache_write.is_some() {
                        self.usage.cache_write = usage.cache_write;
                    }
                }
                Ok(())
            }
            StreamEvent::Error { kind, message } => {
                // The provider stopped the stream with its own error event:
                // preserve the class and the provider wording (`TEST-AI-021`
                // for stream errors; `docs/12` §4.1).
                self.finished = true;
                Err(ProviderError {
                    class: match kind.as_deref() {
                        Some("overloaded_error") => ErrorClass::Transient,
                        _ => ErrorClass::InvalidRequest,
                    },
                    status: None,
                    message: message
                        .unwrap_or_else(|| "the stream ended with an error event".to_string()),
                })
            }
            StreamEvent::MessageStop => {
                self.saw_message_stop = true;
                // The final usage is the exact number the provider reported
                // (`TEST-AI-013`; `docs/12` §4.2), then the preserved stop
                // reason (`TEST-AI-014`), then the typed end-of-stream.
                self.outbox.push_back(Ok(TurnEvent::Usage {
                    usage: self.usage.clone(),
                }));
                match self.stop.clone() {
                    Some(reason) => self.outbox.push_back(Ok(TurnEvent::Stop { reason })),
                    None => {
                        // A stream that ended without a stop reason is
                        // contract-malformed; no "end_turn" is invented.
                        self.outbox.push_back(Err(ProviderError {
                            class: ErrorClass::InvalidRequest,
                            status: None,
                            message: "the stream ended without a stop reason".to_string(),
                        }));
                    }
                }
                self.outbox.push_back(Ok(TurnEvent::Done));
                Ok(())
            }
        }
    }
}

impl Stream for TurnStreamImpl {
    type Item = Result<TurnEvent, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        loop {
            if let Some(event) = this.outbox.pop_front() {
                return Poll::Ready(Some(event));
            }
            if this.finished {
                return Poll::Ready(None);
            }
            // The idle window: armed governs; a fired timer is a visible
            // error, exactly `TEST-AI-016`.
            if this.idle_armed && matches!(this.idle.as_mut().poll(cx), Poll::Ready(())) {
                this.finished = true;
                return Poll::Ready(Some(Err(ProviderError {
                    class: ErrorClass::Transient,
                    status: None,
                    message: format!(
                        "the stream went idle for {} s without progress",
                        this.idle_timeout.as_secs().max(1)
                    ),
                })));
            }
            match this.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    this.finished = true;
                    if this.saw_message_stop {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(Err(ProviderError {
                        class: ErrorClass::Unreachable,
                        status: None,
                        message: "the stream ended before the model finished".to_string(),
                    })));
                }
                Poll::Ready(Some(chunk)) => match chunk {
                    Err(error) => {
                        this.finished = true;
                        return Poll::Ready(Some(Err(ProviderError {
                            class: ErrorClass::Unreachable,
                            status: None,
                            message: format!("the stream was interrupted: {error}"),
                        })));
                    }
                    Ok(chunk) => {
                        if let Err(error) = this.consume(&chunk) {
                            return Poll::Ready(Some(Err(error)));
                        }
                    }
                },
            }
        }
    }
}
