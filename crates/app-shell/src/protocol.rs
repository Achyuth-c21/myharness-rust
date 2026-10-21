//! `app://localhost` serving — Phase 1 (TEST-ARCH-003/004).
//!
//! Gates are copied from the verified evidence, docs/02-application-lifecycle.md §4:
//! 1. hostname must be `localhost`; other hosts return 404;
//! 2. an `Origin` header, if present, must be `app://localhost`; otherwise 403;
//! 3. path traversal outside the root returns 403;
//! 4. `index.html` is served with an injected CSP;
//! 5. API paths neither stubbed nor forwarded return typed 503 (docs/10 §5.2, docs/18).

use std::fs;
use std::path::{Component, Path, PathBuf};

use myharness_bridge::{is_api_path, NOT_AVAILABLE_BODY};

pub const SCHEME: &str = "app";
pub const HOST: &str = "localhost";
pub const ALLOWED_ORIGIN: &str = "app://localhost";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCode {
    Ok,
    Forbidden,
    NotFound,
    NotAvailable,
}

impl StatusCode {
    pub const fn as_u16(self) -> u16 {
        match self {
            StatusCode::Ok => 200,
            StatusCode::Forbidden => 403,
            StatusCode::NotFound => 404,
            StatusCode::NotAvailable => 503,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
    /// Top-frame navigations: pages loaded by the webview.
    Navigation,
    /// Subresource/API fetches: carry an `Origin` header.
    Fetch,
}

#[derive(Debug)]
pub struct HttpRequest {
    pub path: String,
    pub host: String,
    pub origin: Option<String>,
    pub kind: RequestKind,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResolveError {
    NotFound,
    OutsideRoot,
    Io,
}

pub struct AssetRoot {
    root: PathBuf,
}

impl AssetRoot {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Read `rel` under the static root. Rejects absolute paths, `..` beyond the
    /// root, backslashes, and — via canonicalization — symlink escapes.
    pub fn read(&self, rel: &str) -> Result<Vec<u8>, ResolveError> {
        let clean = sanitize(rel).ok_or(ResolveError::OutsideRoot)?;
        let joined = self.root.join(&clean);
        let target = fs::canonicalize(&joined).map_err(|e| io_kind_err(e.kind()))?;
        let root_canonical = fs::canonicalize(&self.root).map_err(|_| ResolveError::Io)?;
        if !target.starts_with(&root_canonical) {
            return Err(ResolveError::OutsideRoot);
        }
        if target.is_dir() {
            // A directory is not a document (report-bug-hunt C1: `/.` canonicalizes
            // to the root and `fs::read` would fail EISDIR, surfacing as the typed
            // 503 instead of the SPA fallback). Classify as missing.
            return Err(ResolveError::NotFound);
        }
        fs::read(&target).map_err(|e| io_kind_err(e.kind()))
    }
}

fn io_kind_err(kind: std::io::ErrorKind) -> ResolveError {
    match kind {
        std::io::ErrorKind::NotFound => ResolveError::NotFound,
        _ => ResolveError::Io,
    }
}

/// Path-clean with a hard root wall: `..` pops only when something is left to pop,
/// so `assets/../index.html` is allowed while `../etc/passwd` is rejected.
fn sanitize(rel: &str) -> Option<PathBuf> {
    if rel.is_empty() {
        return Some(PathBuf::new());
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return None;
    }
    if rel.contains('\\') {
        return None;
    }
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Normal(name) => out.push(name),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

/// Minimal percent-decoder for request paths. Unknown escapes, control bytes and
/// backslashes are rejected outright (bogus request, not a literal filename).
fn percent_decode_path(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hi = *bytes.get(i + 1)?;
                let lo = *bytes.get(i + 2)?;
                out.push(hex_val(hi)? * 16 + hex_val(lo)?);
                i += 3;
            }
            b'\\' | 0x00..=0x1f | 0x7f => return None,
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + b - b'a'),
        b'A'..=b'F' => Some(10 + b - b'A'),
        _ => None,
    }
}

fn has_file_extension(path: &str) -> bool {
    let last = path.rsplit('/').next().unwrap_or(path);
    match last.split_once('.') {
        Some((stem, ext)) => !stem.is_empty() && !ext.is_empty(),
        None => false,
    }
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" | "cjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" | "otf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn html_response(status: StatusCode, body: &[u8]) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![("content-type", "text/html; charset=utf-8".to_string())],
        body: body.to_vec(),
    }
}

fn json_response(status: StatusCode, body: &str) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![("content-type", "application/json".to_string())],
        body: body.as_bytes().to_vec(),
    }
}

/// Serve one `app://` request per the verified gates (docs/02 §4).
/// `csp` is the produced Content-Security-Policy for HTML responses.
pub fn serve(root: &AssetRoot, csp: &str, req: &HttpRequest) -> HttpResponse {
    if !req.host.eq_ignore_ascii_case(HOST) {
        return html_response(StatusCode::NotFound, b"<html><body>404</body></html>");
    }
    if let Some(origin) = &req.origin {
        if !origin.eq_ignore_ascii_case(ALLOWED_ORIGIN) {
            return html_response(StatusCode::Forbidden, b"<html><body>403</body></html>");
        }
    }
    let path = match percent_decode_path(&req.path) {
        Some(p) => p,
        None => return html_response(StatusCode::Forbidden, b"<html><body>403</body></html>"),
    };
    if is_api_path(&path) {
        // Endpoint table is empty in Phase 1: stubs land per later phases.
        return json_response(StatusCode::NotAvailable, NOT_AVAILABLE_BODY);
    }
    let rel = path.trim_start_matches('/');
    if rel.is_empty() {
        return serve_index(root, csp);
    }
    match root.read(rel) {
        Ok(body) => {
            let mut headers = Vec::new();
            headers.push(("content-type", mime_for(rel).to_string()));
            // C3 (report-bug-hunt): match documents case-insensitively — an
            // `.HTML` document must still ship with the CSP.
            let lower = rel.to_ascii_lowercase();
            if lower.ends_with(".html") || lower.ends_with(".htm") {
                headers.push(("content-security-policy", csp.to_string()));
            }
            HttpResponse {
                status: StatusCode::Ok,
                headers,
                body,
            }
        }
        Err(ResolveError::OutsideRoot) => html_response(StatusCode::Forbidden, b"<html><body>403</body></html>"),
        Err(ResolveError::NotFound) => {
            // C4 (report-bug-hunt): the Navigation/Fetch distinction now has an
            // effect — the SPA fallback answers only top-frame navigations (a
            // fetch of an unknown extensionless path is a missing resource, not a
            // document, and must not receive the app shell HTML with a 200).
            if req.kind == RequestKind::Navigation && !has_file_extension(rel) {
                serve_index(root, csp)
            } else {
                html_response(StatusCode::NotFound, b"<html><body>404</body></html>")
            }
        }
        // C2 residual (report-bug-hunt): genuine filesystem faults land in the
        // typed 503 envelope — the only contractually defined protocol error body
        // (docs/18), so no alternative kind exists to faithfully label a host FS
        // failure. Recorded as an unknown in docs/14 §9 rather than invented.
        Err(ResolveError::Io) => json_response(StatusCode::NotAvailable, NOT_AVAILABLE_BODY),
    }
}

/// index.html fallback with injected CSP; a missing dist still answers the typed
/// envelope so the shell cannot crash on packaging problems (docs/02 §3: the setup
/// window must stay reachable — full setup-window wiring lands in Phase 2).
fn serve_index(root: &AssetRoot, csp: &str) -> HttpResponse {
    match root.read("index.html") {
        Ok(body) => HttpResponse {
            status: StatusCode::Ok,
            headers: vec![
                ("content-type", "text/html; charset=utf-8".to_string()),
                ("content-security-policy", csp.to_string()),
                ("cache-control", "no-store".to_string()),
            ],
            body,
        },
        Err(_) => json_response(StatusCode::NotAvailable, NOT_AVAILABLE_BODY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSP: &str = "default-src 'self'; object-src 'none'";

    fn temp_root(files: &[(&str, &str)]) -> AssetRoot {
        let dir = std::env::temp_dir().join(format!(
            "mh-proto-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for (name, content) in files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, content).unwrap();
        }
        AssetRoot::new(dir)
    }

    fn req(path: &str) -> HttpRequest {
        HttpRequest {
            path: path.to_string(),
            host: HOST.to_string(),
            origin: None,
            kind: RequestKind::Navigation,
        }
    }

    #[test]
    fn wrong_host_is_404_not_403() {
        let root = temp_root(&[("index.html", "<html>")]);
        let mut re = req("/");
        re.host = "evil.example".to_string();
        assert_eq!(serve(&root, CSP, &re).status, StatusCode::NotFound);
    }

    #[test]
    fn bad_origin_is_403_navigation_without_origin_ok() {
        let root = temp_root(&[("index.html", "<html>")]);
        let mut re = req("/");
        re.kind = RequestKind::Fetch;
        re.origin = Some("https://evil".to_string());
        assert_eq!(serve(&root, CSP, &re).status, StatusCode::Forbidden);
        assert_eq!(serve(&root, CSP, &req("/")).status, StatusCode::Ok);
    }

    #[test]
    fn traversal_is_403_within_root_steps_ok() {
        let root = temp_root(&[("index.html", "<html>")]);
        assert_eq!(serve(&root, CSP, &req("/../etc/passwd")).status, StatusCode::Forbidden);
        assert_eq!(serve(&root, CSP, &req("/%2e%2e/x")).status, StatusCode::Forbidden);
        assert_eq!(serve(&root, CSP, &req("/a\\b")).status, StatusCode::Forbidden);
        assert_eq!(serve(&root, CSP, &req("/assets/../index.html")).status, StatusCode::Ok);
    }

    #[test]
    fn api_paths_return_typed_503() {
        let root = temp_root(&[("index.html", "<html>")]);
        let resp = serve(&root, CSP, &req("/api/bootstrap"));
        assert_eq!(resp.status, StatusCode::NotAvailable);
        assert!(String::from_utf8(resp.body).unwrap().contains("custom_3p_not_available"));
    }

    #[test]
    fn spa_fallback_and_assets() {
        let root = temp_root(&[
            ("index.html", "<html>SHELL</html>"),
            ("assets/app.js", "console.log(1)"),
        ]);
        assert_eq!(serve(&root, CSP, &req("/cowork")).status, StatusCode::Ok);
        let js = serve(&root, CSP, &req("/assets/app.js"));
        assert_eq!(js.status, StatusCode::Ok);
        assert_eq!(js.headers[0].0, "content-type");
        assert_eq!(serve(&root, CSP, &req("/assets/nope.js")).status, StatusCode::NotFound);
    }

    #[test]
    fn index_html_gets_csp_header() {
        let root = temp_root(&[("index.html", "<html>SHELL</html>")]);
        let resp = serve(&root, CSP, &req("/"));
        assert_eq!(resp.status, StatusCode::Ok);
        assert!(resp.headers.iter().any(|(k, v)| (*k == "content-security-policy") && v == CSP));
    }

    /// report-bug-hunt C1: a path that resolves to a directory (e.g. `/.`) is
    /// missing, not a filesystem fault — it takes the SPA fallback like any other
    /// unknown route instead of the typed 503.
    #[test]
    fn directory_paths_take_the_spa_fallback() {
        let root = temp_root(&[
            ("index.html", "<html>SHELL</html>"),
            ("assets/sub/file.txt", "x"),
        ]);
        assert_eq!(serve(&root, CSP, &req("/.")).status, StatusCode::Ok);
        // the directory body IS the shell, with the CSP injected
        let resp = serve(&root, CSP, &req("/."));
        assert!(String::from_utf8(resp.body).unwrap().contains("SHELL"));
        assert_eq!(serve(&root, CSP, &req("/assets/sub")).status, StatusCode::Ok);
    }

    /// report-bug-hunt C3: document CSP attachment is case-insensitive.
    #[test]
    fn csp_header_is_attached_case_insensitively() {
        let root = temp_root(&[("page.HTML", "<html>")]);
        let resp = serve(&root, CSP, &req("/page.HTML"));
        assert_eq!(resp.status, StatusCode::Ok);
        assert!(resp.headers.iter().any(|(k, _)| *k == "content-security-policy"));
    }

    /// report-bug-hunt C4: `kind` is consumed — only navigations get the SPA
    /// fallback; fetches of unknown extensionless paths are simple 404s.
    #[test]
    fn fetch_unknown_route_is_404_not_the_shell() {
        let root = temp_root(&[("index.html", "<html>SHELL</html>")]);
        let mut fetch = req("/cowork");
        fetch.kind = RequestKind::Fetch;
        let resp = serve(&root, CSP, &fetch);
        assert_eq!(resp.status, StatusCode::NotFound);
        assert!(!String::from_utf8(resp.body).unwrap().contains("SHELL"));
        // unchanged navigation behavior:
        assert_eq!(serve(&root, CSP, &req("/cowork")).status, StatusCode::Ok);
    }

    #[test]
    fn empty_dist_still_answers() {
        let root = AssetRoot::new(std::env::temp_dir().join("mh-proto-missing-dist"));
        assert_eq!(serve(&root, CSP, &req("/cowork")).status, StatusCode::NotAvailable);
    }
}
