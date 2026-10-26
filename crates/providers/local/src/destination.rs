//! The local destination gate (`AI-LOCAL-01..03`): where this wall may
//! connect, decided once at construction and pinned for the adapter's
//! lifetime. A local endpoint is not trusted for being local — it is
//! validated against an explicit loopback rule, with the name resolution
//! injected so the rebinding case is testable.

use std::net::IpAddr;
use std::sync::Arc;

use myharness_provider_sdk::{ErrorClass, ProviderError};

/// The loopback networks the destination gate accepts, spelled by
/// [`is_loopback`]: `127.0.0.0/8` and `::1`. A base URL may name a loopback
/// IP literal directly or `localhost` (resolved and pinned — see
/// [`Destination::resolve`]).
fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(), // 127.0.0.0/8
        IpAddr::V6(v6) => v6.is_loopback(), // ::1
    }
}

/// The documented default ports of the two supported runtimes, surfaced for
/// help text and egress derivation only — never as a fallback that widens
/// the gate.
pub const LOOPBACK_PORTS: [u16; 2] = [11434, 8080];

/// The injected name resolver. Production resolves through the std
/// resolver (`ToSocketAddrs` — a synchronous, bounded lookup the
/// constructor already is); tests inject canned answers so the rebinding
/// refusal is exercised without touching the host's resolver.
pub type ResolveFn = Arc<dyn Fn(&str) -> Result<Vec<IpAddr>, String> + Send + Sync>;

/// The production resolver: the std blocking resolver over the host's own
/// name service. The port is irrelevant to the address answer; resolution
/// failures surface as the gate's error, never as a fallback to "connect
/// anyway". Synchronous by design: the destination gate runs in the
/// adapter's sync constructor and must never start a runtime to work.
pub fn resolve_system(name: &str) -> Result<Vec<IpAddr>, String> {
    let target = format!("{name}:0");
    use std::net::ToSocketAddrs;
    target
        .to_socket_addrs()
        .map(|addrs| addrs.map(|a| a.ip()).collect::<Vec<_>>())
        .map_err(|error| format!("the local host {name:?} could not be resolved: {error}"))
}

/// A validated local destination: the pinned origin requests are issued
/// against (`scheme://<loopback-ip>:port`) and the paths this wall appends.
#[derive(Debug, Clone, PartialEq)]
pub struct Destination {
    pinned_origin: String,
}

impl Destination {
    /// Validate and pin a configured base URL (`AI-LOCAL-01..03`).
    ///
    /// - The host must be a loopback IP literal or `localhost`; anything
    ///   else (a LAN name, a public host, a bare path) refuses before any
    ///   bytes.
    /// - A hostname is resolved through `resolver`; **every** resolved
    ///   address must be loopback, and the first one pins the origin — a
    ///   name that re-resolves elsewhere later cannot move the destination
    ///   (DNS/IP rebinding is refused at construction).
    /// - The scheme must be `http` (loopback TLS is not part of this v1
    ///   contract and is not emulated); a `/v1` suffix is normalized off so
    ///   both base spellings address the same paths.
    /// - Empty falls to [`crate::DEFAULT_LOCAL_BASE`].
    pub fn resolve(base_url: &str, resolver: &ResolveFn) -> Result<Destination, ProviderError> {
        let base = base_url.trim().trim_end_matches('/');
        let base = if base.is_empty() {
            crate::DEFAULT_LOCAL_BASE
        } else {
            base
        };
        let base = base.strip_suffix("/v1").unwrap_or(base);
        let url = reqwest::Url::parse(base).map_err(|error| ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: format!("the local base URL is not a valid URL: {error}"),
        })?;
        if url.scheme() != "http" {
            return Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!(
                    "the local surface speaks plain http on loopback; scheme {:?} is refused",
                    url.scheme()
                ),
            });
        }
        let host = url.host_str().unwrap_or_default().to_string();
        if host.is_empty() {
            return Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: "the local base URL carries no host".to_string(),
            });
        }
        let port = url.port_or_known_default().unwrap_or(80);
        let pinned_ip: IpAddr = if let Ok(literal) = host.parse::<IpAddr>() {
            if !is_loopback(literal) {
                return Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: format!(
                        "the local wall connects to loopback only; {literal} is outside 127.0.0.0/8 and ::1"
                    ),
                });
            }
            literal
        } else if host == "localhost" {
            let resolved = resolver(&host).map_err(|message| ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message,
            })?;
            if resolved.is_empty() {
                return Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: "localhost resolved to no addresses; nothing was pinned".to_string(),
                });
            }
            if let Some(outside) = resolved.iter().find(|ip| !is_loopback(**ip)) {
                return Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: format!(
                        "localhost resolves to {outside}, which is not loopback; the destination is refused (DNS/IP rebinding guard)"
                    ),
                });
            }
            resolved[0]
        } else {
            return Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!(
                    "the local wall connects to loopback only; the host {host:?} is neither a loopback address nor \"localhost\""
                ),
            });
        };
        Ok(Destination {
            pinned_origin: format!("http://{pinned_ip}:{port}"),
        })
    }

    /// The pinned origin requests are issued against.
    pub fn origin(&self) -> &str {
        &self.pinned_origin
    }

    /// A URL under the pinned origin. The path is fixed by this wall —
    /// a request only ever goes to the pinned local endpoint.
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.pinned_origin, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn boxed_ok_resolver() -> ResolveFn {
        Arc::new(|_: &str| Ok(Vec::new()))
    }

    fn resolver_of(addrs: &[IpAddr]) -> ResolveFn {
        let addrs = addrs.to_vec();
        Arc::new(move |_| Ok(addrs.clone()))
    }

    fn failing_resolver() -> ResolveFn {
        Arc::new(|_| Err("resolution failed".to_string()))
    }

    /// `AI-LOCAL-01`: a loopback IP literal pins as-is; anything else refuses.
    #[test]
    fn loopback_literals_pin_and_others_refuse() {
        let any = boxed_ok_resolver();
        let ok = Destination::resolve("http://127.0.0.1:11434", &any).expect("loopback literal");
        assert_eq!(ok.origin(), "http://127.0.0.1:11434");

        let err = Destination::resolve("http://192.168.1.10:11434", &any).unwrap_err();
        assert!(err.message.contains("loopback only"), "{err:?}");
        let err = Destination::resolve("http://ollama.lan.example:11434", &any).unwrap_err();
        assert!(err.message.contains("loopback only"), "{err:?}");
    }

    /// `AI-LOCAL-02`: `localhost` resolving anywhere off-loopback is the
    /// rebinding case — refused, never followed.
    #[test]
    fn a_rebinding_localhost_refuses() {
        let rebound = resolver_of(&[
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)),
        ]);
        let err = Destination::resolve("http://localhost:11434", &rebound).unwrap_err();
        assert!(err.message.contains("rebinding"), "{err:?}");

        let fully_rebound = resolver_of(&[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))]);
        let err = Destination::resolve("http://localhost:11434", &fully_rebound).unwrap_err();
        assert!(err.message.contains("rebinding"), "{err:?}");
    }

    /// `AI-LOCAL-03`: a clean `localhost` pins to its resolved loopback
    /// address; a resolution failure refuses; an empty answer refuses.
    #[test]
    fn localhost_pins_to_the_resolved_loopback() {
        let good = resolver_of(&[IpAddr::V4(Ipv4Addr::LOCALHOST)]);
        let ok = Destination::resolve("http://localhost:11434", &good).expect("clean localhost");
        assert_eq!(ok.origin(), "http://127.0.0.1:11434");

        let err = Destination::resolve("http://localhost:11434", &failing_resolver()).unwrap_err();
        // The injected resolver's own error text travels — nothing is
        // reworded into a fallback.
        assert!(err.message.contains("resolution failed"), "{err:?}");
        let empty = resolver_of(&[]);
        let err = Destination::resolve("http://localhost:11434", &empty).unwrap_err();
        assert!(err.message.contains("no addresses"), "{err:?}");
    }

    /// Scheme and spelling normalization: https refuses on v1, a `/v1`
    /// suffix strips, empty falls to the default base.
    #[test]
    fn scheme_and_spelling_rules() {
        let any = boxed_ok_resolver();
        let err = Destination::resolve("https://127.0.0.1:11434", &any).unwrap_err();
        assert!(err.message.contains("plain http"), "{err:?}");

        let ok = Destination::resolve("http://127.0.0.1:11434/v1/", &any).expect("normalized");
        assert_eq!(ok.origin(), "http://127.0.0.1:11434");

        let ok = Destination::resolve("", &any).expect("default base");
        assert_eq!(ok.origin(), "http://127.0.0.1:11434");
    }
}
