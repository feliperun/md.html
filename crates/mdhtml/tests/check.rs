//! T13 check: deterministic source and artifact reports with diagnostics,
//! portability verdict, external-request count, and byte budgets (CLI-02,
//! FMT-03, §16, §18).

use std::fs;
use std::path::PathBuf;

use mdhtml::build::build;
use mdhtml::check::{Budgets, CheckReport, check_artifact, check_source};
use mdhtml::selection::fonts::{load as load_catalog, select_faces};
use mdhtml::selection::{load, select_fragments};

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

fn fixture_source(name: &str) -> String {
    fs::read_to_string(fixture_dir().join(name)).expect("read fixture")
}

fn check_source_str(source: &str) -> CheckReport {
    check_source(source, &runtime_dist(), &fonts_dir())
}

fn build_source(source: &str) -> String {
    build(
        source,
        &fixture_dir(),
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect("builds")
}

fn expected_runtime_bytes(source: &str) -> usize {
    let manifest = load(&runtime_dist()).expect("committed manifest loads");
    let analysis = mdhtml::analysis::analyze_document(source);
    let body = mdhtml::frontmatter::parse_front_matter(source)
        .expect("valid front matter")
        .body
        .to_owned();
    select_fragments(&body, &analysis, &manifest)
        .iter()
        .map(|id| {
            manifest
                .fragments
                .iter()
                .find(|fragment| fragment.id == *id)
                .expect("selection returns manifest fragments")
                .size as usize
        })
        .sum()
}

fn expected_font_bytes(source: &str) -> usize {
    let catalog = load_catalog(&fonts_dir().join("catalog.json")).expect("committed catalog loads");
    let analysis = mdhtml::analysis::analyze_document(source);
    let body = mdhtml::frontmatter::parse_front_matter(source)
        .expect("valid front matter")
        .body
        .to_owned();
    select_faces(&analysis, &body, &catalog)
        .iter()
        .map(|face| face.bytes as usize)
        .sum()
}

fn between<'a>(html: &'a str, open: &str, close: &str) -> &'a str {
    let start = html.find(open).expect("open marker") + open.len();
    let end = html[start..].find(close).expect("close marker") + start;
    &html[start..end]
}

/// The ordered (path, mime, payload) of every embedded asset block.
fn asset_blocks(html: &str) -> Vec<(String, String, String)> {
    let marker = "<script type=\"application/octet-stream\" ";
    let mut blocks = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find(marker) {
        let after = &rest[start + marker.len()..];
        let open = after.find('>').expect("tag close") + 1;
        let close = after[open..].find("</script>").expect("block close") + open;
        let tag = &after[..open - 1];
        blocks.push((
            attr(tag, "data-path"),
            attr(tag, "data-type"),
            after[open..close].to_string(),
        ));
        rest = &after[close + "</script>".len()..];
    }
    blocks
}

fn attr<'a>(tag: &'a str, name: &str) -> String {
    let marker = format!("{name}=\"");
    let start = tag.find(&marker).expect("attribute") + marker.len();
    let end = tag[start..].find('"').expect("attribute close") + start;
    tag[start..end].to_string()
}

fn decode_base64(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for ch in text.chars() {
        let value = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => break,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
        }
    }
    bytes
}

/// Sum of the decoded font data URIs inside a fonts style block.
fn decoded_font_bytes(css: &str) -> usize {
    let mut total = 0;
    let mut rest = css;
    while let Some(start) = rest.find("base64,") {
        let after = &rest[start + "base64,".len()..];
        let end = after.find(')').unwrap_or(after.len());
        total += decode_base64(&after[..end]).len();
        rest = &after[end..];
    }
    total
}

fn codes(report: &CheckReport) -> Vec<&str> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn portable_fixture_reports_a_clean_portable_verdict() {
    let source = fixture_source("check-portable.md");
    let report = check_source_str(&source);

    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert!(!report.has_errors());
    assert!(report.portable, "hyperlinks must not affect the verdict");
    assert_eq!(report.requests, 0);
    assert_eq!(
        report.budgets,
        Budgets {
            content: source.len(),
            runtime: expected_runtime_bytes(&source),
            fonts: expected_font_bytes(&source),
            images: 0,
        }
    );
    assert_eq!(
        report.render(),
        format!(
            "mdhtml: I-CLI-02: portable: true; requests: 0; content: {} bytes; \
             runtime: {} bytes; fonts: {} bytes; images: 0 bytes\n",
            source.len(),
            expected_runtime_bytes(&source),
            expected_font_bytes(&source),
        )
    );
}

#[test]
fn report_fixture_emits_ordered_diagnostics_and_closes_with_the_verdict() {
    let source = fixture_source("check-report.md");
    let report = check_source_str(&source);

    assert_eq!(codes(&report), ["E-SECT-01", "W-COMP-02", "W-COMP-02"]);
    assert!(report.has_errors());
    assert!(report.portable);
    assert_eq!(report.requests, 0);

    let text = report.render();
    assert!(text.contains("mdhtml: E-SECT-01: sections key has no matching heading slug\n"));
    assert!(text.contains("mdhtml: W-COMP-02: unknown section component\n"));
    assert!(text.contains("mdhtml: W-COMP-02: unknown container name\n"));
    assert!(text.ends_with(&format!(
        "mdhtml: I-CLI-02: portable: true; requests: 0; content: {} bytes; \
         runtime: {} bytes; fonts: {} bytes; images: 0 bytes\n",
        source.len(),
        expected_runtime_bytes(&source),
        expected_font_bytes(&source),
    )));
}

#[test]
fn warning_only_source_has_no_errors() {
    let source = "---\ntitle: Warnings only\n---\n\n::: not-a-container\nBody.\n:::\n";
    let report = check_source_str(source);
    assert_eq!(codes(&report), ["W-COMP-02"]);
    assert!(!report.has_errors());
    assert!(report.portable);
    assert_eq!(report.requests, 0);
}

#[test]
fn fonts_url_marks_the_source_non_portable_with_external_requests() {
    let source = "---\ntitle: Online fonts\nfonts:\n  url: https://fonts.googleapis.com/css2?family=Instrument+Sans\n---\n# Body\n";
    let report = check_source_str(source);
    assert!(!report.portable);
    assert_eq!(report.requests, 2, "stylesheet and font origins");
    assert!(report.render().contains("portable: false; requests: 2;"));
}

#[test]
fn external_image_origin_marks_the_source_non_portable() {
    let source = "---\ntitle: External image\n---\n\n![x](https://cdn.example.test/a.png)\n";
    let report = check_source_str(source);
    assert!(!report.portable);
    assert_eq!(report.requests, 1);
}

#[test]
fn hyperlinks_do_not_affect_the_verdict() {
    let source = "---\ntitle: Links\n---\n\n[site](https://example.test)\n";
    let report = check_source_str(source);
    assert!(report.portable);
    assert_eq!(report.requests, 0);
}

#[test]
fn artifact_report_reads_budgets_from_the_document() {
    let source = "---\ntitle: Artifact\n---\n\n# Heading\n\n![tiny](asset-tiny.svg)\n\nCode: `x`\n";
    let html = build_source(source);
    let report = check_artifact(&html);

    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
    assert!(!report.has_errors());
    assert!(report.portable);
    assert_eq!(report.requests, 0);
    assert_eq!(report.budgets.content, source.len());
    assert_eq!(
        report.budgets.runtime,
        between(&html, "<script id=\"mdhtml-runtime\">", "</script>").len()
    );
    assert_eq!(
        report.budgets.fonts,
        decoded_font_bytes(&between(&html, "<style id=\"mdhtml-fonts\">", "</style>"))
    );
    let blocks = asset_blocks(&html);
    assert_eq!(blocks.len(), 1);
    assert_eq!(report.budgets.images, decode_base64(&blocks[0].2).len());
}

#[test]
fn artifact_with_missing_embedded_asset_warns_w_ui_04() {
    let source = "---\ntitle: Missing asset\n---\n\n![tiny](asset-tiny.svg)\n";
    let html = build_source(source);
    let stripped = remove_asset_blocks(&html);
    let report = check_artifact(&stripped);

    assert_eq!(codes(&report), ["W-UI-04"]);
    assert!(!report.has_errors());
    assert!(
        report
            .render()
            .contains("mdhtml: W-UI-04: asset 'asset-tiny.svg' is referenced but not embedded\n")
    );
    assert_eq!(report.budgets.images, 0);
}

#[test]
fn artifact_with_external_image_contradicts_the_declared_attribute() {
    let source = "---\ntitle: External image\n---\n\n![x](https://cdn.example.test/a.png)\n";
    let html = build_source(source);
    let report = check_artifact(&html);

    assert!(!report.portable);
    assert_eq!(report.requests, 1);
    assert_eq!(codes(&report), ["E-FMT-03"]);
    assert!(
        report
            .render()
            .contains("data-mdhtml-portable contradicts the content verdict")
    );
}

#[test]
fn artifact_with_wrong_csp_reports_e_fmt_03() {
    let source = "---\ntitle: Tampered\n---\n\n# Body\n";
    let html = build_source(source);
    let tampered = html.replace(
        "script-src 'sha256-",
        "script-src 'sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    );
    let report = check_artifact(&tampered);

    assert!(report.portable, "the CSP meta itself is not a subresource");
    assert!(codes(&report).contains(&"E-FMT-03"));
}

#[test]
fn fonts_url_artifact_is_consistently_non_portable() {
    let source = "---\ntitle: Online fonts\nfonts:\n  url: https://fonts.googleapis.com/css2?family=Instrument+Sans\n---\n# Body\n";
    let html = build_source(source);
    let report = check_artifact(&html);

    assert!(!report.portable);
    assert_eq!(report.requests, 2);
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
}

#[test]
fn artifacts_declare_the_safe_attestation_and_unsafe_builds_still_check_clean() {
    let safe_source = "---\ntitle: Safe\n---\n# Body\n";
    let safe = build(
        &safe_source,
        &fixture_dir(),
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect("safe build");
    assert!(
        safe.contains("data-mdhtml-safe=\"true\""),
        "every safe build attests data-mdhtml-safe=\"true\""
    );
    let safe_report = check_artifact(&safe);
    assert!(!safe_report.has_errors(), "{:?}", safe_report.diagnostics);
    assert!(
        safe_report.diagnostics.is_empty(),
        "{:?}",
        safe_report.diagnostics
    );

    let unsafe_source = "---\ntitle: Unsafe\n---\n\n[click](javascript:alert(1))\n";
    let unsafe_html = mdhtml::build::build_unsafe(
        &unsafe_source,
        &fixture_dir(),
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect("unsafe build");
    assert!(
        unsafe_html.contains("data-mdhtml-safe=\"false\""),
        "an unsafe build attests data-mdhtml-safe=\"false\""
    );
    let unsafe_report = check_artifact(&unsafe_html);
    assert!(
        !unsafe_report.has_errors(),
        "mdhtml check stays green on an unsafe artifact: {:?}",
        unsafe_report.diagnostics
    );
}

#[test]
fn artifact_missing_the_source_script_reports_e_fmt_01() {
    let html = "<!doctype html>\n<html lang=\"en\" data-mdhtml=\"1.0\" data-mdhtml-portable=\"true\">\n<head></head>\n<body></body>\n</html>\n";
    let report = check_artifact(html);
    assert!(codes(&report).contains(&"E-FMT-01"));
    assert!(report.has_errors());
}

#[test]
fn artifact_with_duplicate_source_id_reports_e_fmt_01() {
    let source = "---\ntitle: Dup\n---\n# Body\n";
    let html = build_source(source);
    let duplicated = html.replace(
        "<script id=\"mdhtml-runtime\">",
        "<script id=\"mdhtml-source\" type=\"text/markdown\"># Body\n</script><script id=\"mdhtml-runtime\">",
    );
    let report = check_artifact(&duplicated);
    assert!(codes(&report).contains(&"E-FMT-01"));
}

#[test]
fn artifact_with_wrong_source_type_reports_e_fmt_01() {
    let source = "---\ntitle: Wrong type\n---\n# Body\n";
    let html = build_source(source);
    let tampered = html.replace(
        "<script id=\"mdhtml-source\" type=\"text/markdown\">",
        "<script id=\"mdhtml-source\" type=\"text/plain\">",
    );
    let report = check_artifact(&tampered);
    assert!(codes(&report).contains(&"E-FMT-01"));
}

#[test]
fn artifact_without_root_identity_reports_e_fmt_01() {
    let source = "---\ntitle: No root\n---\n# Body\n";
    let html = build_source(source);
    let tampered = html.replace(" data-mdhtml=\"1.0\"", "");
    let report = check_artifact(&tampered);
    assert!(codes(&report).contains(&"E-FMT-01"));
}

/// Remove every embedded asset block line from a built document.
fn remove_asset_blocks(html: &str) -> String {
    let mut result = String::new();
    let mut rest = html;
    while let Some(start) = rest.find("<script type=\"application/octet-stream\"") {
        let line_start = rest[..start]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0);
        let after = &rest[start..];
        let line_end = after
            .find('\n')
            .map(|index| start + index + 1)
            .unwrap_or(rest.len());
        result.push_str(&rest[..line_start]);
        rest = &rest[line_end..];
    }
    result.push_str(rest);
    result
}
