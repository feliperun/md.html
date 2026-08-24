//! Phase 6 adversarial corpus walk (PRD §15 categories `mutation-xss`,
//! `malformed`, `runtime`, `external`): every
//! `fixtures/security/<category>-*.json` case runs through the build path —
//! valid cases must build, audit SAFE and round-trip through `extract`;
//! invalid cases must fail with the exact frozen diagnostic (an `E-MDHSEC-*`
//! code, or the `E-FMT-02` stored-source terminator the mXSS breakout cases
//! pin). Cases with `kind: "artifact"` audit instead of building. Render-time
//! neutralization of the mXSS payloads is asserted by the Node harness in
//! `runtime/test/security-mutation-xss.test.mjs`, which walks the same files.

use std::fs;
use std::path::{Path, PathBuf};

use mdhtml::audit::audit_artifact;
use mdhtml::build::{build, build_unsafe};
use mdhtml::extract::extract_source;

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

struct AdversarialFixture {
    id: String,
    category: String,
    status: String,
    artifact: bool,
    diagnostic: Option<String>,
    location: Option<String>,
    source: Option<String>,
    html: Option<String>,
    unsafe_build: bool,
    assets: Vec<(String, Vec<u8>)>,
}

/// The fixture fields the harness needs: strings, a bool, and a nested object
/// of strings (`assets`).
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

fn load_fixture(path: &Path) -> AdversarialFixture {
    let raw = fs::read_to_string(path).expect("fixture is readable");
    let Json::Obj(object) = JsonParser::parse(&raw) else {
        panic!("fixture root must be an object");
    };
    let mut assets = Vec::new();
    if let Some(Json::Obj(entries)) = field(&object, "assets") {
        for (name, value) in entries {
            let Json::Str(payload) = value else {
                panic!("asset {name} must be a base64 string");
            };
            assets.push((name.clone(), decode_base64(payload)));
        }
    }
    AdversarialFixture {
        id: string_field(&object, "id").expect("fixture id"),
        category: string_field(&object, "category").expect("fixture category"),
        status: string_field(&object, "status").expect("fixture status"),
        artifact: string_field(&object, "kind").as_deref() == Some("artifact"),
        diagnostic: string_field(&object, "diagnostic"),
        location: string_field(&object, "location"),
        source: string_field(&object, "source"),
        html: string_field(&object, "html"),
        unsafe_build: matches!(field(&object, "unsafe"), Some(Json::Bool(true))),
        assets,
    }
}

/// The Phase 6 walk set: the four new category prefixes under
/// fixtures/security, in deterministic sorted order.
fn cases() -> Vec<(String, PathBuf)> {
    let prefixes = ["mutation-xss-", "malformed-", "runtime-", "external-"];
    let mut cases = Vec::new();
    for entry in fs::read_dir(security_dir()).expect("fixtures/security is readable") {
        let path = entry.expect("readable entry").path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.ends_with(".json") {
            continue;
        }
        if let Some(prefix) = prefixes.iter().find(|prefix| name.starts_with(**prefix)) {
            cases.push((prefix.trim_end_matches('-').to_string(), path));
        }
    }
    cases.sort();
    cases
}

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mdhtml-security-adversarial-{}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn materialize_assets(dir: &Path, fixture: &AdversarialFixture) {
    for (name, bytes) in &fixture.assets {
        fs::write(dir.join(name), bytes).expect("write materialized asset");
    }
}

fn build_in(
    dir: &Path,
    source: &str,
    unsafe_mode: bool,
) -> Result<String, mdhtml::build::BuildError> {
    if unsafe_mode {
        build_unsafe(source, dir, &runtime_dist(), &themes_dir(), &fonts_dir())
    } else {
        build(source, dir, &runtime_dist(), &themes_dir(), &fonts_dir())
    }
}

fn assert_location(context: &str, message: &str, location: &Option<String>) {
    if let Some(location) = location {
        let (line, column) = location
            .split_once(':')
            .expect("fixture location is LINE:COLUMN");
        assert!(
            message.contains(&format!("(line {line}, column {column})")),
            "{context} must cite line {line}, column {column}: {message}"
        );
    }
}

#[test]
fn adversarial_build_cases_reach_a_deterministic_verdict() {
    let dir = temp_dir();
    let cases = cases();
    let build_cases: Vec<_> = cases
        .iter()
        .filter(|(_, path)| {
            let fixture = load_fixture(path);
            !fixture.artifact && fixture.source.is_some()
        })
        .collect();
    assert!(
        build_cases.len() >= 15,
        "the mutation-xss corpus is present: {}",
        build_cases.len()
    );

    for (category, path) in build_cases {
        let fixture = load_fixture(path);
        assert_eq!(
            fixture.category.as_str(),
            category.as_str(),
            "{} category matches its file prefix",
            fixture.id
        );
        materialize_assets(&dir, &fixture);
        let result = build_in(&dir, fixture.source.as_deref().expect("source"), false);
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
                assert_location(&fixture.id, &error.to_string(), &fixture.location);
            }
            "valid" => {
                let built = result.expect(&format!("{} must build cleanly", fixture.id));
                let report = audit_artifact(&built);
                assert!(
                    report.safe,
                    "{} must audit SAFE after building: {}",
                    fixture.id,
                    report.render()
                );
                let extracted = extract_source(built.as_bytes())
                    .expect(&format!("{} must extract", fixture.id));
                assert_eq!(
                    extracted,
                    fixture.source.as_deref().expect("source").as_bytes(),
                    "{} must round-trip extract(build(source)) == source",
                    fixture.id
                );
            }
            "unsafe" => {
                let error = result.expect_err(&format!("{} must fail the safe build", fixture.id));
                let expected = fixture
                    .diagnostic
                    .as_deref()
                    .expect("unsafe fixtures carry a diagnostic");
                assert_eq!(
                    error.code(),
                    expected,
                    "{} must fail the safe build with exactly {expected}",
                    fixture.id
                );
                let built = build_in(&dir, fixture.source.as_deref().expect("source"), true)
                    .expect(&format!("{} must build with --unsafe", fixture.id));
                assert!(
                    built.contains("data-mdhtml-safe=\"false\""),
                    "{} must carry the unsafe attestation",
                    fixture.id
                );
                let extracted = extract_source(built.as_bytes())
                    .expect(&format!("{} must extract unsafely", fixture.id));
                assert_eq!(
                    extracted,
                    fixture.source.as_deref().expect("source").as_bytes(),
                    "{} must round-trip even when built unsafely",
                    fixture.id
                );
            }
            other => panic!("fixture {} has unknown status {other}", fixture.id),
        }
    }
}

#[test]
fn adversarial_artifact_cases_audit_to_the_frozen_verdicts() {
    let dir = temp_dir();
    let cases = cases();
    let artifact_cases: Vec<_> = cases
        .iter()
        .filter(|(_, path)| load_fixture(path).artifact)
        .collect();

    for (category, path) in artifact_cases {
        let fixture = load_fixture(path);
        assert_eq!(
            fixture.category.as_str(),
            category.as_str(),
            "{} category matches",
            fixture.id
        );
        let html = match (&fixture.source, &fixture.html) {
            (Some(source), None) => {
                materialize_assets(&dir, &fixture);
                build_in(&dir, source, fixture.unsafe_build)
                    .expect(&format!("{} must build", fixture.id))
            }
            (None, Some(html)) => html.clone(),
            _ => panic!("artifact fixtures carry exactly one of source or html"),
        };
        let report = audit_artifact(&html);
        match fixture.status.as_str() {
            "valid" => {
                assert!(
                    report.safe,
                    "{} must audit SAFE: {}",
                    fixture.id,
                    report.render()
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
                assert_location(&fixture.id, &report.render(), &fixture.location);
            }
            other => panic!("artifact fixture {} has unknown status {other}", fixture.id),
        }
    }
}
