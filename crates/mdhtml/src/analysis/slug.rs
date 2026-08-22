//! Heading identity: inline-text projection (matching runtime `inlineText`),
//! SECT-01 slugify with the generated ASCII decomposition table, explicit-id
//! uniqueness, and section body ranges over scanner evidence.

use std::collections::HashSet;
use std::ops::Range;

use crate::scanner::HeadingEvidence;

use super::slug_ascii;
use super::{AnalyzedSection, Diagnostic};

const ASCII_PUNCTUATION: &[u8] = b"!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

fn is_ascii_punctuation(ch: char) -> bool {
    ch.is_ascii() && ASCII_PUNCTUATION.contains(&(ch as u8))
}

fn is_escaped(chars: &[char], index: usize) -> bool {
    let mut backslashes = 0;
    let mut i = index;
    while i > 0 && chars[i - 1] == '\\' {
        backslashes += 1;
        i -= 1;
    }
    backslashes % 2 == 1
}

fn count_run(chars: &[char], start: usize, end: usize, delimiter: char) -> usize {
    let mut count = 0;
    while start + count < end && chars[start + count] == delimiter {
        count += 1;
    }
    count
}

fn decode_escapes(chars: &[char]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() && is_ascii_punctuation(chars[i + 1]) {
            out.push(chars[i + 1]);
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Label normalization for reference definitions and lookups: trim, collapse
/// whitespace runs to one space, lowercase.
pub(super) fn normalize_label(label: &str) -> String {
    let mut out = String::new();
    let mut in_whitespace = false;
    for ch in label.trim().chars() {
        if ch.is_whitespace() {
            if !in_whitespace {
                out.push(' ');
                in_whitespace = true;
            }
        } else {
            out.extend(ch.to_lowercase());
            in_whitespace = false;
        }
    }
    out
}

fn find_bracket_close(chars: &[char], start: usize, end: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = start;
    while i < end {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == '[' {
            depth += 1;
        } else if chars[i] == ']' {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
        i += 1;
    }
    None
}

fn find_parenthesis_close(chars: &[char], start: usize, end: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = start;
    while i < end {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] == '(' {
            depth += 1;
        } else if chars[i] == ')' {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
        i += 1;
    }
    None
}

/// Destination plus optional double-quoted title, matching the runtime's
/// inline-destination rule; the values are validated but not needed for the
/// projected text.
fn parse_destination(content: &str) -> bool {
    let content = content.trim();
    let mut parts = content.splitn(2, char::is_whitespace);
    let Some(destination) = parts.next() else {
        return false;
    };
    if destination.is_empty() {
        return false;
    }
    match parts.next() {
        Some(rest) => quoted_title(rest),
        None => true,
    }
}

fn quoted_title(rest: &str) -> bool {
    let Some(inner) = rest
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return false;
    };
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if chars.next().is_none() {
                return false;
            }
        } else if ch == '"' {
            return false;
        }
    }
    true
}

fn find_delimiter_close(
    chars: &[char],
    start: usize,
    end: usize,
    delimiter: char,
    length: usize,
    exact: bool,
) -> Option<usize> {
    let mut i = start;
    while i < end {
        if chars[i] == '\\' {
            i += 2;
            continue;
        }
        if chars[i] != delimiter {
            i += 1;
            continue;
        }
        let run = count_run(chars, i, end, delimiter);
        if (exact && run != length) || (!exact && run < length) {
            i += run.max(1);
            continue;
        }
        let close = if exact { i } else { i + run - length };
        if close > start {
            return Some(close);
        }
        i += run.max(1);
    }
    None
}

fn find_code_close(chars: &[char], start: usize, end: usize, run: usize) -> Option<usize> {
    let mut i = start;
    while i < end {
        if chars[i] == '`' {
            let candidate = count_run(chars, i, end, '`');
            if candidate == run {
                return Some(i);
            }
            i += candidate;
        } else {
            i += 1;
        }
    }
    None
}

/// Project the accepted inline syntax to the visible text used by the runtime
/// `inlineText`: escapes decoded, code spans literal, emphasis/strong/strike
/// delimiters removed, link labels and image alt text kept, `[text][id]`
/// reference labels resolved, footnotes dropped, and every unresolved or
/// ordinary construct preserved verbatim.
///
/// Code spans are tokenized first at every nesting level, matching the
/// runtime's `codeSpanSegments`, so delimiters inside a code span never match
/// emphasis, strong, strike, or link brackets (PARSE-03).
fn project_range(
    chars: &[char],
    start: usize,
    end: usize,
    references: &HashSet<String>,
    out: &mut String,
) {
    let mut plain_start = start;
    let mut i = start;
    while i < end {
        if chars[i] == '`' && !is_escaped(chars, i) {
            let run = count_run(chars, i, end, '`');
            if let Some(close) = find_code_close(chars, i + run, end, run) {
                if plain_start < i {
                    project_plain(chars, plain_start, i, references, out);
                }
                for k in i + run..close {
                    out.push(chars[k]);
                }
                i = close + run;
                plain_start = i;
                continue;
            }
            i += run;
            continue;
        }
        i += 1;
    }
    if plain_start < end {
        project_plain(chars, plain_start, end, references, out);
    }
}

/// A text segment between code spans: escapes, brackets, and delimiter
/// matching without code-span shielding (none remain in the segment).
fn project_plain(
    chars: &[char],
    start: usize,
    end: usize,
    references: &HashSet<String>,
    out: &mut String,
) {
    let mut i = start;
    while i < end {
        let ch = chars[i];
        if ch == '\\' && i + 1 < end && is_ascii_punctuation(chars[i + 1]) {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if ch == '!' && i + 1 < end && chars[i + 1] == '[' {
            if let Some(consumed) = project_image(chars, i, end, references, out) {
                i += consumed;
                continue;
            }
            out.push(chars[i]);
            out.push(chars[i + 1]);
            i += 2;
            continue;
        } else if ch == '[' {
            if let Some(consumed) = project_link(chars, i, end, references, out) {
                i += consumed;
                continue;
            }
        }
        if ch == '*' || ch == '_' || ch == '~' {
            let run = count_run(chars, i, end, ch);
            if run >= 2 {
                if let Some(close) = find_delimiter_close(chars, i + 2, end, ch, 2, false) {
                    project_range(chars, i + 2, close, references, out);
                    i = close + 2;
                    continue;
                }
                for k in i..i + run {
                    out.push(chars[k]);
                }
                i += run;
                continue;
            }
            if run == 1 && ch != '~' {
                if let Some(close) = find_delimiter_close(chars, i + 1, end, ch, 1, true) {
                    project_range(chars, i + 1, close, references, out);
                    i = close + 1;
                    continue;
                }
            }
        }
        out.push(ch);
        i += 1;
    }
}

fn project_image(
    chars: &[char],
    start: usize,
    end: usize,
    references: &HashSet<String>,
    out: &mut String,
) -> Option<usize> {
    let label_start = start + 2;
    let label_end = find_bracket_close(chars, label_start, end)?;
    let after = label_end + 1;
    if after < end && chars[after] == '(' {
        let close = find_parenthesis_close(chars, after + 1, end)?;
        let content: String = chars[after + 1..close].iter().collect();
        if parse_destination(&content) {
            out.push_str(&decode_escapes(&chars[label_start..label_end]));
            return Some(close + 1 - start);
        }
        return None;
    }
    if after < end && chars[after] == '[' {
        let reference_end = find_bracket_close(chars, after + 1, end)?;
        let id: String = chars[after + 1..reference_end].iter().collect();
        if references.contains(&normalize_label(&id)) {
            out.push_str(&decode_escapes(&chars[label_start..label_end]));
            return Some(reference_end + 1 - start);
        }
        return None;
    }
    None
}

fn project_link(
    chars: &[char],
    start: usize,
    end: usize,
    references: &HashSet<String>,
    out: &mut String,
) -> Option<usize> {
    let label_start = start + 1;
    let label_end = find_bracket_close(chars, label_start, end)?;
    let label = &chars[label_start..label_end];
    if label.len() > 1 && label[0] == '^' {
        return Some(label_end + 1 - start);
    }
    let after = label_end + 1;
    if after < end && chars[after] == '(' {
        let close = find_parenthesis_close(chars, after + 1, end)?;
        let content: String = chars[after + 1..close].iter().collect();
        if parse_destination(&content) {
            project_range(chars, label_start, label_end, references, out);
            return Some(close + 1 - start);
        }
        return None;
    }
    if after < end && chars[after] == '[' {
        let reference_end = find_bracket_close(chars, after + 1, end)?;
        let id: String = chars[after + 1..reference_end].iter().collect();
        let label: String = label.iter().collect();
        let resolved = if id.is_empty() {
            references.contains(&normalize_label(&label))
        } else {
            references.contains(&normalize_label(&id))
        };
        if resolved {
            project_range(chars, label_start, label_end, references, out);
            return Some(reference_end + 1 - start);
        }
        return None;
    }
    if references.contains(&normalize_label(&label.iter().collect::<String>())) {
        project_range(chars, label_start, label_end, references, out);
        return Some(label_end + 1 - start);
    }
    None
}

/// SECT-01 slug: lowercase, NFD with combining marks dropped (via the
/// generated table), whitespace runs to one hyphen, then only ASCII
/// `[a-z0-9-_]` survives.
pub fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut pending_hyphen = false;
    for ch in value.chars() {
        if ch.is_whitespace() {
            pending_hyphen = true;
            continue;
        }
        if pending_hyphen {
            out.push('-');
            pending_hyphen = false;
        }
        for lower in ch.to_lowercase() {
            if lower.is_ascii() {
                match lower {
                    'a'..='z' | '0'..='9' | '-' | '_' => out.push(lower),
                    _ => {}
                }
            } else if let Some(byte) = slug_ascii::lookup(lower) {
                out.push(char::from(byte));
            }
        }
    }
    if pending_hyphen {
        out.push('-');
    }
    out
}

/// Explicit `{#id}` normalization: trim, lowercase, whitespace runs to `-`.
fn normalize_explicit_id(value: &str) -> String {
    let mut out = String::new();
    let mut pending_hyphen = false;
    for ch in value.trim().chars() {
        if ch.is_whitespace() {
            pending_hyphen = true;
            continue;
        }
        if pending_hyphen {
            out.push('-');
            pending_hyphen = false;
        }
        out.extend(ch.to_lowercase());
    }
    if pending_hyphen {
        out.push('-');
    }
    out
}

/// Reference definitions in the body, registered exactly where the runtime's
/// recursive `parseBlocks` consumes them: at block level inside blockquotes,
/// list items, containers, and footnote content. Fences, headings, rules,
/// quotes, lists, and paragraphs shield their payload from definition
/// matching, mirroring the runtime dispatch order.
pub(super) fn collect_reference_labels(body: &str) -> HashSet<String> {
    let mut labels = HashSet::new();
    let lines: Vec<&str> = body.split('\n').collect();
    collect_block_labels(&lines, 0, lines.len(), &mut labels);
    labels
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn dedent(line: &str, max: usize) -> &str {
    let n = line
        .bytes()
        .take_while(|byte| *byte == b' ')
        .count()
        .min(max);
    &line[n..]
}

fn match_quote(line: &str) -> Option<&str> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = line[indent..].strip_prefix('>')?;
    Some(match rest.strip_prefix(' ') {
        Some(rest) => rest,
        None => rest,
    })
}

#[derive(Clone, Copy)]
struct ListMarker<'a> {
    kind: char,
    ordered: bool,
    start: i64,
    content: &'a str,
    content_indent: usize,
}

fn list_marker_at(line: &str) -> Option<ListMarker<'_>> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let bytes = rest.as_bytes();
    if matches!(bytes.first(), Some(b'-') | Some(b'+') | Some(b'*')) {
        let after = &rest[1..];
        if !after.is_empty() && !matches!(after.as_bytes().first(), Some(b' ') | Some(b'\t')) {
            return None;
        }
        let spaces = after
            .bytes()
            .take_while(|byte| *byte == b' ' || *byte == b'\t')
            .count();
        let content_start = 1 + spaces.min(4);
        let content = &rest[content_start..];
        let content_indent = if content.is_empty() {
            indent + 2
        } else {
            indent + content_start
        };
        return Some(ListMarker {
            kind: bytes[0] as char,
            ordered: false,
            start: 1,
            content,
            content_indent,
        });
    }
    let digits = bytes
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits == 0 || digits > 9 {
        return None;
    }
    if !matches!(bytes.get(digits), Some(b'.') | Some(b')')) {
        return None;
    }
    let after = &rest[digits + 1..];
    if !after.is_empty() && !matches!(after.as_bytes().first(), Some(b' ') | Some(b'\t')) {
        return None;
    }
    let spaces = after
        .bytes()
        .take_while(|byte| *byte == b' ' || *byte == b'\t')
        .count();
    let width = digits + 1;
    let content_start = width + spaces.min(4);
    let content = &rest[content_start..];
    let content_indent = if content.is_empty() {
        indent + width + 1
    } else {
        indent + content_start
    };
    let start = rest[..digits].parse::<i64>().unwrap_or(1);
    Some(ListMarker {
        kind: bytes[digits] as char,
        ordered: true,
        start,
        content,
        content_indent,
    })
}

fn match_atx_heading(line: &str) -> bool {
    let indent = leading_spaces(line);
    if indent > 3 {
        return false;
    }
    let rest = &line[indent..];
    let hashes = rest.bytes().take_while(|byte| *byte == b'#').count();
    (1..=6).contains(&hashes)
        && matches!(rest.as_bytes().get(hashes), None | Some(b' ') | Some(b'\t'))
}

fn is_hr(line: &str) -> bool {
    let indent = leading_spaces(line);
    if indent > 3 {
        return false;
    }
    let rest = &line[indent..];
    let Some(marker) = rest.chars().next() else {
        return false;
    };
    if marker != '*' && marker != '-' && marker != '_' {
        return false;
    }
    let mut remaining = &rest[marker.len_utf8()..];
    let mut count = 1;
    loop {
        let ws = remaining
            .bytes()
            .take_while(|byte| *byte == b' ' || *byte == b'\t')
            .count();
        remaining = &remaining[ws..];
        if remaining.is_empty() {
            break;
        }
        let ch = remaining.chars().next().unwrap();
        if ch != marker {
            return false;
        }
        count += 1;
        remaining = &remaining[ch.len_utf8()..];
    }
    count >= 3
}

fn is_valid_container_name(name: &str) -> bool {
    let mut chars = name.chars();
    if !matches!(chars.next(), Some('a'..='z')) {
        return false;
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn match_container_opener(line: &str) -> Option<(String, Option<String>)> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let bytes = rest.as_bytes();
    if bytes.len() < 3 {
        return None;
    }
    let mut colons = 0;
    while colons < bytes.len() && bytes[colons] == b':' {
        colons += 1;
    }
    if colons < 3 {
        return None;
    }
    let mut after = &rest[colons..];
    let ws = after
        .bytes()
        .take_while(|byte| *byte == b' ' || *byte == b'\t')
        .count();
    if ws == 0 {
        return None;
    }
    after = &after[ws..];
    if after.is_empty() {
        return None;
    }
    let (name, remaining) = if let Some(inner) = after.strip_prefix("{.") {
        let Some(end) = inner.find('}') else {
            return None;
        };
        let name = &inner[..end];
        if !is_valid_container_name(name) {
            return None;
        }
        (name, &inner[end + 1..])
    } else {
        let name_end = after
            .find(|ch: char| ch.is_whitespace() || ch == '|')
            .unwrap_or(after.len());
        let name = &after[..name_end];
        if !is_valid_container_name(name) {
            return None;
        }
        (name, &after[name_end..])
    };
    let remaining = remaining.trim_start();
    let argument = if remaining.is_empty() {
        None
    } else if let Some(stripped) = remaining.strip_prefix('|') {
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    } else {
        return None;
    };
    Some((name.to_string(), argument))
}

fn is_container_close(line: &str) -> bool {
    let trimmed = line.trim_matches([' ', '\t']);
    trimmed.len() >= 3 && trimmed.bytes().all(|byte| byte == b':')
}

fn strip_quote_prefixes(line: &str) -> &str {
    let mut rest = line;
    for _ in 0..16 {
        let Some(quoted) = match_quote(rest) else {
            break;
        };
        rest = quoted;
    }
    rest
}

fn strip_structural_prefixes(line: &str) -> (&str, usize) {
    let mut rest = line;
    let mut list_indent = 0;
    for _ in 0..16 {
        if let Some(quoted) = match_quote(rest) {
            rest = quoted;
            continue;
        }
        let Some(marker) = list_marker_at(rest) else {
            break;
        };
        if marker.content.is_empty() {
            break;
        }
        rest = &rest[marker.content_indent.min(rest.len())..];
        list_indent += marker.content_indent;
    }
    (rest, list_indent)
}

fn is_prefixed_fence_closer(content: &str, character: char, length: usize) -> bool {
    let indent = leading_spaces(content);
    if indent > 3 {
        return false;
    }
    let rest = content[indent..].trim_end();
    let bytes = rest.as_bytes();
    if bytes.len() < length {
        return false;
    }
    let mut run = 0;
    while run < bytes.len() && bytes[run] as char == character {
        run += 1;
    }
    if run < length {
        return false;
    }
    rest[run..].chars().all(|ch| ch == ' ' || ch == '\t')
}

fn find_prefixed_fence_close(
    lines: &[&str],
    start: usize,
    end: usize,
    character: char,
    length: usize,
    list_indent: usize,
) -> Option<usize> {
    let mut i = start + 1;
    while i < end {
        let line = strip_quote_prefixes(lines[i]);
        if list_indent > 0 && leading_spaces(line) < list_indent {
            i += 1;
            continue;
        }
        if is_prefixed_fence_closer(dedent(line, list_indent), character, length) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_container_close(lines: &[&str], start: usize, end: usize) -> Option<usize> {
    let mut nested = 0i32;
    let mut i = start + 1;
    while i < end {
        let (structural, list_indent) = strip_structural_prefixes(lines[i]);
        if let Some((character, length, _)) = match_fence_opener(structural) {
            let close = find_prefixed_fence_close(lines, i, end, character, length, list_indent);
            if close.is_none() {
                return None;
            }
            i = close.unwrap() + 1;
            continue;
        }
        if match_container_opener(structural).is_some() {
            nested += 1;
            i += 1;
            continue;
        }
        if is_container_close(structural) {
            if nested == 0 {
                return Some(i);
            }
            nested -= 1;
        }
        i += 1;
    }
    None
}

fn match_footnote_def(line: &str) -> Option<&str> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = line[indent..].strip_prefix("[^")?;
    let end = rest.find(']')?;
    if end == 0 {
        return None;
    }
    let after = rest[end + 1..].strip_prefix(':')?;
    Some(after.trim_start_matches([' ', '\t']))
}

fn starts_structural_block(line: &str) -> bool {
    match_fence_opener(line).is_some()
        || match_container_opener(line).is_some()
        || match_atx_heading(line)
        || is_hr(line)
        || match_quote(line).is_some()
        || list_marker_at(line).is_some()
}

fn interrupts_paragraph(line: &str, lines: &[&str], index: usize) -> bool {
    if match_fence_opener(line).is_some() {
        return true;
    }
    if match_container_opener(line).is_some() {
        return find_container_close(lines, index, lines.len()).is_some();
    }
    if match_atx_heading(line) {
        return true;
    }
    if is_hr(line) {
        return true;
    }
    if match_quote(line).is_some() {
        return true;
    }
    if let Some(marker) = list_marker_at(line) {
        if marker.content.is_empty() {
            return false;
        }
        if marker.ordered && marker.start != 1 {
            return false;
        }
        return true;
    }
    false
}

fn collect_paragraph(
    lines: &[&str],
    start: usize,
    end: usize,
    _labels: &mut HashSet<String>,
) -> usize {
    let mut i = start;
    while i < end {
        let line = lines[i];
        if is_blank(line) {
            break;
        }
        if i > start && interrupts_paragraph(line, lines, i) {
            break;
        }
        i += 1;
    }
    i - start
}

fn collect_quote(lines: &[&str], start: usize, end: usize, labels: &mut HashSet<String>) -> usize {
    let mut content: Vec<&str> = Vec::new();
    let mut i = start;
    let mut open_paragraph = false;
    while i < end {
        let line = lines[i];
        if let Some(quoted) = match_quote(line) {
            content.push(quoted);
            open_paragraph = quoted != "" && !starts_structural_block(quoted);
            i += 1;
            continue;
        }
        if is_blank(line) {
            break;
        }
        if open_paragraph && !starts_structural_block(line) {
            content.push(line);
            i += 1;
            continue;
        }
        break;
    }
    while content.last().is_some_and(|line| line.trim().is_empty()) {
        content.pop();
    }
    collect_block_labels(&content, 0, content.len(), labels);
    i - start
}

fn collect_list_item(
    lines: &[&str],
    start: usize,
    end: usize,
    labels: &mut HashSet<String>,
) -> usize {
    let marker = list_marker_at(lines[start]).expect("called on a list marker line");
    let content_indent = marker.content_indent;
    let mut content: Vec<&str> = Vec::new();
    let mut i = start + 1;
    let mut open_paragraph = false;
    if marker.content != "" {
        content.push(marker.content);
        open_paragraph = !starts_structural_block(marker.content);
    }
    while i < end {
        let line = lines[i];
        if is_blank(line) {
            let mut j = i + 1;
            while j < end && is_blank(lines[j]) {
                j += 1;
            }
            if j >= end {
                break;
            }
            if leading_spaces(lines[j]) >= content_indent {
                content.push("");
                open_paragraph = false;
                i += 1;
                continue;
            }
            if list_marker_at(lines[j]).is_some_and(|next| next.kind == marker.kind) {
                i = j;
                break;
            }
            break;
        }
        if leading_spaces(line) >= content_indent {
            let content_line = dedent(line, content_indent);
            content.push(content_line);
            open_paragraph = content_line != "" && !starts_structural_block(content_line);
            i += 1;
            continue;
        }
        if list_marker_at(line).is_some() {
            break;
        }
        if open_paragraph && !starts_structural_block(line) {
            content.push(line);
            i += 1;
            continue;
        }
        break;
    }
    while content.last().is_some_and(|line| line.trim().is_empty()) {
        content.pop();
    }
    collect_block_labels(&content, 0, content.len(), labels);
    i - start
}

fn collect_list(lines: &[&str], start: usize, end: usize, labels: &mut HashSet<String>) -> usize {
    let first = list_marker_at(lines[start]).expect("called on a list marker line");
    let mut i = start;
    while i < end {
        let Some(marker) = list_marker_at(lines[i]) else {
            break;
        };
        if marker.kind != first.kind {
            break;
        }
        i += collect_list_item(lines, i, end, labels);
    }
    i - start
}

fn collect_footnote(
    lines: &[&str],
    start: usize,
    end: usize,
    first: &str,
    labels: &mut HashSet<String>,
) -> usize {
    let mut content: Vec<&str> = vec![first];
    let mut j = start + 1;
    while j < end {
        let next = lines[j];
        if is_blank(next) {
            let mut k = j + 1;
            while k < end && is_blank(lines[k]) {
                k += 1;
            }
            if k < end && leading_spaces(lines[k]) >= 4 {
                content.push("");
                j += 1;
                continue;
            }
            break;
        }
        if leading_spaces(next) >= 4 {
            content.push(dedent(next, 4));
            j += 1;
            continue;
        }
        break;
    }
    while content.last().is_some_and(|line| line.trim().is_empty()) {
        content.pop();
    }
    collect_block_labels(&content, 0, content.len(), labels);
    j - start
}

/// One level of the runtime `parseBlocks` dispatch, collecting only reference
/// labels. Definitions, containers, fences, headings, rules, quotes, lists,
/// and paragraphs are consumed in the runtime's order.
fn collect_block_labels(lines: &[&str], start: usize, end: usize, labels: &mut HashSet<String>) {
    let mut i = start;
    while i < end {
        let line = lines[i];
        if is_blank(line) {
            i += 1;
            continue;
        }
        if let Some(first) = match_footnote_def(line) {
            i += collect_footnote(lines, i, end, first, labels);
            continue;
        }
        if let Some(label) = match_reference_def(line) {
            labels.insert(label);
            i += 1;
            continue;
        }
        if let Some(_) = match_container_opener(line) {
            if let Some(close) = find_container_close(lines, i, end) {
                collect_block_labels(lines, i + 1, close, labels);
                i = close + 1;
                continue;
            }
            i += collect_paragraph(lines, i, end, labels);
            continue;
        }
        if let Some((character, length, indent)) = match_fence_opener(line) {
            i += skip_fence(lines, i, end, character, length, indent);
            continue;
        }
        if match_atx_heading(line) {
            i += 1;
            continue;
        }
        if is_hr(line) {
            i += 1;
            continue;
        }
        if match_quote(line).is_some() {
            i += collect_quote(lines, i, end, labels);
            continue;
        }
        if list_marker_at(line).is_some() {
            i += collect_list(lines, i, end, labels);
            continue;
        }
        i += collect_paragraph(lines, i, end, labels);
    }
}

fn skip_fence(
    lines: &[&str],
    start: usize,
    end: usize,
    character: char,
    length: usize,
    indent: usize,
) -> usize {
    let mut i = start + 1;
    while i < end {
        if is_fence_closer(lines[i], character, length, indent) {
            return i + 1 - start;
        }
        i += 1;
    }
    end - start
}

fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b' ').count()
}

fn match_fence_opener(line: &str) -> Option<(char, usize, usize)> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let bytes = rest.as_bytes();
    if bytes.len() < 3 {
        return None;
    }
    let character = bytes[0] as char;
    if character != '`' && character != '~' {
        return None;
    }
    let mut length = 0;
    while length < bytes.len() && bytes[length] as char == character {
        length += 1;
    }
    if length < 3 {
        return None;
    }
    if character == '`' && rest[length..].contains('`') {
        return None;
    }
    Some((character, length, indent))
}

fn is_fence_closer(line: &str, character: char, length: usize, opener_indent: usize) -> bool {
    let indent = leading_spaces(line);
    if indent > 3 || indent > opener_indent {
        return false;
    }
    let rest = line[indent..].trim_end();
    let bytes = rest.as_bytes();
    if bytes.len() < length {
        return false;
    }
    let mut run = 0;
    while run < bytes.len() && bytes[run] as char == character {
        run += 1;
    }
    if run < length {
        return false;
    }
    rest[run..].chars().all(|ch| ch == ' ' || ch == '\t')
}

/// The runtime's reference-definition grammar (`REF_DEF_RE`); returns the
/// normalized label when the line is a complete definition.
fn match_reference_def(line: &str) -> Option<String> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    if !rest.starts_with('[') {
        return None;
    }
    let label_end = rest.find(']')?;
    let label = &rest[1..label_end];
    if label.is_empty() {
        return None;
    }
    let after = rest[label_end + 1..].strip_prefix(':')?;
    let after = after.trim_start_matches([' ', '\t']);
    if after.is_empty() {
        return None;
    }
    let after_destination = if let Some(inner) = after.strip_prefix('<') {
        let end = inner.find('>')?;
        if end == 0 {
            return None;
        }
        &inner[end + 1..]
    } else {
        let end = after.find([' ', '\t']).unwrap_or(after.len());
        if end == 0 {
            return None;
        }
        &after[end..]
    };
    let after_destination = after_destination.trim_start_matches([' ', '\t']);
    if !after_destination.is_empty() {
        let (trailing, title_valid) = match_title(after_destination);
        if !title_valid || !trailing.trim_matches([' ', '\t']).is_empty() {
            return None;
        }
    }
    Some(normalize_label(label))
}

/// Matches a title in double quotes, single quotes, or parentheses; returns
/// the text after it and whether it was valid.
fn match_title(rest: &str) -> (&str, bool) {
    let mut chars = rest.char_indices();
    let Some((_, opening)) = chars.next() else {
        return ("", false);
    };
    let closing = match opening {
        '"' => '"',
        '\'' => '\'',
        '(' => ')',
        _ => return (rest, false),
    };
    while let Some((index, ch)) = chars.next() {
        if ch == '\\' {
            if chars.next().is_none() {
                return ("", false);
            }
            continue;
        }
        if ch == closing {
            return (&rest[index + 1..], true);
        }
        if opening == '(' && ch == '(' {
            return ("", false);
        }
    }
    ("", false)
}

/// Register every heading in document order with its final unique id and its
/// body range, emitting W-SECT-01 for duplicate explicit ids.
pub(super) fn compute_sections(
    headings: &[HeadingEvidence<'_>],
    body: &str,
    references: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<AnalyzedSection> {
    let mut used: HashSet<String> = HashSet::new();
    let mut sections = Vec::with_capacity(headings.len());
    for heading in headings {
        let text = heading_text(heading.text, references);
        let (base, explicit) = match heading.explicit_id {
            Some(requested) => (normalize_explicit_id(requested), true),
            None => (slugify(&text), false),
        };
        if explicit && used.contains(&base) {
            diagnostics.push(Diagnostic::warning(
                "W-SECT-01",
                "duplicate explicit heading id",
            ));
        }
        let mut id = base.clone();
        let mut suffix = 2;
        while used.contains(&id) {
            id = format!("{base}-{suffix}");
            suffix += 1;
        }
        used.insert(id.clone());
        sections.push(AnalyzedSection {
            id,
            level: heading.level,
            text,
            explicit,
            body_range: section_body_range(heading, body, headings),
            offset: heading.offset,
            line: heading.line,
        });
    }
    sections
}

pub(super) fn heading_text(text: &str, references: &HashSet<String>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    project_range(&chars, 0, chars.len(), references, &mut out);
    out
}

/// From the end of the heading line to the next heading whose level is <= the
/// current level, or to the end of the body.
fn section_body_range(
    heading: &HeadingEvidence<'_>,
    body: &str,
    headings: &[HeadingEvidence<'_>],
) -> Range<usize> {
    let start = line_end(body, heading.offset);
    let end = headings
        .iter()
        .find(|other| other.offset > heading.offset && other.level <= heading.level)
        .map(|other| other.offset)
        .unwrap_or(body.len());
    start..end
}

fn line_end(body: &str, offset: usize) -> usize {
    match body[offset..].find('\n') {
        Some(position) => offset + position + 1,
        None => body.len(),
    }
}
