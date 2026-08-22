use super::{FrontMatterError, Parser, Value, is_space};

pub(super) fn resolve_scalar(
    parser: &Parser<'_>,
    raw: &str,
    line: usize,
    column: usize,
) -> Result<Value, FrontMatterError> {
    if raw == "null" {
        return Ok(Value::Null);
    }
    if raw == "true" {
        return Ok(Value::Bool(true));
    }
    if raw == "false" {
        return Ok(Value::Bool(false));
    }
    if numeric_syntax(raw) {
        let value = raw
            .parse::<f64>()
            .map_err(|_| parser.error("numeric scalar must be finite", line, column))?;
        if !value.is_finite() {
            return Err(parser.error("numeric scalar must be finite", line, column));
        }
        return Ok(Value::Number(value));
    }
    Ok(Value::String(raw.to_owned()))
}

pub(super) fn assert_plain_start(
    parser: &Parser<'_>,
    raw: &str,
    line: usize,
    column: usize,
) -> Result<(), FrontMatterError> {
    match raw.as_bytes().first().copied() {
        Some(b'!') => Err(parser.error("tags are not supported", line, column)),
        Some(b'&') => Err(parser.error("anchors are not supported", line, column)),
        Some(b'*') => Err(parser.error("aliases are not supported", line, column)),
        Some(b'%' | b'@' | b'`') => Err(parser.error(
            format!(
                "plain scalar cannot start with '{}'",
                raw.as_bytes()[0] as char
            ),
            line,
            column,
        )),
        _ => Ok(()),
    }
}

pub(super) fn scan_quoted(
    parser: &Parser<'_>,
    text: &str,
    start: usize,
    line: usize,
) -> Result<(String, usize), FrontMatterError> {
    let quote = text.as_bytes()[start];
    let mut index = start + 1;
    let mut output = String::new();
    while index < text.len() {
        let byte = text.as_bytes()[index];
        if quote == b'\'' {
            if byte == b'\'' {
                if text.as_bytes().get(index + 1) == Some(&b'\'') {
                    output.push('\'');
                    index += 2;
                    continue;
                }
                return Ok((output, index + 1));
            }
            let rest = &text[index..];
            let character = rest.chars().next().unwrap();
            output.push(character);
            index += character.len_utf8();
            continue;
        }
        if byte == b'"' {
            return Ok((output, index + 1));
        }
        if byte == b'\\' {
            let escape = *text
                .as_bytes()
                .get(index + 1)
                .ok_or_else(|| parser.error("unterminated quoted scalar", line, start + 1))?;
            let (character, consumed) = match escape {
                b'\\' => ('\\', 2),
                b'"' => ('"', 2),
                b'n' => ('\n', 2),
                b't' => ('\t', 2),
                b'r' => ('\r', 2),
                b'b' => ('\u{0008}', 2),
                b'f' => ('\u{000c}', 2),
                b'0' => ('\0', 2),
                b'u' => {
                    let hex = text
                        .get(index + 2..index + 6)
                        .ok_or_else(|| parser.error("invalid unicode escape", line, index + 1))?;
                    let value = u32::from_str_radix(hex, 16)
                        .map_err(|_| parser.error("invalid unicode escape", line, index + 1))?;
                    let character = char::from_u32(value)
                        .ok_or_else(|| parser.error("invalid unicode escape", line, index + 1))?;
                    (character, 6)
                }
                _ => return Err(parser.error("invalid escape sequence", line, index + 1)),
            };
            output.push(character);
            index += consumed;
            continue;
        }
        let rest = &text[index..];
        let character = rest.chars().next().unwrap();
        output.push(character);
        index += character.len_utf8();
    }
    Err(parser.error("unterminated quoted scalar", line, start + 1))
}

fn numeric_syntax(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut start = 0;
    if bytes.first() == Some(&b'-') {
        start = 1;
    }
    if start == bytes.len() {
        return false;
    }
    let mut exponent = None;
    for index in start..bytes.len() {
        if bytes[index] == b'e' || bytes[index] == b'E' {
            if exponent.is_some() {
                return false;
            }
            exponent = Some(index);
        }
    }
    let mantissa_end = exponent.unwrap_or(bytes.len());
    let mantissa = &bytes[start..mantissa_end];
    let exponent_valid = exponent.is_none_or(|index| {
        let mut index = index + 1;
        if bytes
            .get(index)
            .is_some_and(|byte| *byte == b'+' || *byte == b'-')
        {
            index += 1;
        }
        index < bytes.len() && bytes[index..].iter().all(u8::is_ascii_digit)
    });
    if !exponent_valid {
        return false;
    }
    let dots = mantissa.iter().filter(|byte| **byte == b'.').count();
    if dots == 0 {
        return mantissa.iter().all(u8::is_ascii_digit);
    }
    if dots != 1 {
        return false;
    }
    let dot = mantissa.iter().position(|byte| *byte == b'.').unwrap();
    let left = &mantissa[..dot];
    let right = &mantissa[dot + 1..];
    (!left.is_empty() || !right.is_empty())
        && left.iter().all(u8::is_ascii_digit)
        && right.iter().all(u8::is_ascii_digit)
}

pub(super) fn reject_mapping_colon(
    parser: &Parser<'_>,
    raw: &str,
    line: usize,
    column: usize,
) -> Result<(), FrontMatterError> {
    let bytes = raw.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] == b':' && (index + 1 == bytes.len() || is_space(bytes[index + 1])) {
            return Err(parser.error(
                "mapping value not allowed in plain scalar",
                line,
                column + index,
            ));
        }
    }
    Ok(())
}
