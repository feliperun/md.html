use super::scalar::{assert_plain_start, resolve_scalar, scan_quoted};
use super::{FrontMatterError, Parser, Value, is_space};

impl Parser<'_> {
    pub(super) fn parse_flow(
        &self,
        text: &str,
        start: usize,
        line: usize,
    ) -> Result<(Value, usize), FrontMatterError> {
        let open = text.as_bytes()[start];
        let close = if open == b'[' { b']' } else { b'}' };
        let is_map = open == b'{';
        let mut index = start + 1;
        let mut values = Vec::new();
        let mut entries = Vec::new();
        let mut seen = Vec::new();
        loop {
            index = skip_flow_spaces(text, index);
            if index >= text.len() {
                return Err(self.error("unterminated flow collection", line, start + 1));
            }
            if text.as_bytes()[index] == close {
                return Ok(self.finish_flow(index, is_map, values, entries));
            }
            if text.as_bytes()[index] == b',' {
                return Err(self.error("empty element in flow collection", line, index + 1));
            }
            if is_map {
                let ((key, value), end) = self.parse_flow_entry(text, index, close, line)?;
                index = end;
                if seen.iter().any(|seen: &String| seen == &key) {
                    return Err(self.error("duplicate key", line, index + 1));
                }
                seen.push(key.clone());
                entries.push((key, value));
            } else {
                let (value, end) = self.parse_flow_value(text, index, line)?;
                index = end;
                values.push(value);
            }
            index = skip_flow_spaces(text, index);
            match text.as_bytes().get(index) {
                Some(byte) if *byte == close => {
                    return Ok(self.finish_flow(index, is_map, values, entries));
                }
                Some(b',') => index += 1,
                _ => {
                    return Err(self.error(
                        "expected ',' or closing bracket in flow collection",
                        line,
                        index + 1,
                    ));
                }
            }
        }
    }

    fn parse_flow_entry(
        &self,
        text: &str,
        index: usize,
        close: u8,
        line: usize,
    ) -> Result<((String, Value), usize), FrontMatterError> {
        let (key, end) = if matches!(text.as_bytes()[index], b'\'' | b'"') {
            scan_quoted(self, text, index, line)?
        } else {
            self.scan_flow_plain_key(text, index, line)?
        };
        let index = skip_flow_spaces(text, end);
        if text.as_bytes().get(index) != Some(&b':') {
            return Err(self.error("expected ':' in flow mapping", line, index + 1));
        }
        let index = skip_flow_spaces(text, index + 1);
        if index >= text.len() || text.as_bytes()[index] == b',' || text.as_bytes()[index] == close
        {
            return Err(self.error("missing value in flow mapping", line, index + 1));
        }
        let (value, end) = self.parse_flow_value(text, index, line)?;
        Ok(((key, value), end))
    }

    fn finish_flow(
        &self,
        index: usize,
        is_map: bool,
        values: Vec<Value>,
        entries: Vec<(String, Value)>,
    ) -> (Value, usize) {
        let value = if is_map {
            Value::Mapping(entries)
        } else {
            Value::Sequence(values)
        };
        (value, index + 1)
    }

    fn scan_flow_plain_key(
        &self,
        text: &str,
        start: usize,
        line: usize,
    ) -> Result<(String, usize), FrontMatterError> {
        let mut index = start;
        while index < text.len() {
            let byte = text.as_bytes()[index];
            if byte == b':' {
                let raw = text[start..index].trim();
                if raw.is_empty() {
                    return Err(self.error("missing key in flow mapping", line, start + 1));
                }
                assert_plain_start(self, raw, line, start + 1)?;
                return Ok((
                    match resolve_scalar(self, raw, line, start + 1)? {
                        Value::String(value) => value,
                        Value::Null => "null".into(),
                        Value::Bool(value) => value.to_string(),
                        Value::Number(value) => value.to_string(),
                        Value::Sequence(_) | Value::Mapping(_) => unreachable!(),
                    },
                    index,
                ));
            }
            if matches!(byte, b',' | b'}' | b']' | b'[' | b'{') {
                return Err(self.error("expected ':' in flow mapping key", line, index + 1));
            }
            index += 1;
        }
        Err(self.error("expected ':' in flow mapping key", line, start + 1))
    }

    fn parse_flow_value(
        &self,
        text: &str,
        start: usize,
        line: usize,
    ) -> Result<(Value, usize), FrontMatterError> {
        match text.as_bytes()[start] {
            b'[' | b'{' => self.parse_flow(text, start, line),
            b'\'' | b'"' => {
                let (value, end) = scan_quoted(self, text, start, line)?;
                Ok((Value::String(value), end))
            }
            _ => {
                let mut index = start;
                let mut saw_space = false;
                while index < text.len() {
                    let byte = text.as_bytes()[index];
                    if matches!(byte, b',' | b']' | b'}') {
                        break;
                    }
                    if byte == b' ' {
                        saw_space = true;
                        index += 1;
                        continue;
                    }
                    if byte == b'#' && saw_space {
                        break;
                    }
                    if byte == b':'
                        && (index + 1 == text.len()
                            || is_space(text.as_bytes()[index + 1])
                            || matches!(text.as_bytes()[index + 1], b',' | b']' | b'}'))
                    {
                        return Err(self.error(
                            "mapping value not allowed in flow scalar",
                            line,
                            index + 1,
                        ));
                    }
                    if matches!(byte, b'[' | b'{' | b'\'' | b'"') {
                        return Err(self.error(
                            "unexpected flow indicator in plain scalar",
                            line,
                            index + 1,
                        ));
                    }
                    saw_space = false;
                    index += 1;
                }
                let raw = text[start..index].trim();
                if raw.is_empty() {
                    return Err(self.error("missing value in flow collection", line, start + 1));
                }
                assert_plain_start(self, raw, line, start + 1)?;
                Ok((resolve_scalar(self, raw, line, start + 1)?, index))
            }
        }
    }

    pub(super) fn check_trailing(
        &self,
        text: &str,
        end: usize,
        line: usize,
    ) -> Result<(), FrontMatterError> {
        let mut index = end;
        let mut separated = false;
        while index < text.len() && is_space(text.as_bytes()[index]) {
            separated = true;
            index += 1;
        }
        if index < text.len() && (text.as_bytes()[index] != b'#' || !separated) {
            return Err(self.error("unexpected content after value", line, index + 1));
        }
        Ok(())
    }
}

fn skip_flow_spaces(text: &str, mut index: usize) -> usize {
    while text.as_bytes().get(index) == Some(&b' ') {
        index += 1;
    }
    index
}
