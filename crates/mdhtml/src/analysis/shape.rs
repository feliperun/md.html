//! Shallow, convention-only block classification over one container body.
//!
//! This is not a Markdown parser and never renders content. It answers the
//! narrow questions the SPEC container shapes need: top-level block count,
//! heading levels, list order/nonemptiness/task/kv-prefix validity,
//! standalone-image paragraphs, and the original (unpadded) table cells.
//! Balanced nested containers are supplied as authoritative body-relative
//! spans; each counts as one block and its inner source is not re-classified
//! at the parent's top level. Code and HTML comment content is masked so
//! marker-looking text there cannot create shape evidence.

use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ListItemShape {
    pub task: bool,
    pub nonempty: bool,
    pub first_is_paragraph: bool,
    pub kv_prefix_valid: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ListShape {
    pub ordered: bool,
    pub items: Vec<ListItemShape>,
}

/// Original source cells: split on unescaped pipes with optional edge pipes
/// stripped and every cell trimmed, before any padding or truncation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TableShape<'a> {
    pub header: Vec<&'a str>,
    pub rows: Vec<Vec<&'a str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ShapeBlock<'a> {
    Paragraph { image_only: bool },
    Heading(u8),
    Code,
    BlockQuote,
    List(ListShape),
    Table(TableShape<'a>),
    ThematicBreak,
    Container,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ShapeSummary<'a> {
    pub blocks: Vec<ShapeBlock<'a>>,
}

#[derive(Clone, Copy)]
struct Line<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
struct ListMarker<'a> {
    ordered: bool,
    kind: char,
    start: i64,
    content_indent: usize,
    content: &'a str,
}

#[derive(Clone, Copy)]
struct FenceOpener {
    char: u8,
    length: usize,
    indent: usize,
}

pub(super) fn classify<'a>(body: &'a str, nested: &[Range<usize>]) -> ShapeSummary<'a> {
    let lines = split_lines(body);
    let masked = comment_masked_lines(&lines, body);
    let mut nested = nested.to_vec();
    nested.sort_by_key(|range| range.start);

    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].text.trim().is_empty() || masked[i] {
            i += 1;
            continue;
        }
        if let Some(span) = nested.iter().find(|span| span.start == lines[i].start) {
            blocks.push(ShapeBlock::Container);
            let mut j = i;
            while j < lines.len() && lines[j].start <= span.end {
                j += 1;
            }
            i = j;
            continue;
        }
        if let Some(opener) = fence_opener(lines[i].text) {
            blocks.push(ShapeBlock::Code);
            i = consume_fence(&lines, i, opener);
            continue;
        }
        if is_indented_code(lines[i].text) {
            blocks.push(ShapeBlock::Code);
            let mut j = i + 1;
            while j < lines.len()
                && !masked[j]
                && !lines[j].text.trim().is_empty()
                && is_indented_code(lines[j].text)
            {
                j += 1;
            }
            i = j;
            continue;
        }
        if let Some(level) = atx_heading(lines[i].text) {
            blocks.push(ShapeBlock::Heading(level));
            i += 1;
            continue;
        }
        if is_thematic_break(lines[i].text) {
            blocks.push(ShapeBlock::ThematicBreak);
            i += 1;
            continue;
        }
        if strip_quote(lines[i].text).is_some() {
            blocks.push(ShapeBlock::BlockQuote);
            i = consume_blockquote(&lines, i);
            continue;
        }
        if list_marker_at(lines[i].text).is_some() {
            let (list, next) = consume_list(&lines, i);
            blocks.push(ShapeBlock::List(list));
            i = next;
            continue;
        }
        if is_table_start(&lines, i) {
            let (table, next) = consume_table(&lines, i);
            blocks.push(ShapeBlock::Table(table));
            i = next;
            continue;
        }
        let (image_only, next) = consume_paragraph(&lines, i, &nested);
        blocks.push(ShapeBlock::Paragraph { image_only });
        i = next;
    }
    ShapeSummary { blocks }
}

/// The SPEC bars numeric grammar: `/^[+\-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+\-]?\d+)?$/`
/// followed by a finite f64 that is not less than zero.
pub(super) fn parse_bar_value(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    i = parse_number_part(bytes, i)?;
    i = parse_exponent(bytes, i)?;
    if i != bytes.len() {
        return None;
    }
    let number: f64 = trimmed.parse().ok()?;
    if number.is_finite() && number >= 0.0 {
        Some(number)
    } else {
        None
    }
}

fn parse_number_part(bytes: &[u8], mut i: usize) -> Option<usize> {
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
        Some(i)
    } else {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
        if i < bytes.len() && bytes[i] == b'.' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        Some(i)
    }
}

fn parse_exponent(bytes: &[u8], mut i: usize) -> Option<usize> {
    if !(i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E')) {
        return Some(i);
    }
    i += 1;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == start {
        return None;
    }
    Some(i)
}

fn split_lines(source: &str) -> Vec<Line<'_>> {
    let mut lines = Vec::new();
    let mut start = 0;
    let bytes = source.as_bytes();
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' {
            let mut text_end = index;
            if text_end > start && bytes[text_end - 1] == b'\r' {
                text_end -= 1;
            }
            lines.push(Line {
                text: &source[start..text_end],
                start,
                end: text_end,
            });
            start = index + 1;
        }
    }
    if start < source.len() {
        let mut text_end = source.len();
        if text_end > start && bytes[text_end - 1] == b'\r' {
            text_end -= 1;
        }
        lines.push(Line {
            text: &source[start..text_end],
            start,
            end: text_end,
        });
    }
    lines
}

fn comment_masked_lines(lines: &[Line<'_>], source: &str) -> Vec<bool> {
    let mut masked = vec![false; lines.len()];
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<!--") {
            let end = source[i + 4..]
                .find("-->")
                .map(|position| i + 4 + position + 3)
                .unwrap_or(source.len());
            for (index, line) in lines.iter().enumerate() {
                if line.start >= i && line.end <= end {
                    masked[index] = true;
                }
            }
            i = end;
            continue;
        }
        i += 1;
    }
    masked
}

fn consume_fence(lines: &[Line<'_>], start: usize, opener: FenceOpener) -> usize {
    let mut j = start + 1;
    while j < lines.len() {
        if is_fence_closer(lines[j].text, opener) {
            return j + 1;
        }
        j += 1;
    }
    j
}

fn is_indented_code(line: &str) -> bool {
    leading_spaces(line) >= 4 && !line.trim().is_empty()
}

fn consume_blockquote(lines: &[Line<'_>], start: usize) -> usize {
    let mut j = start;
    let mut open_paragraph = false;
    loop {
        if j >= lines.len() {
            break;
        }
        let line = lines[j];
        if let Some(quoted) = strip_quote(line.text) {
            open_paragraph = !quoted.trim().is_empty() && !starts_structural_block(quoted);
            j += 1;
            continue;
        }
        if line.text.trim().is_empty() {
            break;
        }
        if open_paragraph && !starts_structural_block(line.text) {
            j += 1;
            continue;
        }
        break;
    }
    j
}

fn consume_list(lines: &[Line<'_>], start: usize) -> (ListShape, usize) {
    let first = list_marker_at(lines[start].text).expect("caller checked the marker");
    let mut items = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let Some(marker) = list_marker_at(lines[i].text) else {
            break;
        };
        if marker.kind != first.kind {
            break;
        }
        let (item, next) = consume_list_item(lines, i);
        items.push(item);
        i = next;
    }
    (
        ListShape {
            ordered: first.ordered,
            items,
        },
        i,
    )
}

fn consume_list_item(lines: &[Line<'_>], start: usize) -> (ListItemShape, usize) {
    let marker = list_marker_at(lines[start].text).expect("caller checked the marker");
    let mut content_lines: Vec<&str> = Vec::new();
    if !marker.content.is_empty() {
        content_lines.push(marker.content);
    }
    let mut i = start + 1;
    loop {
        if i >= lines.len() {
            break;
        }
        let line = lines[i];
        if line.text.trim().is_empty() {
            let mut j = i + 1;
            while j < lines.len() && lines[j].text.trim().is_empty() {
                j += 1;
            }
            if j >= lines.len() {
                break;
            }
            if leading_spaces(lines[j].text) >= marker.content_indent {
                content_lines.push("");
                i += 1;
                continue;
            }
            if let Some(next_marker) = list_marker_at(lines[j].text) {
                if next_marker.kind == marker.kind {
                    i = j;
                    break;
                }
            }
            break;
        }
        if leading_spaces(line.text) >= marker.content_indent {
            content_lines.push(dedent(line.text, marker.content_indent));
            i += 1;
            continue;
        }
        break;
    }
    (item_shape(&content_lines), i)
}

fn item_shape(content_lines: &[&str]) -> ListItemShape {
    let mut idx = 0;
    while idx < content_lines.len() && content_lines[idx].trim().is_empty() {
        idx += 1;
    }
    if idx >= content_lines.len() {
        return ListItemShape {
            task: false,
            nonempty: false,
            first_is_paragraph: false,
            kv_prefix_valid: false,
        };
    }
    let mut task = false;
    let mut first = content_lines[idx];
    if let Some(stripped) = strip_task_marker(first) {
        task = true;
        first = stripped;
        if first.trim().is_empty() {
            idx += 1;
            while idx < content_lines.len() && content_lines[idx].trim().is_empty() {
                idx += 1;
            }
            if idx >= content_lines.len() {
                return ListItemShape {
                    task: true,
                    nonempty: false,
                    first_is_paragraph: false,
                    kv_prefix_valid: false,
                };
            }
            first = content_lines[idx];
        }
    }
    let next = content_lines
        .get(idx + 1)
        .copied()
        .filter(|line| !line.trim().is_empty());
    let first_is_paragraph = !starts_structural_block(first)
        && !next.is_some_and(|next| looks_like_table_start(first, next));
    let kv_prefix_valid = first_is_paragraph && kv_prefix_valid(first);
    ListItemShape {
        task,
        nonempty: true,
        first_is_paragraph,
        kv_prefix_valid,
    }
}

/// The paragraph begins with a nonempty strong (`**` or `__`) span and the
/// character immediately after its closing run is `:`.
fn kv_prefix_valid(text: &str) -> bool {
    let (delimiter, rest) = if let Some(rest) = text.strip_prefix("**") {
        ('*', rest)
    } else if let Some(rest) = text.strip_prefix("__") {
        ('_', rest)
    } else {
        return false;
    };
    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            i += 2;
            continue;
        }
        if chars[i] != delimiter {
            i += 1;
            continue;
        }
        let mut run = 0;
        while i + run < chars.len() && chars[i + run] == delimiter {
            run += 1;
        }
        if run >= 2 {
            let close = i + run - 2;
            if close > 0 {
                if chars[..close].contains(&'`') {
                    return false;
                }
                return chars.get(close + 2) == Some(&':');
            }
            i += run;
            continue;
        }
        i += run;
    }
    false
}

fn strip_task_marker(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'[' {
        return None;
    }
    let check = bytes[1];
    if check != b' ' && check != b'x' && check != b'X' {
        return None;
    }
    if bytes[2] != b']' {
        return None;
    }
    if bytes.len() == 3 {
        return Some("");
    }
    if bytes[3] != b' ' && bytes[3] != b'\t' {
        return None;
    }
    let mut i = 3;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    Some(&line[i..])
}

fn starts_structural_block(line: &str) -> bool {
    fence_opener(line).is_some()
        || is_container_opener(line)
        || atx_heading(line).is_some()
        || is_thematic_break(line)
        || strip_quote(line).is_some()
        || list_marker_at(line).is_some()
}

fn consume_paragraph(lines: &[Line<'_>], start: usize, nested: &[Range<usize>]) -> (bool, usize) {
    let mut end = start;
    while end + 1 < lines.len() {
        let next = &lines[end + 1];
        if next.text.trim().is_empty() {
            break;
        }
        if interrupts_paragraph(lines, end + 1, nested) {
            break;
        }
        end += 1;
    }
    let mut text = String::new();
    for (index, line) in lines[start..=end].iter().enumerate() {
        if index == 0 {
            text.push_str(strip_leading(line.text));
        } else {
            text.push_str(line.text);
        }
        if index < end - start {
            text.push('\n');
        }
    }
    let image_only = is_standalone_image(&text);
    (image_only, end + 1)
}

fn interrupts_paragraph(lines: &[Line<'_>], index: usize, nested: &[Range<usize>]) -> bool {
    let line = lines[index].text;
    if fence_opener(line).is_some() {
        return true;
    }
    if is_container_opener(line) {
        return nested.iter().any(|span| span.start == lines[index].start);
    }
    if atx_heading(line).is_some() || is_thematic_break(line) || strip_quote(line).is_some() {
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
    if index + 1 < lines.len() && looks_like_table_start(line, lines[index + 1].text) {
        return true;
    }
    false
}

fn strip_leading(line: &str) -> &str {
    line.trim_start_matches(' ')
}

fn list_marker_at(line: &str) -> Option<ListMarker<'_>> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let mut chars = rest.chars();
    let first = chars.next()?;
    if first == '-' || first == '+' || first == '*' {
        let after = &rest[first.len_utf8()..];
        if !is_marker_separator(after) {
            return None;
        }
        let (content, content_indent) = marker_content(after, 1, indent);
        return Some(ListMarker {
            ordered: false,
            kind: first,
            start: 1,
            content_indent,
            content,
        });
    }
    ordered_marker(rest, indent)
}

fn is_marker_separator(after: &str) -> bool {
    match after.chars().next() {
        None => true,
        Some(ch) => ch == ' ' || ch == '\t',
    }
}

fn marker_content(after: &str, width: usize, indent: usize) -> (&str, usize) {
    let mut spaces = 0usize;
    let mut consumed = width;
    for ch in after.chars() {
        if ch == ' ' || ch == '\t' {
            spaces += 1;
            consumed += ch.len_utf8();
        } else {
            break;
        }
    }
    let content = &after[consumed - width..];
    let content_indent = if content.is_empty() {
        indent + width + 1
    } else {
        indent + width + spaces.min(4)
    };
    (content, content_indent)
}

fn ordered_marker(rest: &str, indent: usize) -> Option<ListMarker<'_>> {
    let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digit_count == 0 || digit_count > 9 {
        return None;
    }
    let after_digits = rest[digit_count..].chars().next()?;
    if after_digits != ')' && after_digits != '.' {
        return None;
    }
    let width = digit_count + 1;
    let after = &rest[width..];
    if !is_marker_separator(after) {
        return None;
    }
    let (content, content_indent) = marker_content(after, width, indent);
    Some(ListMarker {
        ordered: true,
        kind: after_digits,
        start: rest[..digit_count].parse().ok()?,
        content_indent,
        content,
    })
}

fn dedent(line: &str, max: usize) -> &str {
    let mut removed = 0usize;
    for ch in line.chars() {
        if removed < max && ch == ' ' {
            removed += 1;
        } else {
            break;
        }
    }
    &line[removed..]
}

fn atx_heading(line: &str) -> Option<u8> {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut spaces = 0;
    while spaces < 3 && i < bytes.len() && bytes[i] == b' ' {
        spaces += 1;
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'#' {
        return None;
    }
    let mut level = 0;
    while i < bytes.len() && bytes[i] == b'#' {
        level += 1;
        i += 1;
    }
    if !(1..=6).contains(&level) {
        return None;
    }
    if i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' {
        return None;
    }
    Some(level as u8)
}

fn is_thematic_break(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut spaces = 0;
    while spaces < 3 && i < bytes.len() && bytes[i] == b' ' {
        spaces += 1;
        i += 1;
    }
    if i >= bytes.len() {
        return false;
    }
    let ch = bytes[i];
    if ch != b'*' && ch != b'-' && ch != b'_' {
        return false;
    }
    let mut count = 0;
    while i < bytes.len() {
        if bytes[i] == ch {
            count += 1;
            i += 1;
        } else if bytes[i] == b' ' || bytes[i] == b'\t' {
            i += 1;
        } else {
            return false;
        }
    }
    count >= 3
}

fn strip_quote(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut spaces = 0;
    while spaces < 3 && i < bytes.len() && bytes[i] == b' ' {
        spaces += 1;
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'>' {
        return None;
    }
    i += 1;
    if i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    Some(&line[i..])
}

fn fence_opener(line: &str) -> Option<FenceOpener> {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut indent = 0;
    while indent < 3 && i < bytes.len() && bytes[i] == b' ' {
        indent += 1;
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let ch = bytes[i];
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let mut length = 0;
    while i + length < bytes.len() && bytes[i + length] == ch {
        length += 1;
    }
    if length < 3 {
        return None;
    }
    if ch == b'`' && line[i + length..].contains('`') {
        return None;
    }
    Some(FenceOpener {
        char: ch,
        length,
        indent,
    })
}

fn is_fence_closer(line: &str, opener: FenceOpener) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut indent = 0;
    while indent < 3 && i < bytes.len() && bytes[i] == b' ' {
        indent += 1;
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != opener.char || indent > opener.indent {
        return false;
    }
    let mut length = 0;
    while i + length < bytes.len() && bytes[i + length] == opener.char {
        length += 1;
    }
    if length < opener.length {
        return false;
    }
    line[i + length..].chars().all(|ch| ch == ' ' || ch == '\t')
}

/// Syntactic COMP-01 opener grammar; whether the opener is balanced is
/// decided by the caller through scanner evidence.
fn is_container_opener(line: &str) -> bool {
    let bytes = line.as_bytes();
    let i = match skip_container_indent(bytes) {
        Some(i) => i,
        None => return false,
    };
    let i = match parse_container_name(line, bytes, i) {
        Some(i) => i,
        None => return false,
    };
    let i = skip_horizontal_ws(bytes, i);
    match bytes.get(i) {
        None | Some(b'|') => true,
        Some(_) => false,
    }
}

fn skip_container_indent(bytes: &[u8]) -> Option<usize> {
    let mut i = 0;
    let mut spaces = 0;
    while spaces < 3 && i < bytes.len() && bytes[i] == b' ' {
        spaces += 1;
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b':' {
        return None;
    }
    let mut colons = 0;
    while i < bytes.len() && bytes[i] == b':' {
        colons += 1;
        i += 1;
    }
    if colons < 3 || i >= bytes.len() {
        return None;
    }
    skip_after_container_colons(bytes, i)
}

fn skip_after_container_colons(bytes: &[u8], i: usize) -> Option<usize> {
    if bytes[i] != b' ' && bytes[i] != b'\t' {
        return None;
    }
    let mut j = i;
    while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
        j += 1;
    }
    if j >= bytes.len() {
        return None;
    }
    Some(j)
}

fn parse_container_name(line: &str, bytes: &[u8], i: usize) -> Option<usize> {
    if bytes[i] == b'{' {
        if i + 2 >= bytes.len() || bytes[i + 1] != b'.' {
            return None;
        }
        let name_start = i + 2;
        let Some(relative_close) = line[name_start..].find('}') else {
            return None;
        };
        let name = &line[name_start..name_start + relative_close];
        if !is_valid_name(name) {
            return None;
        }
        Some(name_start + relative_close + 1)
    } else {
        let name_start = i;
        let mut i = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_lowercase() || bytes[i].is_ascii_digit() || bytes[i] == b'-')
        {
            i += 1;
        }
        if !is_valid_name(&line[name_start..i]) {
            return None;
        }
        Some(i)
    }
}

fn skip_horizontal_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    i
}

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn has_unescaped_pipe(line: &str) -> bool {
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '|' {
            return true;
        }
    }
    false
}

fn looks_like_table_start(line: &str, next: &str) -> bool {
    has_unescaped_pipe(line) && is_delimiter_row(next)
}

fn is_table_start(lines: &[Line<'_>], index: usize) -> bool {
    let Some(next) = lines.get(index + 1) else {
        return false;
    };
    if !looks_like_table_start(lines[index].text, next.text) {
        return false;
    }
    split_table_row(lines[index].text).len() == split_table_row(next.text).len()
}

fn is_delimiter_row(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut spaces = 0;
    while spaces < 3 && i < bytes.len() && bytes[i] == b' ' {
        spaces += 1;
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'|' {
        i += 1;
    }
    if !consume_delim_cell(bytes, &mut i) {
        return false;
    }
    loop {
        if i < bytes.len() && bytes[i] == b'|' {
            i += 1;
            if !consume_delim_cell(bytes, &mut i) {
                return i == bytes.len();
            }
            continue;
        }
        break;
    }
    i == bytes.len()
}

fn consume_delim_cell(bytes: &[u8], i: &mut usize) -> bool {
    while *i < bytes.len() && (bytes[*i] == b' ' || bytes[*i] == b'\t') {
        *i += 1;
    }
    if *i < bytes.len() && bytes[*i] == b':' {
        *i += 1;
    }
    let dash_start = *i;
    while *i < bytes.len() && bytes[*i] == b'-' {
        *i += 1;
    }
    if *i == dash_start {
        return false;
    }
    if *i < bytes.len() && bytes[*i] == b':' {
        *i += 1;
    }
    while *i < bytes.len() && (bytes[*i] == b' ' || bytes[*i] == b'\t') {
        *i += 1;
    }
    true
}

fn split_table_row(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut cells = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut escaped = false;
    while i < bytes.len() {
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if bytes[i] == b'\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if bytes[i] == b'|' {
            cells.push(&line[start..i]);
            start = i + 1;
            i += 1;
            continue;
        }
        i += 1;
    }
    cells.push(&line[start..]);
    if cells.len() > 1 && cells[0].is_empty() {
        cells.remove(0);
    }
    if cells.len() > 1 && cells[cells.len() - 1].is_empty() {
        cells.pop();
    }
    cells.iter().map(|cell| cell.trim()).collect()
}

fn consume_table<'s, 'a>(lines: &'s [Line<'a>], start: usize) -> (TableShape<'a>, usize) {
    let header = split_table_row(lines[start].text);
    let mut rows = Vec::new();
    let mut i = start + 2;
    while i < lines.len() {
        let line = lines[i];
        if line.text.trim().is_empty() || starts_structural_block(line.text) {
            break;
        }
        rows.push(split_table_row(line.text));
        i += 1;
    }
    (TableShape { header, rows }, i)
}

fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|&byte| byte == b' ').count()
}

/// A paragraph whose whole inline content is exactly one Markdown image in
/// the inline `![alt](dest "title")` form. Reference forms are treated
/// conservatively until a caller needs them.
fn is_standalone_image(text: &str) -> bool {
    let text = text.trim();
    let Some(rest) = text.strip_prefix("![") else {
        return false;
    };
    let Some(close) = find_bracket_close(rest) else {
        return false;
    };
    let after = &rest[close + 1..];
    if let Some(inner) = after.strip_prefix('(') {
        let Some(paren_close) = find_parenthesis_close(inner) else {
            return false;
        };
        if !parse_destination(&inner[..paren_close]) {
            return false;
        }
        inner[paren_close + 1..].trim().is_empty()
    } else {
        false
    }
}

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

fn find_bracket_close(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && is_ascii_punctuation(bytes[i + 1]) {
            i += 2;
            continue;
        }
        if bytes[i] == b'[' {
            depth += 1;
        } else if bytes[i] == b']' {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
        i += 1;
    }
    None
}

fn find_parenthesis_close(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && is_ascii_punctuation(bytes[i + 1]) {
            i += 2;
            continue;
        }
        if bytes[i] == b'(' {
            depth += 1;
        } else if bytes[i] == b')' {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
        i += 1;
    }
    None
}

fn is_ascii_punctuation(byte: u8) -> bool {
    b"!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".contains(&byte)
}
