use mdhtml::frontmatter::{Value, parse_front_matter};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

struct JsonDecoder<'a> {
    input: &'a [u8],
    index: usize,
}

#[derive(Debug)]
struct Fixture {
    id: String,
    status: String,
    diagnostic: Option<String>,
    source: String,
    expected: Option<Json>,
}

fn decode_json(input: &str) -> Json {
    let mut decoder = JsonDecoder {
        input: input.as_bytes(),
        index: 0,
    };
    let value = decoder.value();
    decoder.whitespace();
    assert_eq!(
        decoder.index,
        decoder.input.len(),
        "JSON has trailing input"
    );
    value
}

impl JsonDecoder<'_> {
    fn whitespace(&mut self) {
        while matches!(
            self.input.get(self.index),
            Some(b' ' | b'\n' | b'\r' | b'\t')
        ) {
            self.index += 1;
        }
    }

    fn value(&mut self) -> Json {
        self.whitespace();
        match self.input.get(self.index) {
            Some(b'n') => {
                self.expect_bytes(b"null");
                Json::Null
            }
            Some(b't') => {
                self.expect_bytes(b"true");
                Json::Bool(true)
            }
            Some(b'f') => {
                self.expect_bytes(b"false");
                Json::Bool(false)
            }
            Some(b'"') => Json::String(self.string()),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'-' | b'0'..=b'9') => self.number(),
            other => panic!("unexpected JSON byte {other:?} at {}", self.index),
        }
    }

    fn expect_bytes(&mut self, expected: &[u8]) {
        assert_eq!(
            &self.input[self.index..self.index + expected.len()],
            expected
        );
        self.index += expected.len();
    }

    fn string(&mut self) -> String {
        assert_eq!(self.input[self.index], b'"');
        self.index += 1;
        let mut output = String::new();
        loop {
            match self.input[self.index] {
                b'"' => {
                    self.index += 1;
                    return output;
                }
                b'\\' => {
                    self.index += 1;
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
                        b'u' => {
                            let hex = &self.input[self.index..self.index + 4];
                            let value = std::str::from_utf8(hex)
                                .unwrap()
                                .chars()
                                .try_fold(0u32, |value, c| Some(value * 16 + c.to_digit(16)?))
                                .unwrap();
                            output.push(char::from_u32(value).unwrap());
                            self.index += 4;
                        }
                        other => panic!("unsupported JSON escape {other:?}"),
                    }
                }
                byte if byte >= 0x20 => {
                    let rest = std::str::from_utf8(&self.input[self.index..]).unwrap();
                    let character = rest.chars().next().unwrap();
                    output.push(character);
                    self.index += character.len_utf8();
                }
                other => panic!("invalid JSON string byte {other:?}"),
            }
        }
    }

    fn number(&mut self) -> Json {
        let start = self.index;
        while matches!(
            self.input.get(self.index),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.index += 1;
        }
        let number = std::str::from_utf8(&self.input[start..self.index]).unwrap();
        Json::Number(number.parse().unwrap())
    }

    fn array(&mut self) -> Json {
        self.index += 1;
        let mut values = Vec::new();
        self.whitespace();
        if self.input.get(self.index) == Some(&b']') {
            self.index += 1;
            return Json::Array(values);
        }
        loop {
            values.push(self.value());
            self.whitespace();
            match self.input[self.index] {
                b',' => self.index += 1,
                b']' => {
                    self.index += 1;
                    return Json::Array(values);
                }
                other => panic!("unexpected JSON array byte {other:?}"),
            }
        }
    }

    fn object(&mut self) -> Json {
        self.index += 1;
        let mut entries = Vec::new();
        self.whitespace();
        if self.input.get(self.index) == Some(&b'}') {
            self.index += 1;
            return Json::Object(entries);
        }
        loop {
            self.whitespace();
            let key = self.string();
            self.whitespace();
            assert_eq!(self.input[self.index], b':');
            self.index += 1;
            entries.push((key, self.value()));
            self.whitespace();
            match self.input[self.index] {
                b',' => self.index += 1,
                b'}' => {
                    self.index += 1;
                    return Json::Object(entries);
                }
                other => panic!("unexpected JSON object byte {other:?}"),
            }
        }
    }
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn fixtures() -> Vec<Fixture> {
    let mut paths = fs::read_dir(fixture_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("frontmatter-") && name.ends_with(".json"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let Json::Object(object) = decode_json(&fs::read_to_string(path).unwrap()) else {
                panic!("fixture root must be an object")
            };
            let field = |name: &str| {
                object
                    .iter()
                    .find(|(key, _)| key == name)
                    .unwrap_or_else(|| panic!("fixture field {name} is missing"))
                    .1
                    .to_owned()
            };
            let Json::String(id) = field("id") else {
                panic!("fixture id must be a string")
            };
            let Json::String(status) = field("status") else {
                panic!("fixture status must be a string")
            };
            let diagnostic =
                object
                    .iter()
                    .find(|(key, _)| key == "diagnostic")
                    .map(|(_, value)| match value {
                        Json::String(value) => value.to_owned(),
                        _ => panic!("fixture diagnostic must be a string"),
                    });
            let Json::String(source) = field("source") else {
                panic!("fixture source must be a string")
            };
            let expected = object
                .iter()
                .find(|(key, _)| key == "expect")
                .and_then(|(_, value)| match value {
                    Json::Object(object) => object
                        .iter()
                        .find(|(key, _)| key == "frontMatter")
                        .map(|(_, value)| value.to_owned()),
                    _ => panic!("fixture expect must be an object"),
                });
            Fixture {
                id,
                status,
                diagnostic,
                source,
                expected,
            }
        })
        .collect()
}

fn json_to_value(json: Json) -> Value {
    match json {
        Json::Null => Value::Null,
        Json::Bool(value) => Value::Bool(value),
        Json::Number(value) => Value::Number(value),
        Json::String(value) => Value::String(value),
        Json::Array(values) => Value::Sequence(values.into_iter().map(json_to_value).collect()),
        Json::Object(entries) => Value::Mapping(
            entries
                .into_iter()
                .map(|(key, value)| (key, json_to_value(value)))
                .collect(),
        ),
    }
}

#[test]
fn shared_fixtures_are_discovered_and_have_structural_parity() {
    let fixtures = fixtures();
    assert!(!fixtures.is_empty());
    let valid = fixtures
        .iter()
        .filter(|fixture| fixture.status == "valid")
        .count();
    let invalid = fixtures
        .iter()
        .filter(|fixture| fixture.status == "invalid")
        .count();
    assert_eq!(valid + invalid, fixtures.len());
    assert_eq!(
        fixtures
            .iter()
            .map(|fixture| &fixture.id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        fixtures.len(),
        "fixture ids must be unique"
    );

    for fixture in fixtures {
        match fixture.status.as_str() {
            "valid" => {
                let parsed = parse_front_matter(&fixture.source).unwrap();
                assert_eq!(
                    parsed.front_matter,
                    json_to_value(fixture.expected.unwrap()),
                    "{}",
                    fixture.id
                );
                assert_eq!(parsed.raw.len() + parsed.body.len(), fixture.source.len());
                assert_eq!(parsed.body_offset, parsed.raw.len());
                assert_eq!(parsed.raw.to_owned() + parsed.body, fixture.source);
            }
            "invalid" => {
                let error = parse_front_matter(&fixture.source).unwrap_err();
                assert_eq!(
                    error.code(),
                    fixture.diagnostic.as_deref().unwrap_or("E-PARSE-01")
                );
                assert!(error.line() >= 1 && error.column() >= 1);
            }
            status => panic!("unsupported fixture status {status}"),
        }
    }
}

#[test]
fn absent_empty_eof_and_crlf_sources_preserve_slices() {
    let absent = "# body\n";
    let parsed = parse_front_matter(absent).unwrap();
    assert_eq!(parsed.front_matter, Value::Mapping(Vec::new()));
    assert_eq!(parsed.raw, "");
    assert_eq!(parsed.body, absent);
    assert_eq!(parsed.body_offset, 0);

    let parsed = parse_front_matter("---\n---\n# body\n").unwrap();
    assert_eq!(parsed.front_matter, Value::Mapping(Vec::new()));
    assert_eq!(parsed.raw, "---\n---\n");
    assert_eq!(parsed.body, "# body\n");

    let source = "---\ntitle: EOF\n---";
    let parsed = parse_front_matter(source).unwrap();
    assert_eq!(parsed.body, "");
    assert_eq!(parsed.raw, source);
    assert_eq!(parsed.body_offset, source.len());

    let source = "---\r\ntitle: CRLF\r\n---\r\n\r\nbody\r\n";
    let parsed = parse_front_matter(source).unwrap();
    assert_eq!(
        parsed.front_matter,
        Value::Mapping(vec![("title".into(), Value::String("CRLF".into()))])
    );
    assert_eq!(parsed.body, "\r\nbody\r\n");
    assert_eq!(parsed.raw, "---\r\ntitle: CRLF\r\n---\r\n");
}

#[test]
fn mappings_preserve_source_order_for_integer_like_keys() {
    let parsed = parse_front_matter("---\nsections:\n  2: cards\n  1: timeline\n---\n").unwrap();
    assert_eq!(
        parsed.front_matter,
        Value::Mapping(vec![(
            "sections".into(),
            Value::Mapping(vec![
                ("2".into(), Value::String("cards".into())),
                ("1".into(), Value::String("timeline".into()))
            ]),
        )])
    );
}

#[test]
fn normative_edges_match_reference_acceptance() {
    let block = parse_front_matter("---\nl: |\n  one\n  two\n\nf: >\n  one\n  two\n---\n").unwrap();
    assert_eq!(
        block.front_matter,
        Value::Mapping(vec![
            ("l".into(), Value::String("one\ntwo\n".into())),
            ("f".into(), Value::String("one two\n".into())),
        ])
    );

    let quoted_key =
        parse_front_matter("---\nitems:\n  - \"name\": Ada\n    'role': dev\n---\n").unwrap();
    assert!(matches!(quoted_key.front_matter, Value::Mapping(_)));

    for marker in ["!tag", "&anchor", "*alias"] {
        for source in [
            format!("---\n{marker}: value\n---\n"),
            format!("---\na: {marker}\n---\n"),
            format!("---\nitems:\n  - {marker}: value\n---\n"),
            format!("---\na: {{ {marker}: value }}\n---\n"),
        ] {
            assert!(
                parse_front_matter(&source).is_err(),
                "accepted invalid source: {source:?}"
            );
        }
    }
    for source in [
        "---\na: !!str hello\n---\n",
        "---\na: one: two\n---\n",
        "---\na: \"value\"#not-comment\n---\n",
        "---\na: [1]#not-comment\n---\n",
        "---\na: { b: 1 }#not-comment\n---\n",
    ] {
        assert!(
            parse_front_matter(source).is_err(),
            "accepted invalid source: {source:?}"
        );
    }

    let comments = parse_front_matter(
        "---\na: \"x\" # comment\nb: [1] # comment\nc: { d: 2 } # comment\n---\n",
    )
    .unwrap();
    assert_eq!(
        comments.front_matter,
        Value::Mapping(vec![
            ("a".into(), Value::String("x".into())),
            ("b".into(), Value::Sequence(vec![Value::Number(1.0)])),
            (
                "c".into(),
                Value::Mapping(vec![("d".into(), Value::Number(2.0))]),
            ),
        ])
    );
}

#[test]
fn rejects_malformed_mapping_syntax_in_sequence_items() {
    for source in [
        "---\nitems:\n  - foo : bar\n---\n",
        "---\nitems:\n  - - foo : bar\n---\n",
    ] {
        assert!(
            parse_front_matter(source).is_err(),
            "accepted invalid source: {source:?}"
        );
    }
}

#[test]
fn comments_before_nested_mapping_values_preserve_children() {
    let parsed = parse_front_matter("---\na: # comment\n  b: 1\n---\n").unwrap();
    assert_eq!(
        parsed.front_matter,
        Value::Mapping(vec![(
            "a".into(),
            Value::Mapping(vec![("b".into(), Value::Number(1.0))]),
        )])
    );
}

#[test]
fn errors_are_stable_and_do_not_leak_internals() {
    let error = parse_front_matter("---\nvalue: 1e309\n---\n").unwrap_err();
    assert_eq!(error.code(), "E-PARSE-01");
    assert_eq!(error.line(), 2);
    assert_eq!(error.column(), 8);
    assert!(!error.message().contains('\n'));
    assert!(!error.message().contains('/'));

    let error = parse_front_matter("---\na: 1\n b: 2\n---\n").unwrap_err();
    assert_eq!(error.line(), 3);
    assert_eq!(error.column(), 2);
    assert_eq!(error.to_string(), error.message());

    let error = parse_front_matter("---\na: 1\n").unwrap_err();
    assert_eq!(error.line(), 3);
    assert_eq!(error.column(), 1);
}
