use super::inline::{InlineScanner, find_bracket_close, is_escaped, normalize_ref_label};
use super::{ContainerEvidence, HeadingEvidence, ScanEvidence};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub(super) struct Line<'a> {
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
    pub line_num: usize,
}

pub(super) fn split_lines(source: &str) -> Vec<Line<'_>> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    let mut line_num = 1;
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
                end: index + 1,
                line_num,
            });
            start = index + 1;
            line_num += 1;
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
            end: source.len(),
            line_num,
        });
    }
    lines
}

fn leading_spaces(text: &str) -> usize {
    let mut n = 0;
    for b in text.bytes() {
        if b == b' ' {
            n += 1;
        } else {
            break;
        }
    }
    n
}

#[derive(Clone, Copy, Debug)]
struct FenceOpener {
    char: u8,
    length: usize,
}

fn match_fence_opener(line: &str) -> Option<FenceOpener> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let bytes = rest.as_bytes();
    if bytes.len() < 3 {
        return None;
    }
    let ch = bytes[0];
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let mut len = 0;
    while len < bytes.len() && bytes[len] == ch {
        len += 1;
    }
    if len < 3 {
        return None;
    }
    // For backticks, info string must not contain backticks
    if ch == b'`' {
        let info = &rest[len..];
        if info.contains('`') {
            return None;
        }
    }
    Some(FenceOpener {
        char: ch,
        length: len,
    })
}

fn is_fence_closer(line: &str, opener: FenceOpener) -> bool {
    let indent = leading_spaces(line);
    if indent > 3 {
        return false;
    }
    let rest = line[indent..].trim_end();
    let bytes = rest.as_bytes();
    if bytes.len() < opener.length {
        return false;
    }
    let mut len = 0;
    while len < bytes.len() && bytes[len] == opener.char {
        len += 1;
    }
    if len < opener.length {
        return false;
    }
    // Must contain only fence chars and optional trailing spaces/tabs
    rest[len..].chars().all(|c| c == ' ' || c == '\t')
}

#[derive(Clone, Debug)]
struct ContainerOpener<'a> {
    name: &'a str,
    argument: Option<&'a str>,
    line_index: usize,
}

fn match_container_opener<'a>(line: &'a str, line_index: usize) -> Option<ContainerOpener<'a>> {
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
    if after.is_empty() {
        return None;
    }
    // Must have whitespace after colons
    let ws_len = after
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .count();
    if ws_len == 0 {
        return None;
    }
    after = &after[ws_len..];
    if after.is_empty() {
        return None;
    }

    // Name is [a-z][a-z0-9-]* or {.name}
    let (name, remaining) = if after.starts_with("{.") {
        let end_brace = after.find('}')?;
        let name_cand = &after[2..end_brace];
        if !is_valid_container_name(name_cand) {
            return None;
        }
        (name_cand, &after[end_brace + 1..])
    } else {
        let name_end = after
            .find(|c: char| c.is_whitespace() || c == '|')
            .unwrap_or(after.len());
        let name_cand = &after[..name_end];
        if !is_valid_container_name(name_cand) {
            return None;
        }
        (name_cand, &after[name_end..])
    };

    // After the name, only optional whitespace and an optional `| argument`
    // are allowed; anything else keeps the line as ordinary prose.
    let remaining = remaining.trim_start();
    let argument = if remaining.is_empty() {
        None
    } else if let Some(stripped) = remaining.strip_prefix('|') {
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    } else {
        return None;
    };

    Some(ContainerOpener {
        name,
        argument,
        line_index,
    })
}

fn is_valid_container_name(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    if !bytes[0].is_ascii_lowercase() {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn is_container_closer(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    trimmed.bytes().all(|b| b == b':')
}

fn strip_closing_hashes(text: &str) -> &str {
    // Remove optional closing # decoration
    // A closing sequence of # must be preceded by space/tab
    if let Some(last_hash) = text.rfind('#') {
        if last_hash == text.len() - 1 {
            let mut h_start = last_hash;
            while h_start > 0 && text.as_bytes()[h_start - 1] == b'#' {
                h_start -= 1;
            }
            if h_start == 0
                || text.as_bytes()[h_start - 1] == b' '
                || text.as_bytes()[h_start - 1] == b'\t'
            {
                return text[..h_start].trim_end();
            }
        }
    }
    text
}

fn parse_explicit_id(text: &str) -> (Option<&str>, &str) {
    // Check optional final space-separated {#id}
    if let Some(brace_open) = text.rfind("{#") {
        if text.ends_with('}') {
            let candidate_id = &text[brace_open + 2..text.len() - 1];
            // Must be space-separated
            if brace_open > 0
                && (text.as_bytes()[brace_open - 1] == b' '
                    || text.as_bytes()[brace_open - 1] == b'\t')
            {
                return (Some(candidate_id), text[..brace_open].trim_end());
            }
        }
    }
    (None, text)
}

fn match_atx_heading(line: &str) -> Option<(u8, &str, Option<&str>)> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let bytes = rest.as_bytes();
    if bytes.is_empty() || bytes[0] != b'#' {
        return None;
    }
    let mut level = 0;
    while level < bytes.len() && bytes[level] == b'#' {
        level += 1;
    }
    if level < 1 || level > 6 {
        return None;
    }
    let after_hashes = &rest[level..];
    if !after_hashes.is_empty() && !after_hashes.starts_with(' ') && !after_hashes.starts_with('\t')
    {
        return None;
    }

    let heading_text = strip_closing_hashes(after_hashes.trim());
    let (explicit_id, heading_text) = parse_explicit_id(heading_text);

    Some((level as u8, heading_text, explicit_id))
}

fn match_reference_def(line: &str) -> Option<(&str, &str)> {
    let indent = leading_spaces(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    if !rest.starts_with('[') {
        return None;
    }
    let bytes = rest.as_bytes();
    let label_end = find_bracket_close(bytes, 1, bytes.len())?;
    let label = &rest[1..label_end];
    let after_label = &rest[label_end + 1..];
    if !after_label.starts_with(':') {
        return None;
    }
    let after_colon = after_label[1..].trim_start();
    if after_colon.is_empty() {
        return None;
    }

    let dest = if after_colon.starts_with('<') {
        if let Some(angle_close) = after_colon.find('>') {
            &after_colon[1..angle_close]
        } else {
            after_colon
        }
    } else {
        after_colon.split_whitespace().next().unwrap_or("")
    };

    Some((label, dest))
}

pub(super) fn scan_lines(source: &str) -> ScanEvidence<'_> {
    if source.is_empty() {
        return ScanEvidence::default();
    }
    let lines = split_lines(source);
    let mut mask = vec![false; source.len()];
    let mut references = HashMap::new();

    // Line offsets for binary search
    let line_starts: Vec<usize> = lines.iter().map(|l| l.start).collect();
    let line_offset_to_num = |offset: usize| -> usize {
        match line_starts.binary_search(&offset) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        }
    };
    let line_text_end = |offset: usize| -> usize {
        match line_starts.binary_search(&offset) {
            Ok(index) => lines[index].start + lines[index].text.len(),
            Err(index) => {
                let index = index.saturating_sub(1).min(lines.len() - 1);
                lines[index].start + lines[index].text.len()
            }
        }
    };

    // Phase 1: Mask complete HTML comments
    mask_html_comments(source, &mut mask);

    // Phase 2: Mask fenced code blocks and indented code blocks
    let mut has_code = mask_code_blocks(&lines, &mut mask);

    // Phase 3: Mask inline code spans and collect reference definitions
    has_code |= mask_inline_code(&lines, source.as_bytes(), &mut mask, &mut references);

    // Phase 4: Headings and Containers
    let (headings, mut containers) = scan_headings_and_containers(&lines, &mask);
    containers.sort_by_key(|c| c.offset);

    // Phase 5: Inline scanning for images, links and single emphasis
    let mut images = Vec::new();
    let mut links = Vec::new();
    let mut has_emphasis = false;

    let inline_scanner = InlineScanner::new(source, &mask, &references);
    inline_scanner.scan_range(
        0..source.len(),
        &mut images,
        &mut links,
        &mut has_emphasis,
        line_offset_to_num,
        line_text_end,
    );

    ScanEvidence {
        headings,
        images,
        links,
        containers,
        has_emphasis,
        has_code,
    }
}

fn mask_html_comments(source: &str, mask: &mut [bool]) {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<!--") {
            let comment_start = i;
            if let Some(comment_end) = source[i + 4..].find("-->") {
                let end_pos = i + 4 + comment_end + 3;
                for idx in comment_start..end_pos {
                    mask[idx] = true;
                }
                i = end_pos;
                continue;
            }
        }
        i += 1;
    }
}

fn mask_code_blocks(lines: &[Line<'_>], mask: &mut [bool]) -> bool {
    let mut has_code = false;
    let mut line_idx = 0;
    while line_idx < lines.len() {
        let line = lines[line_idx];
        if mask[line.start] {
            line_idx += 1;
            continue;
        }

        if let Some(fence) = match_fence_opener(line.text) {
            has_code = true;
            let fence_start = line.start;
            let mut fence_end = line.end;
            let mut j = line_idx + 1;
            while j < lines.len() {
                let current_line = lines[j];
                fence_end = current_line.end;
                if is_fence_closer(current_line.text, fence) {
                    j += 1;
                    break;
                }
                j += 1;
            }
            for idx in fence_start..fence_end {
                mask[idx] = true;
            }
            line_idx = j;
            continue;
        }

        // Check indented code: nonblank line indented by >= 4 spaces
        let indent = leading_spaces(line.text);
        if indent >= 4 && !line.text.trim().is_empty() {
            has_code = true;
            for idx in line.start..line.end {
                mask[idx] = true;
            }
        }

        line_idx += 1;
    }
    has_code
}

fn mask_code_span(line: &Line<'_>, bytes: &[u8], mask: &mut [bool], idx: usize, run: usize) -> Option<usize> {
    // Find closing run of equal length
    let mut c_idx = idx + run;
    while c_idx < line.end {
        if mask[c_idx] {
            c_idx += 1;
            continue;
        }
        if bytes[c_idx] == b'`' {
            let mut c_run = 0;
            while c_idx + c_run < line.end && bytes[c_idx + c_run] == b'`' {
                c_run += 1;
            }
            if c_run == run {
                let span_end = c_idx + run;
                for k in idx..span_end {
                    mask[k] = true;
                }
                return Some(span_end);
            }
            c_idx += c_run;
        } else {
            c_idx += 1;
        }
    }
    None
}

fn mask_inline_code<'a>(
    lines: &[Line<'a>],
    bytes: &[u8],
    mask: &mut [bool],
    references: &mut HashMap<String, &'a str>,
) -> bool {
    let mut has_code = false;
    for line in lines {
        if mask[line.start] && mask[line.end.saturating_sub(1)] {
            // Already completely masked
            continue;
        }

        // Collect reference definitions if not masked
        if !mask[line.start] {
            if let Some((label, dest)) = match_reference_def(line.text) {
                let key = normalize_ref_label(label);
                references.entry(key).or_insert(dest);
            }
        }

        // Inline code spans in this line
        let mut idx = line.start;
        while idx < line.end {
            if mask[idx] {
                idx += 1;
                continue;
            }
            if bytes[idx] == b'`' && !is_escaped(bytes, idx) {
                let mut run = 0;
                while idx + run < line.end && bytes[idx + run] == b'`' {
                    run += 1;
                }
                if let Some(span_end) = mask_code_span(line, bytes, mask, idx, run) {
                    has_code = true;
                    idx = span_end;
                } else {
                    idx += run;
                }
                continue;
            }
            idx += 1;
        }
    }
    has_code
}

fn scan_headings_and_containers<'a>(
    lines: &[Line<'a>],
    mask: &[bool],
) -> (Vec<HeadingEvidence<'a>>, Vec<ContainerEvidence<'a>>) {
    let mut headings = Vec::new();
    let mut containers = Vec::new();
    let mut container_stack: Vec<ContainerOpener<'_>> = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if mask[line.start] {
            continue;
        }

        // Check container closer
        if is_container_closer(line.text) {
            if let Some(opener) = container_stack.pop() {
                let opener_line = lines[opener.line_index];
                let body_start = opener_line.end;
                let body_end = line.start;
                containers.push(ContainerEvidence {
                    name: opener.name,
                    argument: opener.argument,
                    offset: opener_line.start,
                    line: opener_line.line_num,
                    body_range: body_start..body_end,
                });
                continue;
            }
        }

        // Check container opener
        if let Some(opener) = match_container_opener(line.text, idx) {
            container_stack.push(opener);
            continue;
        }

        // Check ATX heading
        if let Some((level, text, explicit_id)) = match_atx_heading(line.text) {
            headings.push(HeadingEvidence {
                level,
                text,
                explicit_id,
                offset: line.start,
                line: line.line_num,
            });
        }
    }

    (headings, containers)
}
