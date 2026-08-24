//! Security corpus walk (Phase 2 html-url-guard): every
//! `fixtures/security/url-*.json` and `html-*.json` case builds through
//! `mdhtml::build::build` with `assets` materialized next to the source in a
//! temp directory; invalid cases must fail with the exact frozen
//! `diagnostic` code and valid cases must build.

use std::fs;
use std::path::{Path, PathBuf};

use mdhtml::build::build;
use mdhtml::security::html::{UrlContext, validate_identifier, validate_svg, validate_url};

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

struct SecurityFixture {
    id: String,
    status: String,
    diagnostic: Option<String>,
    location: Option<String>,
    source: String,
    assets: Vec<(String, Vec<u8>)>,
}

/// The fixture fields the harness needs: strings and a nested object of
/// strings (`assets`).
enum Json {
    Str(String),
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

fn load_fixture(path: &Path) -> SecurityFixture {
    let raw = fs::read_to_string(path).expect("fixture is readable");
    let Json::Obj(object) = JsonParser::parse(&raw) else {
        panic!("fixture root must be an object");
    };
    let id = string_field(&object, "id").expect("fixture id");
    let status = string_field(&object, "status").expect("fixture status");
    let source = string_field(&object, "source").expect("fixture source");
    let diagnostic = string_field(&object, "diagnostic");
    let location = string_field(&object, "location");
    let mut assets = Vec::new();
    if let Some(Json::Obj(entries)) = field(&object, "assets") {
        for (name, value) in entries {
            let Json::Str(payload) = value else {
                panic!("asset {name} must be a base64 string");
            };
            assets.push((name.clone(), decode_base64(payload)));
        }
    }
    SecurityFixture {
        id,
        status,
        diagnostic,
        location,
        source,
        assets,
    }
}

/// The walk set for this node: url-*.json and html-*.json under
/// fixtures/security, in deterministic sorted order.
fn cases() -> Vec<PathBuf> {
    let mut paths = fs::read_dir(security_dir())
        .expect("fixtures/security is readable")
        .map(|entry| entry.expect("readable entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.ends_with(".json")
                        && (name.starts_with("url-") || name.starts_with("html-"))
                })
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[test]
fn url_and_html_cases_reject_or_build_with_the_frozen_diagnostics() {
    let dir = std::env::temp_dir().join(format!("mdhtml-security-html-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    let cases = cases();
    assert!(
        cases.len() >= 11,
        "the eleven url/html fixtures are present: {}",
        cases.len()
    );

    for path in cases {
        let fixture = load_fixture(&path);
        for (name, bytes) in &fixture.assets {
            fs::write(dir.join(name), bytes).expect("write materialized asset");
        }
        let result = build(
            &fixture.source,
            &dir,
            &runtime_dist(),
            &themes_dir(),
            &fonts_dir(),
        );
        match fixture.status.as_str() {
            "invalid" => {
                let error = result.expect_err(&format!("{} must fail the build", fixture.id));
                let expected = fixture
                    .diagnostic
                    .as_deref()
                    .expect("invalid fixtures carry a diagnostic");
                assert_eq!(
                    error.code(),
                    expected,
                    "{} must fail with exactly {expected}",
                    fixture.id
                );
                if let Some(location) = &fixture.location {
                    let (line, column) = location
                        .split_once(':')
                        .expect("fixture location is LINE:COLUMN");
                    assert!(
                        error
                            .to_string()
                            .contains(&format!("(line {line}, column {column})")),
                        "{} must cite line {line}, column {column} in its message: {}",
                        fixture.id,
                        error
                    );
                }
            }
            "valid" => {
                result.expect(&format!("{} must build cleanly", fixture.id));
            }
            other => panic!("fixture {} has unknown status {other}", fixture.id),
        }
    }
}

#[test]
fn url_scheme_matching_is_case_insensitive_and_strips_ascii_whitespace() {
    for destination in [
        "jaVasCript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "java\tscript:alert(1)",
        "java\nscript:alert(1)",
        " \tjavascript:alert(1)",
    ] {
        assert_eq!(
            validate_url(destination, UrlContext::Link)
                .expect_err("unsafe scheme")
                .code,
            "E-MDHSEC-012",
            "{destination:?}"
        );
    }
}

#[test]
fn url_contexts_apply_their_own_allowlists() {
    assert_eq!(
        validate_url("https://example.test/a", UrlContext::Link),
        Ok(())
    );
    assert_eq!(
        validate_url("mailto:author@example.com", UrlContext::Link),
        Ok(())
    );
    assert_eq!(validate_url("tel:+15551234567", UrlContext::Link), Ok(()));
    assert_eq!(validate_url("guide/next.md", UrlContext::Link), Ok(()));
    assert_eq!(validate_url("#section", UrlContext::Link), Ok(()));
    assert_eq!(validate_url("", UrlContext::Link), Ok(()));

    assert_eq!(
        validate_url("data:image/png;base64,AA==", UrlContext::Image),
        Ok(())
    );
    assert_eq!(
        validate_url("data:text/html,<b>hi</b>", UrlContext::Link)
            .expect_err("data in href")
            .code,
        "E-MDHSEC-012"
    );
    assert_eq!(
        validate_url("blob:https://example.test/x", UrlContext::Image)
            .expect_err("blob outside the image allowlist")
            .code,
        "E-MDHSEC-012"
    );

    assert_eq!(
        validate_url("https://example.test/post", UrlContext::Metadata),
        Ok(())
    );
    assert_eq!(
        validate_url("javascript:alert(1)", UrlContext::Metadata)
            .expect_err("metadata outside http/https")
            .code,
        "E-MDHSEC-005"
    );
    assert_eq!(
        validate_url("#fragment", UrlContext::Metadata)
            .expect_err("fragment metadata URL")
            .code,
        "E-MDHSEC-005"
    );
}

#[test]
fn identifiers_must_match_the_heading_and_class_contract() {
    assert_eq!(validate_identifier("abc-1_2"), Ok(()));
    assert_eq!(validate_identifier("results"), Ok(()));
    assert_eq!(
        validate_identifier("bad.id")
            .expect_err("dot outside the contract")
            .code,
        "E-MDHSEC-004"
    );
    assert_eq!(
        validate_identifier("has space")
            .expect_err("space outside the contract")
            .code,
        "E-MDHSEC-004"
    );
    assert_eq!(
        validate_identifier("").expect_err("empty token").code,
        "E-MDHSEC-004"
    );
}

#[test]
fn svg_assets_reject_executables_handlers_and_external_references() {
    assert_eq!(
        validate_svg("<svg xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M0 0\"/></svg>"),
        Ok(())
    );
    assert_eq!(
        validate_svg("<svg><script>alert(1)</script></svg>")
            .expect_err("script is executable")
            .code,
        "E-MDHSEC-011"
    );
    assert_eq!(
        validate_svg("<svg><circle onload=\"alert(1)\"/></svg>")
            .expect_err("event handler")
            .code,
        "E-MDHSEC-001"
    );
    assert_eq!(
        validate_svg("<svg><image href=\"https://evil.example/x.png\"/></svg>")
            .expect_err("external href")
            .code,
        "E-MDHSEC-013"
    );
    assert_eq!(
        validate_svg("<svg><image xlink:href=\"data:image/png;base64,AA==\"/></svg>")
            .expect_err("external xlink:href")
            .code,
        "E-MDHSEC-013"
    );
    assert_eq!(validate_svg("<svg><image href=\"#icon\"/></svg>"), Ok(()));
    assert_eq!(
        validate_svg("<svg><image href=\"local.png\"/></svg>"),
        Ok(())
    );
}

#[test]
fn located_violations_render_the_prd14_excerpt_block() {
    let dir = std::env::temp_dir().join(format!("mdhtml-security-html-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");

    // One URL violation: the destination starts at line 5, column 1, and the
    // caret spans the destination substring within the cited source line.
    let url_source = "---\ntitle: T\n---\n\n[click](javascript:alert(1))\n";
    let url_error = build(
        url_source,
        &dir,
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect_err("unsafe link scheme fails the build");
    assert_eq!(
        url_error.to_string(),
        "mdhtml: E-MDHSEC-012: unsafe URI scheme in destination \"javascript:alert(1)\" \
         (line 5, column 1)\n    [click](javascript:alert(1))\n            \
         ^^^^^^^^^^^^^^^^^^^"
    );

    // One identifier violation: the {#id} override is on line 5, column 1,
    // and the caret spans the offending token within the cited source line.
    let id_source = "---\ntitle: T\n---\n\n# Heading {#bad.id}\n";
    let id_error = build(
        id_source,
        &dir,
        &runtime_dist(),
        &themes_dir(),
        &fonts_dir(),
    )
    .expect_err("invalid heading id fails the build");
    assert_eq!(
        id_error.to_string(),
        "mdhtml: E-MDHSEC-004: identifier \"bad.id\" must match [A-Za-z0-9_-]+ \
         (line 5, column 1)\n    # Heading {#bad.id}\n                ^^^^^^"
    );
}
