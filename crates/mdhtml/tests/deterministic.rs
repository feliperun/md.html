//! PRD §52 deterministic-build validation: the same source + mdhtml version +
//! configuration MUST produce a byte-identical `.md.html` across two
//! consecutive in-process builds, two equivalent directory-path spellings,
//! the build → extract → rebuild round trip, the feature matrix (default,
//! `--no-fonts`, local theme, relaxed `fonts.url`, embedded assets/images,
//! tokens/sections/containers, `--unsafe`) and two CLI subprocess runs.
//!
//! Every comparison is FULL byte equality of the artifact — never hashes of
//! prefixes, never normalized forms. The suite pins the order sources the
//! pipeline relies on, so a future regression cannot land silently:
//! `selection::select_fragments` returns manifest-ordered ids, front matter
//! `Value::Mapping` is an order-preserving Vec, asset embedding follows
//! scanner document order (then `figures:` keys, then `fonts.body`/`mono`),
//! face selection is body-normal → body-italic → mono-normal, and the Phase 2
//! CSS guard re-serializes author CSS with fixed `PrinterOptions`.

use std::fs;
use std::path::{Path, PathBuf};

use mdhtml::build::{BuildError, build, build_no_fonts, build_unsafe};
use mdhtml::extract::extract_source;

type Builder = fn(&str, &Path, &Path, &Path, &Path) -> Result<String, BuildError>;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn runtime_dist() -> PathBuf {
    repo_root().join("runtime").join("dist")
}

fn themes_dir() -> PathBuf {
    repo_root().join("themes")
}

fn fonts_dir() -> PathBuf {
    repo_root().join("fonts")
}

fn fixtures_dir() -> PathBuf {
    repo_root().join("fixtures")
}

fn templates_dir() -> PathBuf {
    repo_root().join("templates")
}

fn examples_dir() -> PathBuf {
    repo_root().join("examples")
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mdhtml-deterministic-{name}-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Build the same source twice in-process and assert the artifacts are
/// byte-identical; returns the first artifact for further assertions.
fn build_twice(source: &str, source_dir: &Path, builder: Builder) -> String {
    let first = builder(source, source_dir, &runtime_dist(), &themes_dir(), &fonts_dir())
        .unwrap_or_else(|error| panic!("first build failed: {error}"));
    assert!(
        first.starts_with("<!doctype html>\n"),
        "artifact must be a complete document"
    );
    let second = builder(source, source_dir, &runtime_dist(), &themes_dir(), &fonts_dir())
        .unwrap_or_else(|error| panic!("second build failed: {error}"));
    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "two consecutive builds of the same source must be byte-identical"
    );
    first
}

/// The exact committed bytes of `fixtures/asset-tiny.svg`, base64-encoded.
const ASSET_TINY_SVG: &str = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxIiBoZWlnaHQ9IjEiIHZpZXdCb3g9IjAgMCAxIDEiPjxyZWN0IHdpZHRoPSIxIiBoZWlnaHQ9IjEiIGZpbGw9IiMwZjc2NmUiLz48L3N2Zz4K";

/// The exact committed bytes of `fixtures/asset-tiny.css`, base64-encoded.
const ASSET_TINY_CSS: &str = "Lm1kLXRpbnkgeyBjb2xvcjogIzBmNzY2ZTsgfQo=";

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

#[test]
fn consecutive_builds_of_the_same_source_are_byte_identical() {
    let source = fs::read_to_string(templates_dir().join("spec.md")).expect("read template");
    let html = build_twice(&source, &templates_dir(), build);
    assert!(
        html.contains("<style id=\"mdhtml-tokens\">"),
        "the spec template carries the tokens block"
    );
}

#[test]
fn equivalent_directory_path_spellings_are_byte_identical() {
    let source = fs::read_to_string(templates_dir().join("resume.md")).expect("read template");
    let root_a = repo_root();
    let root_b = root_a.join("crates").join("..");
    assert_ne!(
        root_a, root_b,
        "the two spellings must differ lexically to prove no path leaks"
    );

    let first = build(
        &source,
        &templates_dir(),
        &root_a.join("runtime/dist"),
        &root_a.join("themes"),
        &root_a.join("fonts"),
    )
    .expect("build with the first spelling");
    let second = build(
        &source,
        &templates_dir(),
        &root_b.join("runtime/dist"),
        &root_b.join("themes"),
        &root_b.join("fonts"),
    )
    .expect("build with the second spelling");
    assert_eq!(
        first.as_bytes(),
        second.as_bytes(),
        "equivalent directory-path spellings must produce byte-identical artifacts"
    );

    let absolute = root_a.to_str().expect("repo root is UTF-8");
    assert!(
        !first.contains(absolute),
        "the absolute repository path must not leak into the artifact"
    );
}

#[test]
fn build_extract_rebuild_round_trip_is_byte_identical() {
    let source = fs::read_to_string(examples_dir().join("spec.md")).expect("read example");
    let html = build_twice(&source, &examples_dir(), build);

    let restored = extract_source(html.as_bytes()).expect("extract canonical source");
    assert_eq!(
        restored,
        source.as_bytes(),
        "extract restores the canonical source byte-for-byte"
    );
    let rebuilt = build(
        std::str::from_utf8(&restored).expect("restored source is UTF-8"),
        &examples_dir(),
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect("rebuild from the extracted source");
    assert_eq!(
        html.as_bytes(),
        rebuilt.as_bytes(),
        "rebuilding from the extracted source must be byte-identical"
    );
}

#[test]
fn feature_matrix_builds_are_byte_identical() {
    let theme_dir = temp_dir("local-theme");
    fs::write(
        theme_dir.join("resume.md"),
        fs::read(examples_dir().join("resume.md")).expect("read example resume"),
    )
    .expect("materialize resume.md");
    fs::write(
        theme_dir.join("resume.theme.css"),
        fs::read(examples_dir().join("resume.theme.css")).expect("read example theme"),
    )
    .expect("materialize resume.theme.css");

    let assets_dir = temp_dir("assets");
    fs::write(
        assets_dir.join("asset-tiny.svg"),
        decode_base64(ASSET_TINY_SVG),
    )
    .expect("materialize the svg from base64");
    fs::write(
        assets_dir.join("asset-tiny.css"),
        decode_base64(ASSET_TINY_CSS),
    )
    .expect("materialize the css from base64");
    let assets_source = fs::read_to_string(fixtures_dir().join("document-assets.md"))
        .expect("read the assets fixture");

    let tokens_source =
        fs::read_to_string(fixtures_dir().join("document-template.md")).expect("read fixture");
    let sections_source = fs::read_to_string(examples_dir().join("spec.md")).expect("read example");
    let no_fonts_source = fs::read_to_string(templates_dir().join("spec.md")).expect("read template");
    let fonts_url_source = "---\ntitle: NP\nfonts:\n  url: https://fonts.googleapis.com/css2?family=Instrument+Sans\n---\n# Body\n";

    let cases: Vec<(&str, String, PathBuf, Builder, Option<&str>)> = vec![
        (
            "default",
            fs::read_to_string(templates_dir().join("resume.md")).expect("read template"),
            templates_dir(),
            build,
            None,
        ),
        (
            "no-fonts",
            no_fonts_source,
            templates_dir(),
            build_no_fonts,
            Some("!mdhtml-fonts"),
        ),
        (
            "local-theme",
            fs::read_to_string(theme_dir.join("resume.md")).expect("read materialized resume"),
            theme_dir,
            build,
            Some("mdhtml-user"),
        ),
        (
            "fonts-url-relaxed",
            fonts_url_source.to_string(),
            fixtures_dir(),
            build,
            Some("fonts.googleapis.com"),
        ),
        (
            "assets-images",
            assets_source,
            assets_dir,
            build,
            Some("application/octet-stream"),
        ),
        ("tokens", tokens_source, fixtures_dir(), build, Some("--md-accent")),
        (
            "sections-containers",
            sections_source,
            examples_dir(),
            build,
            None,
        ),
    ];

    for (name, source, dir, builder, sanity) in cases {
        let html = build_twice(&source, &dir, builder);
        if let Some(marker) = sanity {
            if let Some(negated) = marker.strip_prefix('!') {
                assert!(
                    !html.contains(negated),
                    "{name}: artifact must not contain {negated:?}"
                );
            } else {
                assert!(
                    html.contains(marker),
                    "{name}: artifact must contain {marker:?}"
                );
            }
        }
    }
}

#[test]
fn unsafe_builds_are_byte_identical() {
    let source = "---\ntitle: Unsafe\n---\n\n[click](javascript:alert(1))\n";
    let html = build_twice(&source, &fixtures_dir(), build_unsafe);
    assert!(
        html.contains("data-mdhtml-safe=\"false\""),
        "the --unsafe artifact attests the unsafe profile"
    );
}
