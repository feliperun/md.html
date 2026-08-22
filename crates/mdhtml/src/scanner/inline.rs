use super::{ImageEvidence, ImageKind};
use std::collections::HashMap;

const ASCII_PUNCTUATION: &[u8] = b"!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

fn is_ascii_punctuation(b: u8) -> bool {
    ASCII_PUNCTUATION.contains(&b)
}

pub(super) fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut backslashes = 0;
    let mut i = index;
    while i > 0 && bytes[i - 1] == b'\\' {
        backslashes += 1;
        i -= 1;
    }
    backslashes % 2 == 1
}

pub(super) fn find_bracket_close(bytes: &[u8], start: usize, limit: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = start;
    while i < limit {
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

pub(super) fn find_parenthesis_close(bytes: &[u8], start: usize, limit: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = start;
    while i < limit {
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

pub(super) fn decode_escapes(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                if next.is_ascii_punctuation() {
                    decoded.push(next);
                } else {
                    decoded.push('\\');
                    decoded.push(next);
                }
            } else {
                decoded.push('\\');
            }
        } else {
            decoded.push(ch);
        }
    }
    decoded
}

pub(super) fn normalize_ref_label(label: &str) -> String {
    let mut result = String::new();
    let mut in_whitespace = false;
    for ch in label.trim().chars() {
        if ch.is_whitespace() {
            if !in_whitespace {
                result.push(' ');
                in_whitespace = true;
            }
        } else {
            for lower in ch.to_lowercase() {
                result.push(lower);
            }
            in_whitespace = false;
        }
    }
    result
}

pub(super) struct InlineScanner<'a, 'm, 'r> {
    source: &'a str,
    bytes: &'a [u8],
    mask: &'m [bool],
    references: &'r HashMap<String, &'a str>,
}

impl<'a, 'm, 'r> InlineScanner<'a, 'm, 'r> {
    pub(super) fn new(
        source: &'a str,
        mask: &'m [bool],
        references: &'r HashMap<String, &'a str>,
    ) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            mask,
            references,
        }
    }

    pub(super) fn scan_range(
        &self,
        range: std::ops::Range<usize>,
        images: &mut Vec<ImageEvidence>,
        has_emphasis: &mut bool,
        line_offset_to_num: impl Fn(usize) -> usize,
        line_text_end: impl Fn(usize) -> usize,
    ) {
        let mut i = range.start;
        while i < range.end {
            if self.mask[i] {
                i += 1;
                continue;
            }
            let line_limit = line_text_end(i);

            // Check escaped punctuation
            if self.bytes[i] == b'\\'
                && i + 1 < range.end
                && is_ascii_punctuation(self.bytes[i + 1])
            {
                i += 2;
                continue;
            }

            // Check HTML <img> tag
            if self.bytes[i] == b'<' {
                if let Some((dest, consumed)) = self.match_html_img(i, line_limit) {
                    let line = line_offset_to_num(i);
                    images.push(ImageEvidence {
                        kind: ImageKind::Html,
                        destination: dest,
                        offset: i,
                        line,
                    });
                    i += consumed;
                    continue;
                }
            }

            // Check Markdown image ![alt](dest) or ![alt][ref]
            if self.bytes[i] == b'!' && i + 1 < range.end && self.bytes[i + 1] == b'[' {
                if let Some((dest, consumed)) = self.match_markdown_image(i, line_limit) {
                    let line = line_offset_to_num(i);
                    images.push(ImageEvidence {
                        kind: ImageKind::Markdown,
                        destination: dest,
                        offset: i,
                        line,
                    });
                    i += consumed;
                    continue;
                }
            }

            // Check emphasis
            if !*has_emphasis && (self.bytes[i] == b'*' || self.bytes[i] == b'_') {
                if let Some(consumed) = self.match_emphasis(i, line_limit) {
                    *has_emphasis = true;
                    i += consumed;
                    continue;
                }
            }

            i += 1;
        }
    }

    fn match_html_img(&self, start: usize, limit: usize) -> Option<(String, usize)> {
        // Must start with <img or <IMG followed by whitespace or / or >
        let slice = &self.bytes[start..limit];
        if slice.len() < 4 {
            return None;
        }
        if slice[0] != b'<'
            || (slice[1] != b'i' && slice[1] != b'I')
            || (slice[2] != b'm' && slice[2] != b'M')
            || (slice[3] != b'g' && slice[3] != b'G')
        {
            return None;
        }
        if slice.len() > 4 && !matches!(slice[4], b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>') {
            return None;
        }

        let close_pos = self.find_tag_close(start, limit)?;
        let src = self.find_src_attribute(start + 4, close_pos);
        src.map(|value| (value, (close_pos + 1) - start))
    }

    fn find_tag_close(&self, start: usize, limit: usize) -> Option<usize> {
        let mut in_quote: Option<u8> = None;
        let mut idx = start + 4;
        while idx < limit {
            if self.mask[idx] {
                idx += 1;
                continue;
            }
            let b = self.bytes[idx];
            if let Some(q) = in_quote {
                if b == q {
                    in_quote = None;
                }
            } else if b == b'"' || b == b'\'' {
                in_quote = Some(b);
            } else if b == b'>' {
                return Some(idx);
            }
            idx += 1;
        }
        None
    }

    fn find_src_attribute(&self, mut p: usize, close_pos: usize) -> Option<String> {
        while p < close_pos {
            // skip whitespace
            while p < close_pos && matches!(self.bytes[p], b' ' | b'\t' | b'\n' | b'\r' | b'/') {
                p += 1;
            }
            if p >= close_pos {
                break;
            }
            // read attr name
            let attr_start = p;
            while p < close_pos
                && !matches!(
                    self.bytes[p],
                    b' ' | b'\t' | b'\n' | b'\r' | b'=' | b'/' | b'>'
                )
            {
                p += 1;
            }
            let attr_name = &self.source[attr_start..p];
            let is_src = attr_name.eq_ignore_ascii_case("src");

            // skip whitespace
            while p < close_pos && matches!(self.bytes[p], b' ' | b'\t' | b'\n' | b'\r') {
                p += 1;
            }

            if p < close_pos && self.bytes[p] == b'=' {
                p += 1; // skip '='
                while p < close_pos && matches!(self.bytes[p], b' ' | b'\t' | b'\n' | b'\r') {
                    p += 1;
                }
                if p < close_pos {
                    let value = self.parse_attr_value(&mut p, close_pos);
                    if is_src {
                        return Some(value);
                    }
                }
            }
        }

        None
    }

    fn parse_attr_value(&self, p: &mut usize, close_pos: usize) -> String {
        let b = self.bytes[*p];
        if b == b'"' || b == b'\'' {
            let quote = b;
            *p += 1;
            let val_start = *p;
            while *p < close_pos && self.bytes[*p] != quote {
                *p += 1;
            }
            let val = &self.source[val_start..*p];
            if *p < close_pos && self.bytes[*p] == quote {
                *p += 1;
            }
            val.to_string()
        } else {
            // unquoted
            let val_start = *p;
            while *p < close_pos
                && !matches!(self.bytes[*p], b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')
            {
                *p += 1;
            }
            let val = &self.source[val_start..*p];
            val.to_string()
        }
    }

    fn match_markdown_image(&self, start: usize, limit: usize) -> Option<(String, usize)> {
        // start is at '!'
        let label_start = start + 2;
        let label_end = find_bracket_close(self.bytes, label_start, limit)?;
        if label_end >= limit {
            return None;
        }

        let after_label = label_end + 1;
        if after_label < limit && self.bytes[after_label] == b'(' {
            // Inline destination
            let paren_end = find_parenthesis_close(self.bytes, after_label + 1, limit)?;
            if paren_end >= limit {
                return None;
            }
            let raw_dest = self.source[after_label + 1..paren_end].trim();
            // Parse angle bracket <url> or regular url
            let dest = if raw_dest.starts_with('<') {
                if let Some(angle_close) = raw_dest.find('>') {
                    decode_escapes(&raw_dest[1..angle_close])
                } else {
                    decode_escapes(raw_dest)
                }
            } else {
                // Split by whitespace to ignore optional title
                decode_escapes(raw_dest.split_whitespace().next().unwrap_or(""))
            };
            return Some((dest, (paren_end + 1) - start));
        }

        if after_label < limit && self.bytes[after_label] == b'[' {
            // Reference image
            let ref_end = find_bracket_close(self.bytes, after_label + 1, limit)?;
            if ref_end >= limit {
                return None;
            }
            let ref_label = &self.source[after_label + 1..ref_end];
            let label_to_use = if ref_label.trim().is_empty() {
                &self.source[label_start..label_end]
            } else {
                ref_label
            };
            let key = normalize_ref_label(label_to_use);
            if let Some(dest) = self.references.get(&key) {
                return Some(((**dest).to_string(), (ref_end + 1) - start));
            }
            return None;
        }

        // Shortcut reference: ![label]
        let key = normalize_ref_label(&self.source[label_start..label_end]);
        if let Some(dest) = self.references.get(&key) {
            return Some(((**dest).to_string(), (label_end + 1) - start));
        }

        None
    }

    fn match_emphasis(&self, start: usize, limit: usize) -> Option<usize> {
        let delim = self.bytes[start];
        // Must be single delimiter
        if !self.is_single_delimiter(start, limit, delim) {
            return None;
        }

        // Left flanking check for opener:
        if !self.is_emphasis_opener(start, limit, delim) {
            return None;
        }

        // Find closing delimiter
        let close = self.find_emphasis_close(start + 1, limit, delim)?;
        Some((close + 1) - start)
    }

    fn is_single_delimiter(&self, start: usize, limit: usize, delim: u8) -> bool {
        self.delimiter_run(start, limit, delim) == 1
    }

    fn delimiter_run(&self, start: usize, limit: usize, delim: u8) -> usize {
        let mut count = 0;
        let mut p = start;
        while p < limit && self.bytes[p] == delim {
            count += 1;
            p += 1;
        }
        count
    }

    fn is_emphasis_opener(&self, start: usize, limit: usize, delim: u8) -> bool {
        let prev_char = if start > 0 {
            self.source[..start].chars().next_back()
        } else {
            None
        };
        let next_char = self.source[start + 1..limit].chars().next();

        let Some(next_c) = next_char else {
            return false;
        };
        if next_c.is_whitespace() {
            return false;
        }

        if delim == b'_' {
            if let Some(prev_c) = prev_char {
                if prev_c.is_alphanumeric() && next_c.is_alphanumeric() {
                    return false; // intraword snake_case
                }
            }
        }
        true
    }

    fn find_emphasis_close(&self, mut i: usize, limit: usize, delim: u8) -> Option<usize> {
        while i < limit {
            if self.mask[i] {
                i += 1;
                continue;
            }
            if self.bytes[i] == b'\\' && i + 1 < limit && is_ascii_punctuation(self.bytes[i + 1]) {
                i += 2;
                continue;
            }
            if self.bytes[i] == delim {
                // Check if closing delimiter run length is 1
                if self.is_single_delimiter(i, limit, delim) {
                    // Right flanking check:
                    if !self.is_emphasis_closer(i, limit, delim) {
                        i += 1;
                        continue;
                    }
                    // Valid single emphasis match!
                    return Some(i);
                } else {
                    i += self.delimiter_run(i, limit, delim);
                    continue;
                }
            }
            i += 1;
        }

        None
    }

    fn is_emphasis_closer(&self, i: usize, limit: usize, delim: u8) -> bool {
        let prev_c = self.source[..i].chars().next_back().unwrap();
        let next_c = self.source[i + 1..limit].chars().next();
        if prev_c.is_whitespace() {
            return false;
        }
        if delim == b'_' {
            if let Some(nc) = next_c {
                if prev_c.is_alphanumeric() && nc.is_alphanumeric() {
                    return false;
                }
            }
        }
        true
    }
}
