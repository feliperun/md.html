//! Phase 5 publish (ADR 0012, Option B): build and audit locally for fast
//! failure, then upload the canonical source plus every referenced local
//! asset to the Publish API as `multipart/form-data` over a hand-rolled
//! std-only HTTP/1.1 client, returning the server-issued public URL.

use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::build::BuildError;

/// The multipart `source` part filename (ADR 0012 contract).
const SOURCE_FILENAME: &str = "document.md";
const API_PATH: &str = "/v1/documents";
const ENV_PUBLISH_URL: &str = "MDHTML_PUBLISH_URL";

/// Build + audit the source, collect the exact referenced asset set, upload
/// both as multipart/form-data and return the server-issued public URL.
pub fn publish(source_path: &Path, base_url: Option<&str>) -> Result<String, BuildError> {
    let base_url = resolve_base_url(base_url)?;
    let endpoint = format!("{base_url}{API_PATH}");

    let source = fs::read_to_string(source_path).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is unreadable: {error}", source_path.display()),
        )
    })?;
    let source_dir = parent_dir(source_path);
    let (runtime_dir, themes_dir, fonts_dir) = crate::repo::repository_layout();
    let document =
        crate::build::build(&source, &source_dir, &runtime_dir, &themes_dir, &fonts_dir)?;
    let report = crate::audit::audit_artifact(&document);
    if !report.safe {
        return Err(BuildError::new("E-CLI-06", "security audit failed"));
    }

    let parsed = crate::frontmatter::parse_front_matter(&source)
        .map_err(|error| BuildError::new(error.code(), error.message().to_string()))?;
    let body = parsed.body.to_owned();
    let analysis = crate::analysis::analyze_document(&source);
    let mut assets = Vec::new();
    for path in crate::build::assets::collect_asset_paths(&body, &analysis) {
        let bytes = fs::read(source_dir.join(&path)).map_err(|error| {
            BuildError::new(
                "E-CLI-01",
                format!("asset '{path}' is unresolvable: {error}"),
            )
        })?;
        assets.push((path, bytes));
    }

    let boundary = random_boundary();
    let payload = multipart_payload(&source, &assets, &boundary);
    let (status, body) = http_post(&endpoint, &boundary, &payload)?;
    if status == 200 {
        let url = json_string_field(body.trim(), "url").ok_or_else(|| {
            BuildError::new("E-CLI-05", "publish response is missing a url field")
        })?;
        Ok(format!("{url}\n"))
    } else {
        Err(BuildError::new(
            "E-CLI-05",
            format!("publish failed (HTTP {status}): {}", body.trim()),
        ))
    }
}

/// The base URL: the `--url` argument wins, then `MDHTML_PUBLISH_URL`; a
/// missing URL is E-CLI-05 and only `http://` is in scope (https needs TLS).
/// Trailing slashes are stripped before `/v1/documents` is appended.
fn resolve_base_url(base_url: Option<&str>) -> Result<String, BuildError> {
    let base_url = match base_url {
        Some(url) => url.to_string(),
        None => match std::env::var(ENV_PUBLISH_URL) {
            Ok(url) if !url.is_empty() => url,
            _ => {
                return Err(BuildError::new(
                    "E-CLI-05",
                    "publish requires --url or MDHTML_PUBLISH_URL",
                ));
            }
        },
    };
    if !base_url.starts_with("http://") {
        return Err(BuildError::new(
            "E-CLI-05",
            "publish requires an http:// base URL (https is out of scope)",
        ));
    }
    Ok(base_url.trim_end_matches('/').to_string())
}

/// One `multipart/form-data` payload: a `source` part with the canonical
/// Markdown bytes, then one `asset` part per referenced path in discovery
/// order with the relative path as the filename.
fn multipart_payload(source: &str, assets: &[(String, Vec<u8>)], boundary: &str) -> Vec<u8> {
    let mut payload = Vec::new();
    push_part(
        &mut payload,
        boundary,
        "source",
        SOURCE_FILENAME,
        "text/plain; charset=utf-8",
        source.as_bytes(),
    );
    for (path, bytes) in assets {
        push_part(
            &mut payload,
            boundary,
            "asset",
            path,
            "application/octet-stream",
            bytes,
        );
    }
    payload.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    payload
}

fn push_part(
    payload: &mut Vec<u8>,
    boundary: &str,
    name: &str,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) {
    payload.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    payload.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    payload.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    payload.extend_from_slice(bytes);
    payload.extend_from_slice(b"\r\n");
}

/// A random multipart boundary from the process id and the current clock.
fn random_boundary() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("----mdhtml-{}-{nanos:x}", std::process::id())
}

/// One POST over std::net: a request with `Host`, `Content-Type`, the exact
/// `Content-Length`, and `Connection: close`; the response is read until EOF
/// and returned as `(status, body)`.
fn http_post(endpoint: &str, boundary: &str, payload: &[u8]) -> Result<(u16, String), BuildError> {
    let (host, port, path) = parse_endpoint(endpoint)?;
    let mut stream = TcpStream::connect((host.as_str(), port)).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("publish connection to {endpoint} failed: {error}"),
        )
    })?;
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: multipart/form-data; boundary={boundary}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(payload))
        .and_then(|()| stream.shutdown(Shutdown::Write))
        .map_err(|error| BuildError::new("E-CLI-05", format!("publish request failed: {error}")))?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).map_err(|error| {
        BuildError::new("E-CLI-05", format!("publish response failed: {error}"))
    })?;
    let text = String::from_utf8_lossy(&response);
    let (head, body) = match text.split_once("\r\n\r\n") {
        Some((head, body)) => (head, body),
        None => (text.as_ref(), ""),
    };
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    Ok((status, body.to_string()))
}

/// Split `http://authority/path` into its parts; the port defaults to 80.
fn parse_endpoint(endpoint: &str) -> Result<(String, u16, String), BuildError> {
    let rest = endpoint
        .strip_prefix("http://")
        .ok_or_else(|| BuildError::new("E-CLI-05", "publish requires an http:// base URL"))?;
    let (authority, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return Err(BuildError::new(
            "E-CLI-05",
            "publish requires an http:// base URL",
        ));
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => match port.parse::<u16>() {
            Ok(port) => (host.to_string(), port),
            Err(_) => (authority.to_string(), 80),
        },
        None => (authority.to_string(), 80),
    };
    Ok((host, port, path.to_string()))
}

/// The value of the first `"field": "value"` string in the success JSON,
/// decoding the escapes JSON allows inside a string.
fn json_string_field(json: &str, field: &str) -> Option<String> {
    let marker = format!("\"{field}\"");
    let after = json.split_once(marker.as_str())?.1;
    let after_colon = after.split_once(':')?.1.trim_start();
    let value = after_colon.strip_prefix('"')?;
    decode_json_string(value)
}

/// The decoded contents of a JSON string, starting right after the opening
/// `"` and reading up to (not including) the closing `"`; `None` if the
/// string is unterminated.
fn decode_json_string(value: &str) -> Option<String> {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(result),
            '\\' => result.push(decode_json_escape(&mut chars)?),
            ch => result.push(ch),
        }
    }
    None
}

/// One JSON string escape sequence, starting right after the `\`.
fn decode_json_escape(chars: &mut std::str::Chars<'_>) -> Option<char> {
    match chars.next()? {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '"' => Some('"'),
        '\\' => Some('\\'),
        other => Some(other),
    }
}

fn parent_dir(input: &Path) -> PathBuf {
    match input.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}
