//! T14 extract (CLI-03): byte-exact canonical-source restoration and strict,
//! fail-before-write asset extraction from built `.md.html` artifacts. The
//! artifact is read structurally — raw text of `script`/`style` elements,
//! mirroring the accepted `check` element scan — never re-parsed as Markdown
//! and never re-derived from the canonical source.

use crate::build::BuildError;
use crate::security;

/// A validated, decoded asset block: the declared `data-path`, the declared
/// `data-type`, and the exact payload bytes (standard padded base64, embedded
/// whitespace ignored).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedAsset {
    pub path: String,
    pub data_type: String,
    pub bytes: Vec<u8>,
}

/// A deterministic extraction failure with a stable diagnostic code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractError {
    pub code: &'static str,
    pub message: String,
}

impl ExtractError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<ExtractError> for BuildError {
    fn from(error: ExtractError) -> Self {
        BuildError::new(error.code, error.message)
    }
}

/// Restore the canonical source byte-for-byte (CLI-03): the raw text of the
/// single `script#mdhtml-source[type="text/markdown"]`, verbatim — `<\/script`
/// is never decoded, and newlines and Unicode are never normalized.
pub fn extract_source(artifact: &[u8]) -> Result<Vec<u8>, ExtractError> {
    let html = std::str::from_utf8(artifact)
        .map_err(|_| ExtractError::new("E-CLI-05", "input is not valid UTF-8"))?;
    let elements = scan_elements(html);
    let scripts: Vec<&Element<'_>> = elements
        .iter()
        .filter(|element| element.is("script"))
        .collect();
    let source_scripts: Vec<&Element<'_>> = scripts
        .iter()
        .filter(|element| attr(element, "id") == Some("mdhtml-source"))
        .copied()
        .collect();
    let markdown_scripts: Vec<&Element<'_>> = scripts
        .iter()
        .filter(|element| attr(element, "type") == Some("text/markdown"))
        .copied()
        .collect();
    let source_ok = source_scripts.len() == 1
        && markdown_scripts.len() == 1
        && source_scripts[0]
            .attrs
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("type") && *value == "text/markdown");
    if !source_ok {
        return Err(ExtractError::new(
            "E-FMT-01",
            "document must contain exactly one script#mdhtml-source[type=\"text/markdown\"]",
        ));
    }
    let source = source_scripts[0].text.ok_or_else(|| {
        ExtractError::new(
            "E-FMT-01",
            "document must contain exactly one script#mdhtml-source[type=\"text/markdown\"]",
        )
    })?;
    Ok(source.as_bytes().to_vec())
}

/// Validate and decode every asset block of the artifact in document order
/// (CLI-03). Each block MUST carry a safe relative `data-path` and a
/// `data-type`, the payload MUST be standard padded base64 (embedded
/// whitespace ignored), and `data-path` values MUST be distinct. Any
/// violation fails with `E-CLI-03`; callers must complete every validation
/// before writing anything.
pub fn extract_assets(artifact: &[u8]) -> Result<Vec<ExtractedAsset>, ExtractError> {
    let html = std::str::from_utf8(artifact)
        .map_err(|_| ExtractError::new("E-CLI-05", "input is not valid UTF-8"))?;
    let elements = scan_elements(html);
    let mut extracted = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for element in elements.iter().filter(|element| element.is("script")) {
        if attr(element, "type") != Some("application/octet-stream") {
            continue;
        }
        let Some(path) = attr(element, "data-path") else {
            return Err(ExtractError::new(
                "E-CLI-03",
                "asset block is missing a data-path",
            ));
        };
        if !security::is_safe_relative_path(path) {
            return Err(ExtractError::new(
                "E-CLI-03",
                format!("asset data-path '{path}' is not a safe relative path"),
            ));
        }
        if !seen.insert(path.to_string()) {
            return Err(ExtractError::new(
                "E-CLI-03",
                format!("duplicate asset data-path '{path}'"),
            ));
        }
        let Some(data_type) = attr(element, "data-type") else {
            return Err(ExtractError::new(
                "E-CLI-03",
                format!("asset '{path}' is missing a data-type"),
            ));
        };
        if data_type.is_empty() {
            return Err(ExtractError::new(
                "E-CLI-03",
                format!("asset '{path}' is missing a data-type"),
            ));
        }
        let payload = element.text.unwrap_or("");
        let bytes = decode_base64(payload).ok_or_else(|| {
            ExtractError::new(
                "E-CLI-03",
                format!("asset '{path}' has an invalid base64 payload"),
            )
        })?;
        extracted.push(ExtractedAsset {
            path: path.to_string(),
            data_type: data_type.to_string(),
            bytes,
        });
    }
    Ok(extracted)
}

/// Strict RFC 4648 decoder: standard alphabet with padding, embedded ASCII
/// whitespace ignored. Any other character, misplaced or trailing padding,
/// or a length that is not a multiple of four is invalid.
fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let compact: Vec<char> = text
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    if compact.len() % 4 != 0 {
        return None;
    }
    let mut padding = 0usize;
    let mut padding_started = false;
    for (index, ch) in compact.iter().enumerate() {
        match ch {
            '=' => {
                if index + 2 < compact.len() {
                    return None;
                }
                padding_started = true;
                padding += 1;
            }
            'A'..='Z' | 'a'..='z' | '0'..='9' | '+' | '/' => {
                if padding_started {
                    return None;
                }
            }
            _ => return None,
        }
    }
    if padding > 2 {
        return None;
    }
    let mut buffer = 0u32;
    let mut bits = 0u32;
    let mut bytes = Vec::new();
    for ch in compact {
        if ch == '=' {
            break;
        }
        let value = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => unreachable!("validated above"),
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
        }
    }
    Some(bytes)
}

/// One element of the artifact, scanned structurally: tag name, attributes
/// and — for raw text elements (`script`, `style`) — the text content.
struct Element<'a> {
    name: &'a str,
    attrs: Vec<(&'a str, &'a str)>,
    text: Option<&'a str>,
}

impl Element<'_> {
    fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }
}

fn attr<'a>(element: &Element<'a>, name: &str) -> Option<&'a str> {
    element
        .attrs
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| *value)
}

/// Scan every element of the artifact in source order, extracting the raw
/// text of `script` and `style` elements up to their closing tag.
fn scan_elements(html: &str) -> Vec<Element<'_>> {
    let mut elements = Vec::new();
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt + 1..];
        let name_len = after
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == ':'))
            .unwrap_or(after.len());
        let name = &after[..name_len];
        if name.is_empty() {
            rest = &after;
            continue;
        }
        let (attrs, tag_len) = parse_attributes(&after[name_len..]);
        let tag_end = name_len + tag_len;
        let after_tag = &after[tag_end..];
        let text = if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("style") {
            find_raw_close(after_tag, name)
        } else {
            None
        };
        elements.push(Element {
            name,
            attrs,
            text: text.map(|(text, _)| text),
        });
        let consumed = 1 + tag_end + text.map(|(_, consumed)| consumed).unwrap_or(0);
        rest = &rest[(lt + consumed).min(rest.len())..];
    }
    elements
}

/// Parse tag attributes until the closing `>`, returning the attributes and
/// the number of consumed bytes (including the `>`).
fn parse_attributes(text: &str) -> (Vec<(&str, &str)>, usize) {
    let mut attrs = Vec::new();
    let mut pos = 0;
    loop {
        pos = skip_ascii_ws(text, pos);
        if pos >= text.len() || text.as_bytes()[pos] == b'>' {
            pos += 1;
            break;
        }
        let name_start = pos;
        while pos < text.len() {
            let byte = text.as_bytes()[pos];
            if byte == b'=' || byte.is_ascii_whitespace() || byte == b'>' {
                break;
            }
            pos += 1;
        }
        let name = &text[name_start..pos];
        pos = skip_ascii_ws(text, pos);
        let (value, next) = parse_attr_value(text, pos);
        attrs.push((name, value));
        pos = next;
    }
    (attrs, pos)
}

fn skip_ascii_ws(text: &str, mut pos: usize) -> usize {
    while pos < text.len() && text.as_bytes()[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

fn parse_attr_value(text: &str, mut pos: usize) -> (&str, usize) {
    if pos < text.len() && text.as_bytes()[pos] == b'=' {
        pos += 1;
        pos = skip_ascii_ws(text, pos);
        if pos < text.len() && (text.as_bytes()[pos] == b'"' || text.as_bytes()[pos] == b'\'') {
            let quote = text.as_bytes()[pos];
            pos += 1;
            let value_start = pos;
            while pos < text.len() && text.as_bytes()[pos] != quote {
                pos += 1;
            }
            let value = &text[value_start..pos];
            if pos < text.len() {
                pos += 1;
            }
            (value, pos)
        } else {
            let value_start = pos;
            while pos < text.len()
                && !text.as_bytes()[pos].is_ascii_whitespace()
                && text.as_bytes()[pos] != b'>'
            {
                pos += 1;
            }
            (&text[value_start..pos], pos)
        }
    } else {
        ("", pos)
    }
}

/// Locate the closing tag of a raw text element, returning the text content
/// and the number of consumed bytes through the closing `>`.
fn find_raw_close<'a>(text: &'a str, name: &str) -> Option<(&'a str, usize)> {
    let close = format!("</{}", name.to_ascii_lowercase());
    let mut rest = text;
    let mut base = 0;
    loop {
        let index = rest.to_ascii_lowercase().find(&close)?;
        let after_close = &rest[index + close.len()..];
        let trimmed = after_close.trim_start_matches(|ch: char| ch.is_ascii_whitespace());
        if trimmed.starts_with('>') {
            let skipped = after_close.len() - trimmed.len();
            let end = index + close.len() + skipped + 1;
            return Some((&text[base..base + index], base + end));
        }
        let advance = index + close.len();
        base += advance;
        rest = &rest[advance..];
    }
}
