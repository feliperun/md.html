//! T12b document assets: closed MIME embedding (SPEC §14), embedded
//! @font-face faces (SPEC §18), og:image derivation (FMT-05), and the
//! non-portable CSP relaxation (FMT-03).

use std::fs;
use std::path::PathBuf;

use mdhtml::build::{BuildError, build};

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

fn build_source(source: &str) -> Result<String, BuildError> {
    build(
        source,
        &fixture_dir(),
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
}

fn between<'a>(html: &'a str, open: &str, close: &str) -> &'a str {
    let start = html.find(open).expect("open marker") + open.len();
    let end = html[start..].find(close).expect("close marker") + start;
    &html[start..end]
}

fn csp_of(html: &str) -> &str {
    between(
        html,
        "<meta http-equiv=\"Content-Security-Policy\" content=\"",
        "\">",
    )
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

/// The individual `@font-face` rule bodies inside a fonts style block.
fn font_faces(css: &str) -> Vec<String> {
    let mut faces = Vec::new();
    let mut rest = css;
    while let Some(start) = rest.find("@font-face") {
        let after = &rest[start + "@font-face".len()..];
        let open = after.find('{').expect("open brace") + 1;
        let close = after[open..].find('}').expect("close brace") + open;
        faces.push(after[open..close].to_string());
        rest = &after[close + 1..];
    }
    faces
}

fn css_decl(css: &str, name: &str) -> String {
    let marker = format!("{name}:");
    let start = css.find(&marker).expect("declaration") + marker.len();
    let value = &css[start..];
    let end = value.find(';').expect("declaration close");
    value[..end].trim().to_string()
}

fn css_src(css: &str) -> String {
    let marker = "src:";
    let start = css.find(marker).expect("src declaration") + marker.len();
    let value = &css[start..];
    let end = value.find(");").expect("src close") + 1;
    value[..end].trim().to_string()
}

#[test]
fn document_assets_fixture_embeds_assets_in_first_reference_order() {
    let source = fs::read_to_string(fixture_dir().join("document-assets.md")).expect("fixture");
    let html = build_source(&source).expect("fixture builds");

    let blocks = asset_blocks(&html);
    assert_eq!(
        blocks.len(),
        2,
        "distinct paths embed once: svg (md + html image + figures) and css (fonts body + mono)"
    );
    assert_eq!(blocks[0].0, "asset-tiny.svg");
    assert_eq!(blocks[0].1, "image/svg+xml");
    assert_eq!(blocks[1].0, "asset-tiny.css");
    assert_eq!(blocks[1].1, "text/css");

    let svg = fs::read(fixture_dir().join("asset-tiny.svg")).expect("read svg");
    let css = fs::read(fixture_dir().join("asset-tiny.css")).expect("read css");
    assert_eq!(decode_base64(&blocks[0].2), svg);
    assert_eq!(decode_base64(&blocks[1].2), css);

    assert!(html.contains(
        "<script type=\"application/octet-stream\" data-path=\"asset-tiny.svg\" \
         data-type=\"image/svg+xml\">"
    ));
    assert!(!html.contains("style id=\"mdhtml-fonts\""));
}

#[test]
fn document_assets_fixture_derives_og_image_and_declares_non_portable_csp() {
    let source = fs::read_to_string(fixture_dir().join("document-assets.md")).expect("fixture");
    let html = build_source(&source).expect("fixture builds");

    assert!(
        html.contains(
            "<meta property=\"og:image\" content=\"https://example.test/asset-tiny.svg\">"
        )
    );
    assert!(html.contains("data-mdhtml-portable=\"false\""));
    assert_eq!(
        csp_of(&html),
        "default-src 'none'; script-src 'unsafe-inline'; \
         style-src 'unsafe-inline' https://fonts.googleapis.com; \
         img-src data: blob:; font-src data: https://fonts.gstatic.com; \
         media-src data: blob:"
    );
}

#[test]
fn auto_fonts_embed_faces_for_emphasis_and_code_in_selection_order() {
    let source = "---\ntitle: Fonts\n---\nBody with *emphasis* and `code`.\n";
    let html = build_source(source).expect("builds");
    let faces = font_faces(&between(&html, "<style id=\"mdhtml-fonts\">", "</style>"));

    let expected = [
        (
            "InstrumentSans-latin-wght-normal.woff2",
            "Instrument Sans",
            "normal",
            "400 700",
        ),
        (
            "InstrumentSans-latin-wght-italic.woff2",
            "Instrument Sans",
            "italic",
            "400 700",
        ),
        (
            "GeistMono-wght-normal.woff2",
            "Geist Mono",
            "normal",
            "100 900",
        ),
    ];
    assert_eq!(faces.len(), expected.len());
    for (face, (file, family, style, weight)) in faces.iter().zip(expected) {
        assert_eq!(css_decl(face, "font-family"), format!("\"{family}\""));
        assert_eq!(css_decl(face, "font-style"), style);
        assert_eq!(css_decl(face, "font-weight"), weight);
        let src = css_src(face);
        let payload = src
            .strip_prefix("url(data:font/woff2;base64,")
            .and_then(|value| value.strip_suffix(')'))
            .expect("data uri src");
        let bytes = fs::read(fonts_dir().join(file)).expect("face file");
        assert_eq!(decode_base64(payload), bytes, "{file}");
    }
}

#[test]
fn editorial_theme_selects_newsreader_faces() {
    let source = "---\ntitle: Editorial\ntheme: editorial\n---\nBody with `code`.\n";
    let html = build_source(source).expect("builds");
    let faces = font_faces(&between(&html, "<style id=\"mdhtml-fonts\">", "</style>"));
    assert_eq!(faces.len(), 2);
    assert_eq!(css_decl(&faces[0], "font-family"), "\"Newsreader\"");
    assert_eq!(css_decl(&faces[0], "font-weight"), "200 800");
    assert_eq!(css_decl(&faces[1], "font-family"), "\"Geist Mono\"");
}

#[test]
fn system_and_map_fonts_embed_no_builtin_faces() {
    let system = "---\ntitle: Sys\nfonts: system\n---\nBody with *emphasis* and `code`.\n";
    let html = build_source(system).expect("builds");
    assert!(!html.contains("style id=\"mdhtml-fonts\""));

    let map = "---\ntitle: Map\nfonts:\n  body: asset-tiny.css\n---\n# Body\n";
    let html = build_source(map).expect("builds");
    assert!(!html.contains("style id=\"mdhtml-fonts\""));
    let blocks = asset_blocks(&html);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, "asset-tiny.css");
    assert_eq!(blocks[0].1, "text/css");
}

#[test]
fn out_of_table_mime_and_missing_files_are_e_cli01() {
    let out_of_table = "---\ntitle: Bad\n---\n![x](image.tiff)\n";
    assert_eq!(
        build_source(out_of_table)
            .expect_err("out-of-table extension")
            .code(),
        "E-CLI-01"
    );

    let missing = "---\ntitle: Bad\n---\n![x](no-such-file.png)\n";
    assert_eq!(
        build_source(missing).expect_err("missing asset").code(),
        "E-CLI-01"
    );

    let figures_missing =
        "---\ntitle: Bad\nfigures:\n  missing.png: { align: right }\n---\n# Body\n";
    assert_eq!(
        build_source(figures_missing)
            .expect_err("missing figure")
            .code(),
        "E-CLI-01"
    );

    let fonts_missing = "---\ntitle: Bad\nfonts:\n  body: no-such-font.woff2\n---\n# Body\n";
    assert_eq!(
        build_source(fonts_missing)
            .expect_err("missing font path")
            .code(),
        "E-CLI-01"
    );
}

#[test]
fn og_image_is_derived_only_when_url_and_cover_resolve() {
    let dir = std::env::temp_dir().join(format!("mdhtml-t12b-og-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(dir.join("cover.png"), b"cover").expect("write cover");

    let with_both =
        "---\ntitle: OG\nurl: https://example.test/post\ncover: cover.png\n---\n# Body\n";
    let html = build(
        &with_both,
        &dir,
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect("builds");
    assert!(
        html.contains("<meta property=\"og:image\" content=\"https://example.test/cover.png\">")
    );
    assert!(
        asset_blocks(&html).is_empty(),
        "the cover resolves but is not embedded"
    );

    let no_url = "---\ntitle: OG\ncover: cover.png\n---\n# Body\n";
    let html = build(&no_url, &dir, &runtime_dist(), &themes_dir(), &fonts_dir()).expect("builds");
    assert!(!html.contains("og:image"));

    let missing_cover =
        "---\ntitle: OG\nurl: https://example.test/post\ncover: missing.png\n---\n# Body\n";
    let error = build(
        &missing_cover,
        &dir,
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect_err("missing cover");
    assert_eq!(error.code(), "E-CLI-01");
}

#[test]
fn fonts_url_relaxes_the_csp_only_for_the_declared_origins() {
    let google = "---\ntitle: NP\nfonts:\n  url: https://fonts.googleapis.com/css2?family=Instrument+Sans\n---\n# Body\n";
    let html = build_source(google).expect("builds");
    assert!(html.contains("data-mdhtml-portable=\"false\""));
    assert_eq!(
        csp_of(&html),
        "default-src 'none'; script-src 'unsafe-inline'; \
         style-src 'unsafe-inline' https://fonts.googleapis.com; \
         img-src data: blob:; font-src data: https://fonts.gstatic.com; \
         media-src data: blob:"
    );

    let cdn = "---\ntitle: NP\nfonts:\n  url: https://cdn.example.test/fonts.css\n---\n# Body\n";
    let html = build_source(cdn).expect("builds");
    let csp = csp_of(&html);
    assert!(csp.contains("style-src 'unsafe-inline' https://cdn.example.test"));
    assert!(csp.contains("font-src data: https://cdn.example.test"));
    assert!(!csp.contains("https://cdn.example.test/fonts.css"));
    assert!(!csp.contains("script-src 'unsafe-inline' https://"));
}

#[test]
fn default_build_stays_portable_with_the_canonical_csp() {
    let source = "---\ntitle: Portable\n---\n# Body\n";
    let html = build_source(source).expect("builds");
    assert!(html.contains("data-mdhtml-portable=\"true\""));
    assert_eq!(
        csp_of(&html),
        "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
         img-src data: blob:; font-src data:; media-src data: blob:"
    );
}
