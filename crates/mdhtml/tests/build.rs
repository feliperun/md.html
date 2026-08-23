//! T12a document build: FMT-01 skeleton, FMT-02 rejection, FMT-05 metadata,
//! fragment embedding, theme/token styles, and CLI-01 atomic writes.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use mdhtml::build::{BuildError, build};
use mdhtml::selection::{load, select_fragments};

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    crate_dir().join("..").join("..")
}

fn fixture_path() -> PathBuf {
    repo_root().join("fixtures/document-template.md")
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

fn fixture_dir() -> PathBuf {
    repo_root().join("fixtures")
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

fn template_document() -> String {
    let source = fs::read_to_string(fixture_path()).expect("read fixtures/document-template.md");
    build_source(&source).expect("template fixture builds cleanly")
}

fn expected_runtime(source: &str) -> Vec<u8> {
    let manifest = load(&runtime_dist()).expect("committed manifest loads");
    let analysis = mdhtml::analysis::analyze_document(source);
    let body = mdhtml::frontmatter::parse_front_matter(source)
        .expect("valid front matter")
        .body
        .to_owned();
    let mut bytes = Vec::new();
    for id in select_fragments(&body, &analysis, &manifest) {
        let fragment = manifest
            .fragments
            .iter()
            .find(|fragment| fragment.id == id)
            .expect("selection returns manifest fragments");
        bytes.extend(fs::read(runtime_dist().join(&fragment.file)).expect("fragment file exists"));
    }
    bytes
}

fn embedded<'a>(html: &'a str, id: &str) -> &'a str {
    let marker = format!("<script id=\"{id}\">");
    let start = html.rfind(&marker).expect("element") + marker.len();
    let end = html[start..].find("</script>").expect("element close") + start;
    &html[start..end]
}

fn between<'a>(html: &'a str, open: &str, close: &str) -> &'a str {
    let start = html.find(open).expect("open marker") + open.len();
    let end = html[start..].find(close).expect("close marker") + start;
    &html[start..end]
}

#[test]
fn template_assembles_the_fmt01_skeleton() {
    let html = template_document();
    let source = fs::read_to_string(fixture_path()).expect("read template fixture");

    assert!(html.starts_with("<!doctype html>\n"));
    assert!(html.contains("<html lang=\"en\" data-mdhtml=\"1.0\" data-mdhtml-portable=\"true\">"));
    assert!(html.contains("<meta charset=\"utf-8\">"));
    assert!(
        html.contains("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">")
    );
    assert!(html.contains(&format!(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"{}\">",
        "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; \
         img-src data: blob:; font-src data:; media-src data: blob:"
    )));
    assert!(html.contains("<title>Document template</title>"));
    assert!(html.contains("<meta property=\"og:title\" content=\"Document template\">"));
    assert!(html.contains("<meta property=\"og:type\" content=\"article\">"));
    assert!(html.contains(
        "<meta name=\"description\" content=\"A synthetic asset-free template \
         proving the first complete mdhtml build.\">"
    ));
    assert!(html.contains(
        "<meta property=\"og:description\" content=\"A synthetic asset-free template \
         proving the first complete mdhtml build.\">"
    ));
    assert!(html.contains("<link rel=\"canonical\" href=\"https://example.test/template\">"));
    assert!(html.contains("<meta property=\"og:url\" content=\"https://example.test/template\">"));
    assert!(html.contains("<style id=\"mdhtml-tokens\">"));
    assert!(html.contains("<style id=\"mdhtml-theme\">"));
    assert!(!html.contains("<style id=\"mdhtml-user\">"));
    assert!(html.contains("<div id=\"mdhtml-app\"></div>"));
    assert!(html.contains(
        "<noscript><style>#mdhtml-source{display:block;white-space:pre-wrap;font-family:ui-monospace;padding:2rem}</style></noscript>"
    ));
    assert!(html.contains(&format!(
        "<script id=\"mdhtml-source\" type=\"text/markdown\">{source}</script>"
    )));
    assert!(html.contains("<script id=\"mdhtml-runtime\">"));
}

#[test]
fn runtime_embeds_selected_fragments_byte_exactly() {
    let html = template_document();
    let source = fs::read_to_string(fixture_path()).expect("read template fixture");
    assert_eq!(
        embedded(&html, "mdhtml-runtime").as_bytes(),
        expected_runtime(&source)
    );
}

#[test]
fn toc_fragment_is_omitted_when_disabled_or_out_of_depth() {
    let disabled = "---\ntitle: No toc\n---\n# Heading\n";
    let html = build_source(disabled).expect("builds");
    assert_eq!(
        embedded(&html, "mdhtml-runtime").as_bytes(),
        expected_runtime(disabled)
    );

    let deep = "---\ntitle: Deep\n---\n##### Too deep\n";
    let html = build_source(deep).expect("builds");
    assert_eq!(
        embedded(&html, "mdhtml-runtime").as_bytes(),
        expected_runtime(deep)
    );
}

#[test]
fn lightbox_fragment_is_embedded_when_document_has_images() {
    let source = "---\ntitle: With image\n---\n# Heading\n\n![alt](asset-tiny.svg)\n";
    let html = build_source(source).expect("builds");
    assert_eq!(
        embedded(&html, "mdhtml-runtime").as_bytes(),
        expected_runtime(source)
    );
}

#[test]
fn missing_title_is_e_fmt05() {
    let error = build_source("---\nsummary: no title\n---\n# Body\n").expect_err("missing title");
    assert_eq!(error.code(), "E-FMT-05");
}

#[test]
fn script_terminator_is_e_fmt02_anywhere_in_the_source() {
    for source in [
        "---\ntitle: Bad\n---\nText </script> here\n",
        "---\ntitle: Bad\nsummary: </SCRIPT> in front matter\n---\n# Body\n",
        "---\ntitle: Bad\n---\nText </Script mixed case\n",
    ] {
        let error = build_source(source).expect_err("script terminator");
        assert_eq!(error.code(), "E-FMT-02", "{source:?}");
    }
}

#[test]
fn unresolvable_local_theme_is_e_cli01() {
    for source in [
        "---\ntitle: Bad theme\ntheme: missing.theme.css\n---\n# Body\n",
        "---\ntitle: Bad theme\ntheme: ../evil.theme.css\n---\n# Body\n",
    ] {
        let error = build_source(source).expect_err("unresolvable local theme");
        assert_eq!(error.code(), "E-CLI-01", "{source:?}");
    }
}

#[test]
fn local_theme_embeds_user_style_with_the_technical_preset() {
    let dir = std::env::temp_dir().join(format!("mdhtml-t12a-local-theme-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(dir.join("custom.theme.css"), ":root{--md-accent:#123456}").expect("write theme");

    let source = "---\ntitle: Local theme\ntheme: custom.theme.css\n---\n# Body\n";
    let html = build(source, &dir, &runtime_dist(), &themes_dir(), &fonts_dir()).expect("builds");

    assert!(html.contains(
        "<style id=\"mdhtml-user\">:root {\n  --md-accent: #123456;\n}\n</style>"
    ));
    let theme = between(&html, "<style id=\"mdhtml-theme\">", "</style>");
    assert!(
        theme.contains("Instrument Sans"),
        "local themes use the technical preset"
    );
    assert!(!theme.contains("Newsreader"));
}

#[test]
fn tokens_render_as_escaped_custom_properties() {
    let source = "---\ntitle: Tokens\ntokens:\n  --md-accent: \"#0f766e\"\n  --md-focus: \"a;b}\"\n---\n# Body\n";
    let html = build_source(source).expect("builds");
    let tokens = between(&html, "<style id=\"mdhtml-tokens\">", "</style>");
    assert!(tokens.contains("--md-accent: #0f766e;"));
    assert!(
        tokens.contains("--md-focus: a\\;b\\};"),
        "CSS value is escaped: {tokens}"
    );
}

#[test]
fn lang_defaults_to_en_and_metadata_values_are_html_escaped() {
    let html = build_source("---\ntitle: A & <title> \"quoted\"\n---\n# Body\n").expect("builds");
    assert!(html.contains("<html lang=\"en\" data-mdhtml=\"1.0\" data-mdhtml-portable=\"true\">"));
    assert!(html.contains("<title>A &amp; &lt;title&gt; &quot;quoted&quot;</title>"));
    assert!(html.contains(
        "<meta property=\"og:title\" content=\"A &amp; &lt;title&gt; &quot;quoted&quot;\">"
    ));
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mdhtml-t12a-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mdhtml"))
        .args(args)
        .output()
        .expect("mdhtml binary should run")
}

#[test]
fn build_command_writes_default_and_override_outputs() {
    let dir = temp_dir("output");
    let input = dir.join("note.md");
    fs::write(&input, "---\ntitle: Note\n---\n# Note\n").expect("write input");

    let output = run(&["build", input.to_str().expect("utf8 path")]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let default = dir.join("note.md.html");
    assert!(default.exists(), "default output appends .html");
    assert!(
        fs::read_to_string(&default)
            .expect("read default output")
            .starts_with("<!doctype html>\n")
    );

    let overridden = dir.join("custom.html");
    let output = run(&[
        "build",
        input.to_str().expect("utf8 path"),
        "-o",
        overridden.to_str().expect("utf8 path"),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(overridden.exists());
}

#[test]
fn build_failure_leaves_destination_untouched() {
    let dir = temp_dir("atomic");
    let input = dir.join("bad.md");
    fs::write(&input, "---\ntitle: Bad\n---\nText </script> here\n").expect("write input");
    let destination = dir.join("out.md.html");
    fs::write(&destination, "precious existing content").expect("write destination");

    let output = run(&[
        "build",
        input.to_str().expect("utf8 path"),
        "-o",
        destination.to_str().expect("utf8 path"),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "mdhtml: E-FMT-02: input contains the forbidden sequence </script\n"
    );
    assert_eq!(
        fs::read_to_string(&destination).expect("read destination"),
        "precious existing content",
        "failed build must not touch the destination"
    );
}

#[test]
fn build_command_reports_missing_input_and_title_as_one_line_diagnostics() {
    let dir = temp_dir("diagnostics");

    let output = run(&["build", "no-such-file.md"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("mdhtml: E-CLI-05: input no-such-file.md is unreadable:"),
        "{stderr}"
    );
    assert_eq!(stderr.matches('\n').count(), 1);

    let input = dir.join("no-title.md");
    fs::write(&input, "---\nsummary: missing title\n---\n# Body\n").expect("write input");
    let output = run(&["build", input.to_str().expect("utf8 path")]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "mdhtml: E-FMT-05: front matter title is required and must be a nonempty string\n"
    );
}
