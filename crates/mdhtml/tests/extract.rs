//! T14 extract (CLI-03): byte-exact canonical-source restoration, strict
//! asset validation with fail-before-write semantics, and the deterministic
//! build → extract round-trip over the shared fixtures.

use std::fs;
use std::path::PathBuf;

use mdhtml::build::build;
use mdhtml::extract::{extract_assets, extract_source};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn fixture_dir() -> PathBuf {
    repo_root().join("fixtures")
}

fn runtime_dist() -> PathBuf {
    repo_root().join("runtime/dist")
}

fn themes_dir() -> PathBuf {
    repo_root().join("themes")
}

fn fonts_dir() -> PathBuf {
    repo_root().join("fonts")
}

fn build_source(source: &str) -> String {
    build(
        source,
        &fixture_dir(),
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect("fixture builds")
}

/// A minimal artifact with one canonical source script and the given asset
/// blocks, each `(data-path, data-type, payload)`.
fn artifact_with_assets(blocks: &[(&str, &str, &str)]) -> String {
    let mut html = String::from(
        "<!doctype html>\n<html lang=\"en\" data-mdhtml=\"1.0\" \
         data-mdhtml-portable=\"true\">\n<head></head>\n<body>\n  \
         <script id=\"mdhtml-source\" type=\"text/markdown\">---\ntitle: Inline\n---\n\n# Body\n</script>\n",
    );
    for (path, data_type, payload) in blocks {
        html.push_str(&format!(
            "  <script type=\"application/octet-stream\" data-path=\"{path}\" \
             data-type=\"{data_type}\">{payload}</script>\n"
        ));
    }
    html.push_str("  <script id=\"mdhtml-runtime\">/* runtime */</script>\n</body>\n</html>\n");
    html
}

#[test]
fn roundtrip_source_is_byte_identical_to_the_original() {
    let original = fs::read(fixture_dir().join("extract-roundtrip.md")).expect("read fixture");
    let source = String::from_utf8(original.clone()).expect("fixture is UTF-8");
    let html = build_source(&source);

    assert_eq!(
        extract_source(html.as_bytes()).expect("extracts"),
        original,
        "build → extract must restore the source byte-for-byte"
    );
}

#[test]
fn roundtrip_crlf_source_is_byte_identical_to_the_original() {
    let original = "---\r\ntitle: CRLF round-trip\r\n---\r\n\r\n# CRLF body\r\n\r\nUnicode survives: Olá — 日本語 — 🎉\r\n\r\nThe escaped terminator stays verbatim: <\\/script is not decoded.\r\n";
    let html = build_source(original);

    assert_eq!(
        extract_source(html.as_bytes()).expect("extracts"),
        original.as_bytes(),
        "build → extract must restore a CRLF source byte-for-byte"
    );
}

#[test]
fn extracted_source_keeps_unicode_escapes_and_newlines_verbatim() {
    let source =
        fs::read_to_string(fixture_dir().join("extract-roundtrip.md")).expect("read fixture");
    let html = build_source(&source);
    let extracted = String::from_utf8(extract_source(html.as_bytes()).expect("extracts"))
        .expect("extracted source is UTF-8");

    assert!(extracted.contains("Olá — 日本語 — 🎉"));
    assert!(
        extracted.contains("<\\/script"),
        "the escape is kept verbatim"
    );
    assert!(
        !extracted.contains("</script"),
        "the escape is never decoded"
    );
    assert!(extracted.starts_with("---\ntitle: Extract round-trip"));
    assert!(extracted.ends_with('\n'));
}

#[test]
fn assets_roundtrip_byte_exactly_with_paths_and_types() {
    let source =
        fs::read_to_string(fixture_dir().join("extract-roundtrip.md")).expect("read fixture");
    let html = build_source(&source);
    let assets = extract_assets(html.as_bytes()).expect("extracts");

    assert_eq!(
        assets.len(),
        2,
        "svg (md + html image) and css (fonts) embed once"
    );
    assert_eq!(assets[0].path, "asset-tiny.svg");
    assert_eq!(assets[0].data_type, "image/svg+xml");
    assert_eq!(
        assets[0].bytes,
        fs::read(fixture_dir().join("asset-tiny.svg")).expect("read svg")
    );
    assert_eq!(assets[1].path, "asset-tiny.css");
    assert_eq!(assets[1].data_type, "text/css");
    assert_eq!(
        assets[1].bytes,
        fs::read(fixture_dir().join("asset-tiny.css")).expect("read css")
    );
}

#[test]
fn payloads_with_embedded_whitespace_decode_like_the_compact_form() {
    let html = artifact_with_assets(&[("x.png", "image/png", "SG Vs\nbG8=\r\n")]);
    let assets = extract_assets(html.as_bytes()).expect("whitespace is ignored");
    assert_eq!(assets[0].bytes, b"Hello");
}

#[test]
fn invalid_base64_fails_with_e_cli_03() {
    let html = fs::read(fixture_dir().join("extract-invalid.md.html")).expect("read fixture");
    let error = extract_assets(&html).expect_err("invalid payload");
    assert_eq!(error.code, "E-CLI-03");
    assert!(error.message.contains("images/broken.png"));
}

#[test]
fn unsafe_paths_fail_with_e_cli_03() {
    let paths = [
        "../up.png",
        "a/../../b.png",
        "/absolute.png",
        "https://example.test/a.png",
        "data:image/png;base64,AAAA",
    ];
    for path in paths {
        let html = artifact_with_assets(&[(path, "image/png", "SGVsbG8=")]);
        let error =
            extract_assets(html.as_bytes()).expect_err(&format!("unsafe path must fail: {path}"));
        assert_eq!(error.code, "E-CLI-03", "{path}");
    }
}

#[test]
fn duplicate_data_paths_fail_with_e_cli_03() {
    let html = artifact_with_assets(&[
        ("images/dup.png", "image/png", "SGVsbG8="),
        ("images/dup.png", "image/png", "SGVsbG8="),
    ]);
    let error = extract_assets(html.as_bytes()).expect_err("duplicate path");
    assert_eq!(error.code, "E-CLI-03");
    assert!(error.message.contains("images/dup.png"));
}

#[test]
fn missing_data_path_or_type_fails_with_e_cli_03() {
    let html = artifact_with_assets(&[("", "image/png", "SGVsbG8=")]);
    let error = extract_assets(html.as_bytes()).expect_err("empty path");
    assert_eq!(error.code, "E-CLI-03");

    let html = artifact_with_assets(&[("x.png", "", "SGVsbG8=")]);
    let error = extract_assets(html.as_bytes()).expect_err("empty type");
    assert_eq!(error.code, "E-CLI-03");
}

#[test]
fn asset_block_without_attributes_fails_with_e_cli_03() {
    let html = "<!doctype html>\n<html data-mdhtml=\"1.0\"><body>\n  \
        <script id=\"mdhtml-source\" type=\"text/markdown\"># X\n</script>\n  \
        <script type=\"application/octet-stream\">SGVsbG8=</script>\n</body></html>\n";
    let error = extract_assets(html.as_bytes()).expect_err("missing attributes");
    assert_eq!(error.code, "E-CLI-03");
}

#[test]
fn source_requires_exactly_one_mdhtml_source_script() {
    let without = "<!doctype html>\n<html data-mdhtml=\"1.0\"><body>\n  <script id=\"mdhtml-runtime\">x</script>\n</body></html>\n";
    let error = extract_source(without.as_bytes()).expect_err("missing source");
    assert_eq!(error.code, "E-FMT-01");

    let duplicated = artifact_with_assets(&[]).replace(
        "<script id=\"mdhtml-runtime\">",
        "<script id=\"mdhtml-source\" type=\"text/markdown\"># Dup\n</script>\
         <script id=\"mdhtml-runtime\">",
    );
    let error = extract_source(duplicated.as_bytes()).expect_err("duplicate source");
    assert_eq!(error.code, "E-FMT-01");

    let wrong_type = artifact_with_assets(&[]).replace(
        "<script id=\"mdhtml-source\" type=\"text/markdown\">",
        "<script id=\"mdhtml-source\" type=\"text/plain\">",
    );
    let error = extract_source(wrong_type.as_bytes()).expect_err("wrong type");
    assert_eq!(error.code, "E-FMT-01");
}
