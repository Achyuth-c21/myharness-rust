//! Process-level lifecycle policy: deep links and single instance (Phase 1, TEST-ARCH-002).
//!
//! The runtime lock is enforced by the Tauri binary (single-instance plugin; docs/02 §2
//! step 2: the primary acquires the lock, a second launch routes through it, otherwise
//! the new process quits). This module owns the *decision surface*: which deep links are
//! valid and what they resolve to. Route names follow docs/06-navigation-map.md §1.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceRole {
    Primary,
    Secondary,
}

/// Deep link targets allowed by the rebuild. Grammar: `myharness://<route>` — mirrors
/// the observed navigation grammar (docs/06); unknown schemes or routes are rejected
/// (docs/10 §8: deep links cannot open arbitrary local files/commands, TEST-SEC-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepLinkRoute {
    /// `/new` — default entry (docs/06: runtime entered /new).
    New,
    /// `/cowork/<session-id>` — opens the task surface.
    Cowork,
    /// `/cowork-artifact` — artifacts library.
    CoworkArtifact,
    /// `/projects`
    Projects,
    /// `/scheduled-task`
    ScheduledTask,
    /// `/epitaxy` — Code dashboard.
    Epitaxy,
    /// `/setup-desktop-3p` — separate setup window (opens via main, never the renderer).
    Setup3p,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepLinkError {
    UnrecognizedScheme,
    RouteNotAllowed,
}

pub fn classify_deep_link(uri: &str) -> Result<DeepLinkRoute, DeepLinkError> {
    // Grammar: scheme://<route>[/<arg>][?query] (docs/06 route table). The first path
    // segment is the route; everything after it is route-owned context validated by
    // the receiving surface (docs/18), never by this classifier.
    let (scheme, rest) = uri.split_once("://").ok_or(DeepLinkError::UnrecognizedScheme)?;
    if !scheme.eq_ignore_ascii_case("myharness") {
        return Err(DeepLinkError::UnrecognizedScheme);
    }
    let route_path = rest.split('?').next().unwrap_or(rest);
    let route = route_path
        .split('/')
        .next()
        .unwrap_or(route_path)
        .trim_matches('/');
    match route {
        "" | "new" => Ok(DeepLinkRoute::New),
        "cowork" => Ok(DeepLinkRoute::Cowork),
        "cowork-artifact" => Ok(DeepLinkRoute::CoworkArtifact),
        "projects" => Ok(DeepLinkRoute::Projects),
        "scheduled-task" => Ok(DeepLinkRoute::ScheduledTask),
        "epitaxy" => Ok(DeepLinkRoute::Epitaxy),
        "setup-desktop-3p" => Ok(DeepLinkRoute::Setup3p),
        _ => Err(DeepLinkError::RouteNotAllowed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_routes_resolve() {
        assert_eq!(classify_deep_link("myharness://new"), Ok(DeepLinkRoute::New));
        assert_eq!(classify_deep_link("myharness://"), Ok(DeepLinkRoute::New));
        assert_eq!(classify_deep_link("myharness://cowork/local_abc"), Ok(DeepLinkRoute::Cowork));
        assert_eq!(classify_deep_link("myharness://cowork-artifact"), Ok(DeepLinkRoute::CoworkArtifact));
        assert_eq!(classify_deep_link("MYHARNESS://projects"), Ok(DeepLinkRoute::Projects));
        assert_eq!(classify_deep_link("myharness://scheduled-task"), Ok(DeepLinkRoute::ScheduledTask));
        assert_eq!(classify_deep_link("myharness://epitaxy?focus=1"), Ok(DeepLinkRoute::Epitaxy));
        assert_eq!(classify_deep_link("myharness://setup-desktop-3p"), Ok(DeepLinkRoute::Setup3p));
    }

    #[test]
    fn unknown_schemes_and_routes_rejected() {
        assert_eq!(
            classify_deep_link("claude://new"),
            Err(DeepLinkError::UnrecognizedScheme)
        );
        assert_eq!(
            classify_deep_link("myharness://delete-everything"),
            Err(DeepLinkError::RouteNotAllowed)
        );
        assert_eq!(classify_deep_link("no-scheme"), Err(DeepLinkError::UnrecognizedScheme));
    }
}
