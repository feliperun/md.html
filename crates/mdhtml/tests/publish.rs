//! Phase 5 publish (ADR 0012, Option B): the std-only HTTP client uploads the
//! canonical source plus referenced assets to the Publish API and returns the
//! server-issued public URL. The integration tests run a local mock server;
//! production is never contacted.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

/// A throwaway source directory with `images/photo.png` and a front-matter
/// Markdown source that references the local image asset.
fn temp_source(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mdhtml-publish-{name}-{}", std::process::id()));
    fs::create_dir_all(dir.join("images")).expect("create images dir");
    fs::write(dir.join("images/photo.png"), b"photo-bytes").expect("write asset");
    let source = "---\ntitle: Publish test\n---\n\n![photo](images/photo.png)\n";
    let source_path = dir.join("document.md");
    fs::write(&source_path, source).expect("write source");
    source_path
}

#[test]
fn publish_uploads_source_and_assets_and_returns_the_server_url() {
    let source_path = temp_source("ok");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().expect("local addr").port();
    let expected_url = format!("http://127.0.0.1:{port}/AbCdEfGhIjKl");

    let server_expected_url = expected_url.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept one connection");
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).expect("read the raw request");
        let request = String::from_utf8_lossy(&raw);

        let (head, body) = request.split_once("\r\n\r\n").expect("request head");
        assert!(head.starts_with("POST /v1/documents HTTP/1.1"), "{head}");
        assert!(
            head.contains("Content-Type: multipart/form-data; boundary="),
            "{head}"
        );

        assert!(body.contains("title: Publish test"), "source bytes ride in the body");
        assert!(body.contains("photo-bytes"), "asset bytes ride in the body");
        assert!(
            body.contains("Content-Disposition: form-data; name=\"source\"; filename=\"document.md\""),
            "{body}"
        );
        assert!(
            body.contains("Content-Disposition: form-data; name=\"asset\"; filename=\"images/photo.png\""),
            "{body}"
        );

        let json = format!(
            "{{\"id\":\"AbCdEfGhIjKl\",\"url\":\"{server_expected_url}\",\"sha256\":\"sha\",\"mdhtmlVersion\":\"1.0\"}}"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
            json.len()
        );
        stream.write_all(response.as_bytes()).expect("write response");
    });

    let base_url = format!("http://127.0.0.1:{port}");
    let result = mdhtml::publish::publish(&source_path, Some(&base_url)).expect("publish succeeds");
    assert_eq!(result, format!("{expected_url}\n"));
    server.join().expect("server thread joins");
}

#[test]
fn publish_requires_a_base_url_or_env_var() {
    let source_path = temp_source("nourl");
    unsafe { std::env::remove_var("MDHTML_PUBLISH_URL") };
    let error = mdhtml::publish::publish(&source_path, None).expect_err("missing base url");
    assert_eq!(error.code(), "E-CLI-05");
    assert!(
        error.to_string().contains("--url or MDHTML_PUBLISH_URL"),
        "{}",
        error
    );
}

#[test]
fn publish_rejects_an_https_base_url() {
    let source_path = temp_source("https");
    let error = mdhtml::publish::publish(&source_path, Some("https://example.com"))
        .expect_err("https is out of scope");
    assert_eq!(error.code(), "E-CLI-05");
    assert!(error.to_string().contains("http://"), "{}", error);
}
