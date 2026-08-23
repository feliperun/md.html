//! Security corpus walk (asset/path node): every `fixtures/security/svg-*.json`
//! and `path-*.json` case builds through `mdhtml::build::build` with `assets`
//! materialized next to the source in a temp directory; invalid cases must
//! fail with the exact frozen `diagnostic` code (E-MDHSEC-011/001/013 for
//! SVG asset bytes, E-MDHSEC-014 for unsafe asset paths) before any file is
//! read, and valid cases must build and embed the exact asset bytes.

use std::fs;
use std::path::{Path, PathBuf};

use mdhtml::build::build;

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
        while matches!(self.input.get(self.index), Some(b' ' | b'\t' | b'\n' | b'\r')) {
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
        source,
        assets,
    }
}

/// Materialize one asset, creating any parent directories the relative path
/// needs, so nested paths like `a/b/photo.png` resolve next to the source.
fn materialize(dir: &Path, name: &str, bytes: &[u8]) {
    let target = dir.join(name);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).expect("create asset parent directory");
    }
    fs::write(target, bytes).expect("write materialized asset");
}

/// The walk set for this node: svg-*.json and path-*.json under
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
                        && (name.starts_with("svg-") || name.starts_with("path-"))
                })
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

/// The (path, mime, payload) of every embedded asset block, in order.
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

#[test]
fn svg_and_path_cases_reject_or_build_with_the_frozen_diagnostics() {
    let cases = cases();
    assert!(
        cases.len() >= 9,
        "the nine svg/path fixtures are present: {}",
        cases.len()
    );

    let dir = std::env::temp_dir().join(format!("mdhtml-security-assets-{}", std::process::id()));
    for path in &cases {
        let fixture = load_fixture(path);
        let case_dir = dir.join(&fixture.id);
        fs::create_dir_all(&case_dir).expect("create case directory");
        for (name, bytes) in &fixture.assets {
            materialize(&case_dir, name, bytes);
        }
        let result = build(
            &fixture.source,
            &case_dir,
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
            }
            "valid" => {
                let html = result.expect(&format!("{} must build cleanly", fixture.id));
                let blocks = asset_blocks(&html);
                assert_eq!(
                    blocks.len(),
                    fixture.assets.len(),
                    "{} must embed every referenced asset once",
                    fixture.id
                );
                for (index, (name, bytes)) in fixture.assets.iter().enumerate() {
                    assert_eq!(
                        &blocks[index].0, name,
                        "{} embeds {name} as the first-referenced path",
                        fixture.id
                    );
                    assert_eq!(
                        decode_base64(&blocks[index].2),
                        *bytes,
                        "{} embeds the exact original bytes of {name}",
                        fixture.id
                    );
                }
            }
            other => panic!("fixture {} has unknown status {other}", fixture.id),
        }
        let _ = fs::remove_dir_all(&case_dir);
    }
}

#[test]
fn unsafe_path_rejection_carries_the_offending_path() {
    let dir = std::env::temp_dir().join(format!("mdhtml-security-assets-{}", std::process::id()));
    for (id, source, offender) in [
        (
            "path-parent-traversal",
            "---\ntitle: T\n---\n\n![logo](../escape.png)\n",
            "../escape.png",
        ),
        (
            "path-absolute",
            "---\ntitle: T\n---\n\n![logo](/absolute.png)\n",
            "/absolute.png",
        ),
        (
            "path-backslash-escape",
            "---\ntitle: T\n---\n\n![logo](..\\secret.png)\n",
            "..\\secret.png",
        ),
        (
            "path-scheme-drive",
            "---\ntitle: T\nfigures:\n  C:\\x.png: { align: right }\n---\n\n# Body\n",
            "C:\\x.png",
        ),
    ] {
        let case_dir = dir.join(id);
        fs::create_dir_all(&case_dir).expect("create case directory");
        let error = build(
            source,
            &case_dir,
            &runtime_dist(),
            &themes_dir(),
            &fonts_dir(),
        )
        .expect_err(&format!("{id} must fail the build"));
        assert_eq!(error.code(), "E-MDHSEC-014", "{id}");
        assert!(
            error.to_string().contains(offender),
            "{id} message must name the offending path: {}",
            error
        );
        let _ = fs::remove_dir_all(&case_dir);
    }
}
