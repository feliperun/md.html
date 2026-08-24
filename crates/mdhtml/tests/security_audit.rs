//! mdhtml audit (CLI-06): every `fixtures/security/artifact-*.json` case
//! builds (when source-derived) or is written verbatim (when static) to a
//! temp `.md.html`, is audited through `mdhtml::audit::audit_artifact`, and
//! must produce the frozen human and `--json` verdicts. Mutation cases build
//! a clean document in-memory, tamper the artifact deterministically, and
//! pin the exact audit code and verdict.

use std::fs;
use std::path::{Path, PathBuf};

use mdhtml::audit::{AuditReport, audit_artifact};
use mdhtml::build::{build, build_unsafe};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn security_dir() -> PathBuf {
    repo_root().join("fixtures/security")
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

struct ArtifactFixture {
    id: String,
    status: String,
    diagnostic: Option<String>,
    location: Option<String>,
    source: Option<String>,
    unsafe_build: bool,
    html: Option<String>,
    assets: Vec<(String, Vec<u8>)>,
}

/// The fixture fields the harness needs: strings, a bool, and a nested
/// object of strings (`assets`).
enum Json {
    Str(String),
    Bool(bool),
    Obj(Vec<(String, Json)>),
}

struct JsonParser<'a> {
    input: &'a [u8],
    index: usize,
}

impl<'a> JsonParser<'a> {
    fn parse(input: &'a str) -> Json {
        let mut parser = JsonParser {
            input: input.as_bytes(),
            index: 0,
        };
        parser.whitespace();
        let value = parser.value();
        parser.whitespace();
        assert_eq!(parser.index, parser.input.len(), "trailing JSON content");
        value
    }

    fn whitespace(&mut self) {
        while matches!(
            self.input.get(self.index),
            Some(b' ' | b'\t' | b'\n' | b'\r')
        ) {
            self.index += 1;
        }
    }

    fn value(&mut self) -> Json {
        self.whitespace();
        match self.input.get(self.index) {
            Some(b'"') => Json::Str(self.string()),
            Some(b't') => {
                self.expect(b"true");
                Json::Bool(true)
            }
            Some(b'{') => self.object(),
            other => panic!("unexpected JSON byte {other:?} at {}", self.index),
        }
    }

    fn expect(&mut self, expected: &[u8]) {
        assert_eq!(
            &self.input[self.index..self.index + expected.len()],
            expected
        );
        self.index += expected.len();
    }

    fn string(&mut self) -> String {
        self.expect(b"\"");
        let mut output = String::new();
        loop {
            match self.input[self.index] {
                b'"' => {
                    self.index += 1;
                    return output;
                }
                b'\\' => {
                    self.index += 1;
                    self.push_escape(&mut output);
                }
                byte if byte >= 0x20 => self.push_utf8_char(&mut output),
                other => panic!("invalid JSON string byte {other:?}"),
            }
        }
    }

    /// One JSON string escape sequence, starting right after the `\`.
    fn push_escape(&mut self, output: &mut String) {
        let escaped = self.input[self.index];
        self.index += 1;
        match escaped {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'/' => output.push('/'),
            b'b' => output.push('\u{0008}'),
            b'f' => output.push('\u{000c}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
            b'u' => self.push_unicode_escape(output),
            other => panic!("unsupported JSON escape {other:?}"),
        }
    }

    /// `\uXXXX`, starting right after the `u`.
    fn push_unicode_escape(&mut self, output: &mut String) {
        let hex = std::str::from_utf8(&self.input[self.index..self.index + 4])
            .expect("utf8 hex")
            .chars()
            .try_fold(0u32, |value, ch| Some(value * 16 + ch.to_digit(16)?))
            .expect("hex escape");
        output.push(char::from_u32(hex).expect("unicode escape"));
        self.index += 4;
    }

    /// One unescaped UTF-8 character starting at the current index.
    fn push_utf8_char(&mut self, output: &mut String) {
        let rest = std::str::from_utf8(&self.input[self.index..]).expect("utf8");
        let character = rest.chars().next().expect("character");
        output.push(character);
        self.index += character.len_utf8();
    }

    fn object(&mut self) -> Json {
        self.expect(b"{");
        let mut entries = Vec::new();
        self.whitespace();
        if self.input.get(self.index) == Some(&b'}') {
            self.index += 1;
            return Json::Obj(entries);
        }
        loop {
            self.whitespace();
            let key = self.string();
            self.whitespace();
            self.expect(b":");
            entries.push((key, self.value()));
            self.whitespace();
            match self.input[self.index] {
                b',' => self.index += 1,
                b'}' => {
                    self.index += 1;
                    return Json::Obj(entries);
                }
                other => panic!("unexpected JSON object byte {other:?}"),
            }
        }
    }
}

fn field<'a>(object: &'a [(String, Json)], name: &str) -> Option<&'a Json> {
    object
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value)
}

fn string_field(object: &[(String, Json)], name: &str) -> Option<String> {
    match field(object, name) {
        Some(Json::Str(value)) => Some(value.clone()),
        Some(_) => panic!("fixture field {name} must be a string"),
        None => None,
    }
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

fn load_fixture(path: &Path) -> ArtifactFixture {
    let raw = fs::read_to_string(path).expect("fixture is readable");
    let Json::Obj(object) = JsonParser::parse(&raw) else {
        panic!("fixture root must be an object");
    };
    assert_eq!(
        string_field(&object, "kind").as_deref(),
        Some("artifact"),
        "artifact fixtures carry kind: artifact"
    );
    let mut assets = Vec::new();
    if let Some(Json::Obj(entries)) = field(&object, "assets") {
        for (name, value) in entries {
            let Json::Str(payload) = value else {
                panic!("asset {name} must be a base64 string");
            };
            assets.push((name.clone(), decode_base64(payload)));
        }
    }
    ArtifactFixture {
        id: string_field(&object, "id").expect("fixture id"),
        status: string_field(&object, "status").expect("fixture status"),
        diagnostic: string_field(&object, "diagnostic"),
        location: string_field(&object, "location"),
        source: string_field(&object, "source"),
        unsafe_build: matches!(field(&object, "unsafe"), Some(Json::Bool(true))),
        html: string_field(&object, "html"),
        assets,
    }
}

fn cases() -> Vec<PathBuf> {
    let mut paths = fs::read_dir(security_dir())
        .expect("fixtures/security is readable")
        .map(|entry| entry.expect("readable entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("artifact-") && name.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mdhtml-security-audit-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_artifact(dir: &Path, id: &str, html: &str) -> PathBuf {
    let path = dir.join(format!("{id}.md.html"));
    fs::write(&path, html).expect("write artifact");
    path
}

fn read_audit(path: &Path) -> AuditReport {
    let html = fs::read_to_string(path).expect("audit artifact is readable");
    audit_artifact(&html)
}

#[test]
fn artifact_fixtures_build_or_write_and_audit_to_the_frozen_verdicts() {
    let dir = temp_dir();
    let cases = cases();
    assert!(
        cases.len() >= 4,
        "the four artifact fixtures are present: {}",
        cases.len()
    );

    for path in cases {
        let fixture = load_fixture(&path);
        let artifact_path = match (&fixture.source, &fixture.html) {
            (Some(source), None) => {
                for (name, bytes) in &fixture.assets {
                    fs::write(dir.join(name), bytes).expect("write materialized asset");
                }
                let built = if fixture.unsafe_build {
                    build_unsafe(source, &dir, &runtime_dist(), &themes_dir(), &fonts_dir())
                } else {
                    build(source, &dir, &runtime_dist(), &themes_dir(), &fonts_dir())
                };
                write_artifact(
                    &dir,
                    &fixture.id,
                    &built.expect(&format!("{} must build", fixture.id)),
                )
            }
            (None, Some(html)) => write_artifact(&dir, &fixture.id, html),
            _ => panic!("artifact fixtures carry exactly one of source or html"),
        };

        let report = read_audit(&artifact_path);
        match fixture.status.as_str() {
            "valid" => {
                assert!(report.safe, "{} must audit SAFE", fixture.id);
                assert!(report.render().ends_with("SAFE\n"), "{}", report.render());
                assert!(
                    report.render_json().contains("\"safe\":true"),
                    "{}",
                    fixture.id
                );
            }
            "invalid" => {
                assert!(!report.safe, "{} must audit UNSAFE", fixture.id);
                let expected = fixture
                    .diagnostic
                    .as_deref()
                    .expect("invalid fixtures carry a diagnostic");
                assert!(
                    report.has_code(expected),
                    "{} must report exactly {expected}: {}",
                    fixture.id,
                    report.render()
                );
                assert!(
                    report.render_json().contains("\"safe\":false"),
                    "{}",
                    fixture.id
                );
                if let Some(location) = &fixture.location {
                    let (line, column) = location
                        .split_once(':')
                        .expect("fixture location is LINE:COLUMN");
                    assert!(
                        report
                            .render()
                            .contains(&format!("(line {line}, column {column})")),
                        "{} must cite line {line}, column {column}: {}",
                        fixture.id,
                        report.render()
                    );
                }
            }
            other => panic!("fixture {} has unknown status {other}", fixture.id),
        }
    }
}

fn build_clean(source: &str) -> String {
    let dir = temp_dir();
    build(source, &dir, &runtime_dist(), &themes_dir(), &fonts_dir()).expect("clean build")
}

fn replace_in_source(html: &str, from: &str, to: &str) -> String {
    let marker = "<script id=\"mdhtml-source\" type=\"text/markdown\">";
    let start = html.find(marker).expect("source element") + marker.len();
    let end = html[start..].find("</script>").expect("source close") + start;
    let mut out = html.to_string();
    let source_region = &out[start..end];
    let position = source_region.find(from).expect("source marker");
    out.replace_range(start + position..start + position + from.len(), to);
    out
}

fn assert_unsafe_with_code(report: &AuditReport, code: &str) {
    assert!(!report.safe, "mutation must fail the audit");
    assert!(
        report.has_code(code),
        "report must carry {code}: {}",
        report.render()
    );
    assert!(report.render().ends_with("UNSAFE\n"));
    assert!(report.render_json().contains("\"safe\":false"));
}

#[test]
fn tampering_one_byte_of_the_runtime_reports_e_mdhsec_015() {
    let html = build_clean("---\ntitle: Tamper\n---\n# Body\n");
    let marker = "<script id=\"mdhtml-runtime\">";
    let start = html.find(marker).expect("runtime element") + marker.len();
    let original = html.as_bytes()[start];
    assert!(original.is_ascii(), "runtime starts with ASCII");
    let replacement = if original == b'Z' { b'A' } else { b'Z' };
    let mut tampered = html.clone();
    tampered.replace_range(start..start + 1, &(replacement as char).to_string());

    let report = audit_artifact(&tampered);
    assert_unsafe_with_code(&report, "E-MDHSEC-015");
    assert!(!report.runtime_pass(), "runtime integrity fails");
    assert!(report.render_json().contains("\"runtime\":\"fail\""));
    assert!(report.render_json().contains("\"html\":\"pass\""));
}

#[test]
fn stripping_the_csp_meta_reports_e_mdhsec_016() {
    let html = build_clean("---\ntitle: CSP\n---\n# Body\n");
    let meta = "  <meta http-equiv=\"Content-Security-Policy\" content=\"";
    let start = html.find(meta).expect("csp meta");
    let end = html[start..].find('\n').expect("meta line end") + start + 1;
    let mut stripped = html.clone();
    stripped.replace_range(start..end, "");

    let report = audit_artifact(&stripped);
    assert_unsafe_with_code(&report, "E-MDHSEC-016");
    assert!(!report.runtime_pass());
    assert!(report.render_json().contains("\"runtime\":\"fail\""));
}

#[test]
fn injecting_an_event_handler_attribute_reports_e_mdhsec_001() {
    let html = build_clean("---\ntitle: Handler\n---\n# Body\n");
    let injected = html.replace(
        "<div id=\"mdhtml-app\"></div>",
        "<div id=\"mdhtml-app\" onload=\"alert(1)\"></div>",
    );

    let report = audit_artifact(&injected);
    assert_unsafe_with_code(&report, "E-MDHSEC-001");
    assert!(!report.html_pass(), "the html category fails");
    assert!(report.render_json().contains("\"html\":\"fail\""));
    assert!(report.render_json().contains("\"css\":\"pass\""));
}

#[test]
fn injecting_a_network_url_into_the_user_style_reports_e_mdhsec_009() {
    let html = build_clean("---\ntitle: Style\n---\n# Body\n");
    let injected = html.replace(
        "<script id=\"mdhtml-source\"",
        "<style id=\"mdhtml-user\">.x{background:url(https://evil.example/x.png)}</style>\n  <script id=\"mdhtml-source\"",
    );

    let report = audit_artifact(&injected);
    assert_unsafe_with_code(&report, "E-MDHSEC-009");
    assert!(!report.css_pass(), "the css category fails");
    assert!(report.render_json().contains("\"css\":\"fail\""));
}

#[test]
fn a_stored_javascript_link_reports_e_mdhsec_012_from_the_source_rerun() {
    let html = build_clean("---\ntitle: Link\n---\n\n[click](https://example.test)\n");
    let mutated = replace_in_source(&html, "https://example.test", "javascript:alert(1)");

    let report = audit_artifact(&mutated);
    assert_unsafe_with_code(&report, "E-MDHSEC-012");
    assert!(!report.html_pass(), "the html category fails");
    assert!(report.render_json().contains("\"html\":\"fail\""));
    assert!(
        report.render().contains("(line 5, column 1)"),
        "the located guard cites the stored source: {}",
        report.render()
    );
}

#[test]
fn a_stored_source_that_fails_analysis_has_no_source_integrity() {
    let html = build_clean("---\ntitle: Integrity\n---\n# Body\n");
    let mutated = replace_in_source(&html, "---\ntitle: Integrity\n---\n", "");

    let report = audit_artifact(&mutated);
    assert_unsafe_with_code(&report, "E-FMT-05");
    assert!(!report.source_integrity(), "analysis errors fail integrity");
    assert!(report.render_json().contains("\"sourceIntegrity\":false"));
    assert!(report.render_json().contains("\"runtime\":\"pass\""));
}

#[test]
fn a_clean_artifact_audits_safe_with_every_verdict_green() {
    let html = build_clean("---\ntitle: Clean\n---\n# Body\n");
    let report = audit_artifact(&html);

    assert!(report.safe);
    assert!(report.source_integrity());
    assert!(report.html_pass());
    assert!(report.css_pass());
    assert!(report.runtime_pass());
    assert!(report.external_resources_pass());
    assert!(report.origins.is_empty());
    assert!(
        report
            .render()
            .starts_with("✓ valid mdhtml v1.0\n✓ canonical source present\n")
    );
    assert!(report.render().ends_with("SAFE\n"));
    assert_eq!(
        report.render_json(),
        "{\"safe\":true,\"specVersion\":\"1.0\",\"sourceIntegrity\":true,\"html\":\"pass\",\"css\":\"pass\",\"runtime\":\"pass\",\"externalResources\":[]}\n"
    );
}

#[test]
fn a_fonts_url_artifact_reports_the_sanctioned_origins_and_stays_safe() {
    let html = build_clean(
        "---\ntitle: Online fonts\nfonts:\n  url: https://fonts.googleapis.com/css2?family=Instrument+Sans\n---\n# Body\n",
    );
    let report = audit_artifact(&html);

    assert!(report.safe, "{}", report.render());
    assert_eq!(
        report.origins,
        ["https://fonts.googleapis.com", "https://fonts.gstatic.com"]
    );
    assert!(report.render_json().contains(
        "\"externalResources\":[\"https://fonts.googleapis.com\",\"https://fonts.gstatic.com\"]"
    ));
}

#[test]
fn an_external_image_origin_is_not_sanctioned_and_fails_external_resources() {
    let html =
        build_clean("---\ntitle: External image\n---\n\n![x](https://cdn.example.test/a.png)\n");
    let report = audit_artifact(&html);

    assert!(!report.safe);
    assert!(!report.external_resources_pass());
    assert_eq!(report.origins, ["https://cdn.example.test"]);
    assert!(
        report
            .render_json()
            .contains("\"externalResources\":[\"https://cdn.example.test\"]")
    );
}
