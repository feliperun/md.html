//! Security corpus walk (Phase 2 css-policy): every
//! `fixtures/security/css-*.json` case builds through
//! `mdhtml::build::build` with the `.theme.css` fixture materialized next to
//! the source in a temp directory; invalid cases must fail with the exact
//! frozen `diagnostic` code and valid cases must build. Approved themes are
//! re-serialized deterministically: the same input yields identical bytes.

use std::fs;
use std::path::{Path, PathBuf};

use mdhtml::build::build;
use mdhtml::security::css::guard_author_css;

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

/// The walk set for this node: css-*.json under fixtures/security, in
/// deterministic sorted order.
fn cases() -> Vec<PathBuf> {
    let mut paths = fs::read_dir(security_dir())
        .expect("fixtures/security is readable")
        .map(|entry| entry.expect("readable entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("css-") && name.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[test]
fn css_cases_reject_or_build_with_the_frozen_diagnostics() {
    let dir = std::env::temp_dir().join(format!("mdhtml-security-css-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    let cases = cases();
    assert!(
        cases.len() >= 9,
        "the nine css fixtures are present: {}",
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
                        error.to_string().contains(&format!("(line {line}, column {column})")),
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
fn approved_theme_reserializes_to_identical_bytes() {
    let input = ":root{--md-accent:#123456}";
    let first = guard_author_css(input).expect("approves the theme");
    let second = guard_author_css(input).expect("approves the theme");
    assert_eq!(first, second, "re-serialization must be byte-stable");
    assert_eq!(
        first, ":root {\n  --md-accent: #123456;\n}\n",
        "the pinned deterministic serialization"
    );
}

#[test]
fn approved_theme_embeds_byte_stable_user_style_across_builds() {
    let dir = std::env::temp_dir().join(format!("mdhtml-security-css-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    fs::write(dir.join("custom.theme.css"), ":root{--md-accent:#123456}").expect("write theme");

    let source = "---\ntitle: T\ntheme: custom.theme.css\n---\n\n# Body\n";
    let first =
        build(source, &dir, &runtime_dist(), &themes_dir(), &fonts_dir()).expect("first build");
    let second =
        build(source, &dir, &runtime_dist(), &themes_dir(), &fonts_dir()).expect("second build");
    assert_eq!(
        between(&first, "<style id=\"mdhtml-user\">", "</style>"),
        between(&second, "<style id=\"mdhtml-user\">", "</style>"),
        "the embedded user style is deterministic"
    );
    assert_eq!(
        between(&first, "<style id=\"mdhtml-user\">", "</style>"),
        ":root {\n  --md-accent: #123456;\n}\n"
    );
}

fn between<'a>(html: &'a str, open: &str, close: &str) -> &'a str {
    let start = html.find(open).expect("open marker") + open.len();
    let end = html[start..].find(close).expect("close marker") + start;
    &html[start..end]
}

#[test]
fn network_url_inside_a_custom_property_is_denied() {
    let violation = guard_author_css(":root { --bg: url(https://evil.example/pixel.png); }")
        .expect_err("a url() in a custom-property token stream is network-capable");
    assert_eq!(violation.code, "E-MDHSEC-009");
}

#[test]
fn every_allowlisted_at_rule_parses_and_approves() {
    let css = "@media (prefers-color-scheme: dark) { :root { color: white; } }\n\
               @container (min-width: 100px) { .a { color: blue; } }\n\
               @supports (display: grid) { .b { display: grid; } }\n\
               @layer base { .c { color: green; } }\n\
               @scope (.card) { :scope { color: red; } }\n\
               @page { margin: 1cm; }\n\
               @counter-style thumbs { system: cyclic; symbols: \"A\"; suffix: \" \"; }\n\
               @keyframes pulse { from { opacity: 1; } to { opacity: 0.5; } }";
    guard_author_css(css).expect("the frozen at-rule allowlist approves");
}

#[test]
fn style_close_sequence_in_a_string_value_is_rejected() {
    let payload = "a::after { content: \"</style><img src=x onerror=alert(1)>\" }";
    let violation = guard_author_css(payload)
        .expect_err("a string that re-serializes to a literal </style is a context escape");
    assert_eq!(violation.code, "E-MDHSEC-007");
}
