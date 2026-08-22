//! Minimal strict JSON parser (RFC 8259) for committed artifact manifests.
//! The CLI is std-only, so a closed JSON subset is parsed by hand; the values
//! produced mirror the frontmatter `Value` shape used by the accepted config.

#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

struct Parser<'a> {
    source: &'a [u8],
    index: usize,
}

pub fn parse(source: &str) -> Result<JsonValue, JsonError> {
    let mut parser = Parser {
        source: source.as_bytes(),
        index: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.index != parser.source.len() {
        return Err(parser.error("trailing content after the top-level JSON value"));
    }
    Ok(value)
}

impl Parser<'_> {
    fn error(&self, message: impl Into<String>) -> JsonError {
        let mut line = 1usize;
        let mut column = 1usize;
        for byte in &self.source[..self.index] {
            if *byte == b'\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        JsonError {
            message: message.into(),
            line,
            column,
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(
            self.source.get(self.index),
            Some(b' ' | b'\t' | b'\n' | b'\r')
        ) {
            self.index += 1;
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        let byte = *self
            .source
            .get(self.index)
            .ok_or_else(|| self.error("unexpected end of JSON input"))?;
        match byte {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'"' => Ok(JsonValue::String(self.parse_string()?)),
            b't' => self.parse_literal("true", JsonValue::Bool(true)),
            b'f' => self.parse_literal("false", JsonValue::Bool(false)),
            b'n' => self.parse_literal("null", JsonValue::Null),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => Err(self.error(format!("unexpected character '{}'", byte as char))),
        }
    }

    fn parse_literal(&mut self, literal: &str, value: JsonValue) -> Result<JsonValue, JsonError> {
        if !self.source[self.index..].starts_with(literal.as_bytes()) {
            return Err(self.error("invalid literal"));
        }
        self.index += literal.len();
        if matches!(
            self.source.get(self.index),
            Some(b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
        ) {
            return Err(self.error(format!("invalid literal '{literal}'")));
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.index;
        if self.source.get(self.index) == Some(&b'-') {
            self.index += 1;
        }
        match self.source.get(self.index) {
            Some(b'0') => self.index += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.source.get(self.index), Some(b'0'..=b'9')) {
                    self.index += 1;
                }
            }
            _ => return Err(self.error("invalid number")),
        }
        if self.source.get(self.index) == Some(&b'.') {
            self.index += 1;
            if !matches!(self.source.get(self.index), Some(b'0'..=b'9')) {
                return Err(self.error("invalid number: expected digits after the decimal point"));
            }
            while matches!(self.source.get(self.index), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }
        if matches!(self.source.get(self.index), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.source.get(self.index), Some(b'+' | b'-')) {
                self.index += 1;
            }
            if !matches!(self.source.get(self.index), Some(b'0'..=b'9')) {
                return Err(self.error("invalid number: expected digits in the exponent"));
            }
            while matches!(self.source.get(self.index), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }
        let text =
            std::str::from_utf8(&self.source[start..self.index]).expect("number slice is ASCII");
        text.parse::<f64>()
            .map(JsonValue::Number)
            .map_err(|_| self.error("number out of range"))
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        self.index += 1;
        let mut out = String::new();
        loop {
            let byte = *self
                .source
                .get(self.index)
                .ok_or_else(|| self.error("unterminated string"))?;
            match byte {
                b'"' => {
                    self.index += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.index += 1;
                    let escape = *self
                        .source
                        .get(self.index)
                        .ok_or_else(|| self.error("unterminated escape sequence"))?;
                    self.index += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.parse_unicode_escape()?),
                        _ => {
                            return Err(self
                                .error(format!("invalid escape sequence '\\{}'", escape as char)));
                        }
                    }
                }
                byte if byte < 0x20 => {
                    return Err(self.error("unescaped control character in string"));
                }
                _ => {
                    let remaining = std::str::from_utf8(&self.source[self.index..])
                        .map_err(|_| self.error("invalid UTF-8 in string"))?;
                    let character = remaining
                        .chars()
                        .next()
                        .expect("current byte is a valid UTF-8 start");
                    out.push(character);
                    self.index += character.len_utf8();
                }
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, JsonError> {
        let unit = self.parse_hex_quad()?;
        if (0xd800..=0xdbff).contains(&unit) {
            if self.source.get(self.index) != Some(&b'\\')
                || self.source.get(self.index + 1) != Some(&b'u')
            {
                return Err(self.error("high surrogate without a following low surrogate"));
            }
            self.index += 2;
            let low = self.parse_hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&low) {
                return Err(self.error("invalid low surrogate in \\u escape"));
            }
            let scalar = 0x1_0000u32 + (((unit - 0xd800) as u32) << 10) + ((low - 0xdc00) as u32);
            return char::from_u32(scalar)
                .ok_or_else(|| self.error("invalid surrogate pair in \\u escape"));
        }
        if (0xdc00..=0xdfff).contains(&unit) {
            return Err(self.error("unexpected low surrogate in \\u escape"));
        }
        char::from_u32(unit as u32).ok_or_else(|| self.error("invalid \\u escape"))
    }

    fn parse_hex_quad(&mut self) -> Result<u16, JsonError> {
        if self.index + 4 > self.source.len() {
            return Err(self.error("truncated \\u escape"));
        }
        let mut value = 0u16;
        for _ in 0..4 {
            let byte = self.source[self.index];
            let digit = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                b'A'..=b'F' => byte - b'A' + 10,
                _ => return Err(self.error("invalid hex digit in \\u escape")),
            };
            value = value * 16 + digit as u16;
            self.index += 1;
        }
        Ok(value)
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        self.index += 1;
        self.skip_whitespace();
        let mut entries = Vec::new();
        if self.source.get(self.index) == Some(&b'}') {
            self.index += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.skip_whitespace();
            if self.source.get(self.index) != Some(&b'"') {
                return Err(self.error("object keys must be double-quoted strings"));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            if self.source.get(self.index) != Some(&b':') {
                return Err(self.error("expected ':' after object key"));
            }
            self.index += 1;
            self.skip_whitespace();
            let value = self.parse_value()?;
            if entries.iter().any(|(existing, _)| *existing == key) {
                return Err(self.error(format!("duplicate object key '{key}'")));
            }
            entries.push((key, value));
            self.skip_whitespace();
            match self.source.get(self.index) {
                Some(b',') => self.index += 1,
                Some(b'}') => {
                    self.index += 1;
                    return Ok(JsonValue::Object(entries));
                }
                _ => return Err(self.error("expected ',' or '}' in object")),
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        self.index += 1;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.source.get(self.index) == Some(&b']') {
            self.index += 1;
            return Ok(JsonValue::Array(values));
        }
        loop {
            self.skip_whitespace();
            values.push(self.parse_value()?);
            self.skip_whitespace();
            match self.source.get(self.index) {
                Some(b',') => self.index += 1,
                Some(b']') => {
                    self.index += 1;
                    return Ok(JsonValue::Array(values));
                }
                _ => return Err(self.error("expected ',' or ']' in array")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JsonValue;
    use super::parse;

    fn parse_object(source: &str) -> Vec<(String, JsonValue)> {
        match parse(source).expect("valid JSON") {
            JsonValue::Object(entries) => entries,
            other => panic!("expected object, got {other:?}"),
        }
    }

    #[test]
    fn parses_manifest_shaped_document() {
        let entries = parse_object(
            r#"{"format":"mdhtml/manifest/1.0","fragments":[{"id":"core","size":50847,"requires":[]}]}"#,
        );
        assert_eq!(
            entries[0].1,
            JsonValue::String("mdhtml/manifest/1.0".into())
        );
        match &entries[1].1 {
            JsonValue::Array(fragments) => {
                assert_eq!(fragments.len(), 1);
                match &fragments[0] {
                    JsonValue::Object(fragment) => {
                        assert_eq!(fragment[0].1, JsonValue::String("core".into()));
                        assert_eq!(fragment[1].1, JsonValue::Number(50847.0));
                        assert_eq!(fragment[2].1, JsonValue::Array(Vec::new()));
                    }
                    other => panic!("expected object, got {other:?}"),
                }
            }
            other => panic!("expected array, got {other:?}"),
        }
    }

    #[test]
    fn parses_surrogate_pairs() {
        assert_eq!(
            parse(r#""\ud83d\ude00""#).expect("surrogate pair"),
            JsonValue::String("\u{1f600}".into())
        );
    }

    #[test]
    fn parses_escapes_and_numbers() {
        assert_eq!(
            parse(r#""a\nb\t\u0041\/\\\"""#).expect("escapes"),
            JsonValue::String("a\nb\tA/\\\"".into())
        );
        assert_eq!(
            parse(r#"[0,-1,2.5,1e3,1.5E-2]"#).expect("numbers"),
            JsonValue::Array(vec![
                JsonValue::Number(0.0),
                JsonValue::Number(-1.0),
                JsonValue::Number(2.5),
                JsonValue::Number(1000.0),
                JsonValue::Number(0.015),
            ])
        );
    }

    #[test]
    fn rejects_non_json() {
        for source in [
            "{} trailing",
            "{unquoted: 1}",
            r#"{"a": 1,}"#,
            r#"{"a": 1, "a": 2}"#,
            "01",
            "[1,]",
            r#""unterminated"#,
        ] {
            assert!(parse(source).is_err(), "must reject {source:?}");
        }
    }
}
