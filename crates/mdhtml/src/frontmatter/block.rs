use super::scalar::{assert_plain_start, reject_mapping_colon, resolve_scalar, scan_quoted};
use super::{FrontMatterError, Parser, Value, is_sequence_indicator, is_space};

impl Parser<'_> {
    pub(super) fn parse_block(
        &mut self,
        indent: usize,
        top: bool,
    ) -> Result<Value, FrontMatterError> {
        let Some(significant) = self.peek()? else {
            return Ok(Value::Null);
        };
        let text = self.lines[significant.index].text;
        if significant.column == 0 && text == "---" {
            return Ok(Value::Null);
        }
        if significant.column == 0 && text == "..." {
            return Err(self.error(
                "document end marker is not allowed",
                significant.index + 1,
                1,
            ));
        }
        if significant.column < indent {
            return Ok(Value::Null);
        }
        if significant.column > indent {
            return Err(self.error(
                "inconsistent indentation",
                significant.index + 1,
                significant.column + 1,
            ));
        }
        if is_sequence_indicator(text, significant.column) {
            if top {
                return Err(self.error(
                    "front matter must be a mapping",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            return self.parse_sequence(indent);
        }
        let mut entries = Vec::new();
        let mut seen = Vec::new();
        loop {
            let Some(significant) = self.peek()? else {
                break;
            };
            let text = self.lines[significant.index].text;
            if significant.column == 0 && text == "---" {
                break;
            }
            if significant.column == 0 && text == "..." {
                return Err(self.error(
                    "document end marker is not allowed",
                    significant.index + 1,
                    1,
                ));
            }
            if significant.column < indent {
                break;
            }
            if significant.column > indent {
                return Err(self.error(
                    "inconsistent indentation",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            if is_sequence_indicator(text, significant.column) {
                return Err(self.error(
                    "cannot mix mappings and sequences at the same indentation",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            self.index = significant.index + 1;
            let (key, value) = self.parse_map_entry(indent, significant.index)?;
            if seen.iter().any(|seen: &String| seen == &key) {
                return Err(self.error(
                    "duplicate key",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            seen.push(key.clone());
            entries.push((key, value));
        }
        Ok(Value::Mapping(entries))
    }

    fn parse_sequence(&mut self, indent: usize) -> Result<Value, FrontMatterError> {
        let mut values = Vec::new();
        loop {
            let Some(significant) = self.peek()? else {
                break;
            };
            let text = self.lines[significant.index].text;
            if significant.column == 0 && text == "---" {
                break;
            }
            if significant.column == 0 && text == "..." {
                return Err(self.error(
                    "document end marker is not allowed",
                    significant.index + 1,
                    1,
                ));
            }
            if significant.column < indent {
                break;
            }
            if significant.column > indent {
                return Err(self.error(
                    "inconsistent indentation",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            if !is_sequence_indicator(text, significant.column) {
                return Err(self.error(
                    "cannot mix mappings and sequences at the same indentation",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            self.index = significant.index + 1;
            values.push(self.parse_sequence_item(significant.index, indent)?);
        }
        Ok(Value::Sequence(values))
    }

    fn parse_map_entry(
        &mut self,
        indent: usize,
        line_index: usize,
    ) -> Result<(String, Value), FrontMatterError> {
        let text = self.lines[line_index].text.to_owned();
        let line = line_index + 1;
        let (key, colon) = if matches!(text.as_bytes()[indent], b'\'' | b'"') {
            let (key, end) = scan_quoted(self, &text, indent, line)?;
            if text.as_bytes().get(end) != Some(&b':') {
                return Err(self.error("expected ':' after quoted key", line, end + 1));
            }
            if text
                .as_bytes()
                .get(end + 1)
                .is_some_and(|byte| !is_space(*byte))
            {
                return Err(self.error("expected space after ':'", line, end + 2));
            }
            (key, end)
        } else {
            let colon = self.find_plain_colon(&text, indent, line)?;
            let raw = &text[indent..colon];
            assert_plain_start(self, raw, line, indent + 1)?;
            let key = match resolve_scalar(self, raw, line, indent + 1)? {
                Value::String(value) => value,
                Value::Null => "null".into(),
                Value::Bool(value) => value.to_string(),
                Value::Number(value) => value.to_string(),
                Value::Sequence(_) | Value::Mapping(_) => unreachable!(),
            };
            (key, colon)
        };
        let value = self.parse_value_after_colon(&text, colon, indent, line)?;
        Ok((key, value))
    }

    fn find_plain_colon(
        &self,
        text: &str,
        start: usize,
        line: usize,
    ) -> Result<usize, FrontMatterError> {
        self.find_plain_colon_optional(text, start, line)?
            .ok_or_else(|| self.error("expected ':' after key", line, start + 1))
    }

    fn find_plain_colon_optional(
        &self,
        text: &str,
        start: usize,
        line: usize,
    ) -> Result<Option<usize>, FrontMatterError> {
        let bytes = text.as_bytes();
        for index in start..bytes.len() {
            if bytes[index] == b':' && (index + 1 == bytes.len() || is_space(bytes[index + 1])) {
                if index == start || is_space(bytes[index - 1]) {
                    return Err(self.error("unexpected space before ':'", line, index + 1));
                }
                return Ok(Some(index));
            }
            if bytes[index] == b'#' && index > start && is_space(bytes[index - 1]) {
                break;
            }
        }
        Ok(None)
    }

    fn parse_value_after_colon(
        &mut self,
        text: &str,
        colon: usize,
        key_indent: usize,
        line: usize,
    ) -> Result<Value, FrontMatterError> {
        let mut start = colon + 1;
        while start < text.len() && is_space(text.as_bytes()[start]) {
            start += 1;
        }
        if start >= text.len() {
            if let Some(significant) = self.peek()? {
                if significant.column > key_indent {
                    return self.parse_block(significant.column, false);
                }
            }
            return Ok(Value::Null);
        }
        if text.as_bytes()[start] == b'#' {
            if let Some(significant) = self.peek()? {
                if significant.column > key_indent {
                    return self.parse_block(significant.column, false);
                }
            }
            return Ok(Value::Null);
        }
        if matches!(text.as_bytes()[start], b'|' | b'>') {
            let rest = text[start + 1..].trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                return Err(self.error(
                    "unexpected content after block scalar indicator",
                    line,
                    start + 2,
                ));
            }
            return self.parse_block_scalar(key_indent, text.as_bytes()[start], line);
        }
        if matches!(text.as_bytes()[start], b'[' | b'{') {
            let (value, end) = self.parse_flow(text, start, line)?;
            self.check_trailing(text, end, line)?;
            return Ok(value);
        }
        if matches!(text.as_bytes()[start], b'\'' | b'"') {
            let (value, end) = scan_quoted(self, text, start, line)?;
            self.check_trailing(text, end, line)?;
            return Ok(Value::String(value));
        }
        let mut end = text.len();
        for index in start..text.len() {
            if text.as_bytes()[index] == b'#'
                && index > start
                && is_space(text.as_bytes()[index - 1])
            {
                end = index;
                break;
            }
        }
        let raw = text[start..end].trim();
        if raw.is_empty() {
            return Ok(Value::Null);
        }
        assert_plain_start(self, raw, line, start + 1)?;
        reject_mapping_colon(self, raw, line, start + 1)?;
        if raw.starts_with("- ") {
            return Err(self.error(
                "block sequence entry is not allowed as a value",
                line,
                start + 1,
            ));
        }
        resolve_scalar(self, raw, line, start + 1)
    }

    fn parse_sequence_item(
        &mut self,
        line_index: usize,
        indent: usize,
    ) -> Result<Value, FrontMatterError> {
        let text = self.lines[line_index].text.to_owned();
        let line = line_index + 1;
        let mut start = indent + 1;
        while start < text.len() && text.as_bytes()[start] == b' ' {
            start += 1;
        }
        if start >= text.len() || text.as_bytes()[start] == b'#' {
            if let Some(significant) = self.peek()? {
                if significant.column > indent {
                    return self.parse_block(significant.column, false);
                }
            }
            return Ok(Value::Null);
        }
        if text.as_bytes()[start] == b'-' && is_sequence_indicator(&text, start) {
            return self.parse_nested_sequence_item(text, start, line);
        }
        if matches!(text.as_bytes()[start], b'|' | b'>') {
            let rest = text[start + 1..].trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                return Err(self.error(
                    "unexpected content after block scalar indicator",
                    line,
                    start + 2,
                ));
            }
            return self.parse_block_scalar(indent, text.as_bytes()[start], line);
        }
        if matches!(text.as_bytes()[start], b'[' | b'{') {
            let (value, end) = self.parse_flow(&text, start, line)?;
            self.check_trailing(&text, end, line)?;
            return Ok(value);
        }
        if matches!(text.as_bytes()[start], b'\'' | b'"') {
            let (key, end) = scan_quoted(self, &text, start, line)?;
            if text.as_bytes().get(end) == Some(&b':') {
                if text
                    .as_bytes()
                    .get(end + 1)
                    .is_some_and(|byte| !is_space(*byte))
                {
                    return Err(self.error("expected space after ':'", line, end + 2));
                }
                let value = self.parse_value_after_colon(&text, end, start, line)?;
                let mut entries = vec![(key.clone(), value)];
                return self.continue_sequence_map(start, &mut entries, vec![key]);
            }
            self.check_trailing(&text, end, line)?;
            return Ok(Value::String(key));
        }
        if let Some(colon) = self.find_plain_colon_optional(&text, start, line)? {
            let raw = &text[start..colon];
            assert_plain_start(self, raw, line, start + 1)?;
            let key = scalar_key(self, raw, line, start + 1)?;
            let value = self.parse_value_after_colon(&text, colon, start, line)?;
            let mut entries = vec![(key.clone(), value)];
            return self.continue_sequence_map(start, &mut entries, vec![key]);
        }
        let mut end = text.len();
        for index in start..text.len() {
            if text.as_bytes()[index] == b'#'
                && index > start
                && is_space(text.as_bytes()[index - 1])
            {
                end = index;
                break;
            }
        }
        let raw = text[start..end].trim();
        if raw.is_empty() {
            return Ok(Value::Null);
        }
        assert_plain_start(self, raw, line, start + 1)?;
        resolve_scalar(self, raw, line, start + 1)
    }

    fn parse_nested_sequence_item(
        &mut self,
        text: String,
        start: usize,
        line: usize,
    ) -> Result<Value, FrontMatterError> {
        let mut values = Vec::new();
        let nested_indent = start;
        let mut nested = start + 1;
        while nested < text.len() && text.as_bytes()[nested] == b' ' {
            nested += 1;
        }
        if nested == text.len() {
            values.push(Value::Null);
        } else if matches!(text.as_bytes()[nested], b'[' | b'{') {
            let (value, end) = self.parse_flow(&text, nested, line)?;
            self.check_trailing(&text, end, line)?;
            values.push(value);
        } else if matches!(text.as_bytes()[nested], b'\'' | b'"') {
            let (value, end) = scan_quoted(self, &text, nested, line)?;
            self.check_trailing(&text, end, line)?;
            values.push(Value::String(value));
        } else {
            let raw = text[nested..].trim();
            if let Some(colon) = self.find_plain_colon_optional(&text, nested, line)? {
                let key = scalar_key(self, &text[nested..colon], line, nested + 1)?;
                let value = self.parse_value_after_colon(&text, colon, nested, line)?;
                let mut entries = vec![(key.clone(), value)];
                values.push(self.continue_sequence_map(nested, &mut entries, vec![key])?);
            } else {
                assert_plain_start(self, raw, line, nested + 1)?;
                values.push(resolve_scalar(self, raw, line, nested + 1)?);
            }
        }

        loop {
            let Some(significant) = self.peek()? else {
                break;
            };
            let current = self.lines[significant.index].text;
            if significant.column < nested_indent {
                break;
            }
            if significant.column > nested_indent {
                return Err(self.error(
                    "inconsistent indentation",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            if !is_sequence_indicator(current, nested_indent) {
                return Err(self.error(
                    "cannot mix mappings and sequences at the same indentation",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            self.index = significant.index + 1;
            values.push(self.parse_sequence_item(significant.index, nested_indent)?);
        }
        Ok(Value::Sequence(values))
    }

    fn continue_sequence_map(
        &mut self,
        indent: usize,
        entries: &mut Vec<(String, Value)>,
        mut seen: Vec<String>,
    ) -> Result<Value, FrontMatterError> {
        loop {
            let Some(significant) = self.peek()? else {
                break;
            };
            let text = self.lines[significant.index].text;
            if significant.column == 0 && text == "---" {
                break;
            }
            if significant.column == 0 && text == "..." {
                return Err(self.error(
                    "document end marker is not allowed",
                    significant.index + 1,
                    1,
                ));
            }
            if significant.column < indent {
                break;
            }
            if significant.column > indent {
                return Err(self.error(
                    "inconsistent indentation",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            if is_sequence_indicator(text, significant.column) {
                return Err(self.error(
                    "cannot mix mappings and sequences at the same indentation",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            self.index = significant.index + 1;
            let (key, value) = self.parse_map_entry(indent, significant.index)?;
            if seen.iter().any(|seen| seen == &key) {
                return Err(self.error(
                    "duplicate key",
                    significant.index + 1,
                    significant.column + 1,
                ));
            }
            seen.push(key.clone());
            entries.push((key, value));
        }
        Ok(Value::Mapping(std::mem::take(entries)))
    }

    fn parse_block_scalar(
        &mut self,
        indent: usize,
        indicator: u8,
        _line: usize,
    ) -> Result<Value, FrontMatterError> {
        let mut content = Vec::new();
        let mut block_indent = None;
        while self.index < self.lines.len() {
            let current = self.lines[self.index];
            let Some(column) = super::indentation(current.text, self, self.index)? else {
                content.push(String::new());
                self.index += 1;
                continue;
            };
            if column <= indent {
                break;
            }
            let actual_indent = *block_indent.get_or_insert(column);
            if column < actual_indent {
                return Err(self.error(
                    "inconsistent indentation in block scalar",
                    self.index + 1,
                    column + 1,
                ));
            }
            content.push(current.text[actual_indent..].to_owned());
            self.index += 1;
        }
        if indicator == b'|' {
            let mut value = content.join("\n");
            if value.is_empty() {
                return Ok(Value::String(String::new()));
            }
            while value.ends_with('\n') {
                value.pop();
            }
            value.push('\n');
            Ok(Value::String(value))
        } else {
            Ok(Value::String(fold(content)))
        }
    }
}

fn scalar_key(
    parser: &Parser<'_>,
    raw: &str,
    line: usize,
    column: usize,
) -> Result<String, FrontMatterError> {
    match resolve_scalar(parser, raw, line, column)? {
        Value::String(value) => Ok(value),
        Value::Null => Ok("null".into()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::Sequence(_) | Value::Mapping(_) => unreachable!(),
    }
}

fn fold(content: Vec<String>) -> String {
    let mut output = String::new();
    let mut state = "start";
    for raw in content {
        if raw.trim().is_empty() {
            output.push('\n');
            state = "blank";
            continue;
        }
        let extra = raw.len() - raw.trim_start().len();
        let rest = raw.trim_end();
        if extra > 0 {
            output.push('\n');
            output.push_str(rest);
        } else if state == "fold" {
            output.push(' ');
            output.push_str(rest);
        } else if state == "literal" {
            output.push('\n');
            output.push_str(rest);
        } else {
            output.push_str(rest);
        }
        state = "fold";
    }
    while output.ends_with('\n') {
        output.pop();
    }
    if output.is_empty() {
        String::new()
    } else {
        output.push('\n');
        output
    }
}
