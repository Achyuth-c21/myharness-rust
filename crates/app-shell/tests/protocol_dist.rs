//! Integration test: the REAL built renderer (apps/desktop/dist) served through the
//! `app://` protocol implementation — the Phase 1 materialization of TEST-ARCH-003/004
//! against genuine production artifacts, not fixture files.
//!
//! Requires `npm install && npm run build` in apps/desktop; skips (with guidance) when
//! the dist is absent so the rest of the suite still reports.

use myharness_app_shell::protocol::{
    serve, AssetRoot, HttpRequest, RequestKind, StatusCode as Status,
};

fn dist_root() -> Option<AssetRoot> {
    let dist = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/dist")
        .canonicalize()
        .ok()?;
    Some(AssetRoot::new(dist))
}

fn nav(path: &str) -> HttpRequest {
    HttpRequest {
        path: path.to_string(),
        host: "localhost".to_string(),
        origin: None,
        kind: RequestKind::Navigation,
    }
}

fn fetch(path: &str, origin: &str) -> HttpRequest {
    HttpRequest {
        path: path.to_string(),
        host: "localhost".to_string(),
        origin: Some(origin.to_string()),
        kind: RequestKind::Fetch,
    }
}

#[test]
fn real_dist_serves_shell_routes_assets_and_gates() {
    let Some(root) = dist_root() else {
        eprintln!("SKIP: apps/desktop/dist missing — run `npm install && npm run build` in apps/desktop");
        return;
    };

    // Root serves index.html with CSP injected (docs/02 §4 gate 4).
    let home = serve(&root, "default-src 'self'", &nav("/"));
    assert_eq!(home.status, Status::Ok, "GET / should serve the shell");
    let html = String::from_utf8(home.body).unwrap();
    assert!(html.contains("<div id=\"root\">"), "index.html should boot React");
    assert!(
        home.headers.iter().any(|(k, _v)| *k == "content-security-policy"),
        "index.html must carry CSP"
    );

    // Client route → SPA fallback of index.html (docs/02 §4 gate on client routes).
    let client = serve(&root, "", &nav("/cowork/local_abc123"));
    assert_eq!(client.status, Status::Ok);

    // A real bundled asset serves with the right MIME.
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/dist/assets")
        .read_dir()
        .expect("dist/assets present");
    let js_files: Vec<String> = assets
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "js"))
        .map(|e| format!("/assets/{}", e.file_name().to_string_lossy()))
        .collect();
    assert!(!js_files.is_empty(), "dist should contain bundled js");
    let asset = serve(&root, "", &nav(&js_files[0]));
    assert_eq!(asset.status, Status::Ok);
    assert!(asset.headers.iter().any(|(k, v)| *k == "content-type" && v.contains("javascript")));

    // API path → typed 503 (docs/18).
    let api = serve(&root, "", &nav("/api/bootstrap"));
    assert_eq!(api.status, Status::NotAvailable);
    assert!(String::from_utf8(api.body).unwrap().contains("custom_3p_not_available"));

    // Guards on the real root: wrong host 404, bad origin 403.
    let mut wrong_host = nav("/");
    wrong_host.host = "evil.example".to_string();
    assert_eq!(serve(&root, "", &wrong_host).status, Status::NotFound);
    assert_eq!(serve(&root, "", &fetch("/", "https://evil")).status, Status::Forbidden);
}
