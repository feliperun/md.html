//! T13 check (CLI-02): deterministic reports for `.md` sources and built
//! `.md.html` artifacts. The report reuses the accepted analysis and build
//! structures — never re-implements parsing or re-derives diagnostics — and
//! closes with the I-CLI-02 portability verdict plus the §18 byte budgets.

use std::collections::HashSet;
use std::path::Path;

use crate::analysis::{Analysis, Diagnostic, Fonts, Severity, analyze_document};
use crate::build;
use crate::frontmatter::Value;
use crate::scanner::scan_document;
use crate::selection;

/// The §18 byte budget by category: content (canonical source), runtime
/// (selected fragments / embedded runtime), fonts (embedded face bytes),
/// images (embedded asset bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budgets {
    pub content: usize,
    pub runtime: usize,
    pub fonts: usize,
    pub images: usize,
}

/// The full deterministic check report: every diagnostic plus the portability
/// verdict, the external-request count and the byte budgets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
    pub portable: bool,
    pub requests: usize,
    pub budgets: Budgets,
}

impl CheckReport {
    /// Normative violations are errors; warnings and infos are not.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }

    /// Render the report as CLI-05 lines: one `mdhtml: <code>: <message>`
    /// per diagnostic, always closed by the I-CLI-02 verdict and budgets.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for diagnostic in &self.diagnostics {
            out.push_str(&format!(
                "mdhtml: {}: {}\n",
                diagnostic.code, diagnostic.message
            ));
        }
        out.push_str(&format!(
            "mdhtml: I-CLI-02: portable: {}; requests: {}; content: {} bytes; \
             runtime: {} bytes; fonts: {} bytes; images: {} bytes\n",
            self.portable,
            self.requests,
            self.budgets.content,
            self.budgets.runtime,
            self.budgets.fonts,
            self.budgets.images,
        ));
        out
    }
}

/// Check a canonical `.md` source: every accepted analysis diagnostic, the
/// content-derived portability verdict and the selected-fragment/face byte
/// totals from the committed manifest and catalog.
pub fn check_source(source: &str, runtime_dir: &Path, fonts_dir: &Path) -> CheckReport {
    let analysis = analyze_document(source);
    let body = crate::frontmatter::parse_front_matter(source)
        .map(|parsed| parsed.body.to_owned())
        .unwrap_or_default();

    let mut diagnostics = analysis.diagnostics.clone();
    let runtime = selected_runtime_bytes(&analysis, &body, runtime_dir, &mut diagnostics);
    let fonts = selected_font_bytes(&analysis, &body, fonts_dir, &mut diagnostics);
    let (portable, requests) = content_verdict(&analysis, &body);

    CheckReport {
        diagnostics,
        portable,
        requests,
        budgets: Budgets {
            content: source.len(),
            runtime,
            fonts,
            images: 0,
        },
    }
}

/// Check a built `.md.html` artifact structurally per FMT-01: document
/// identity, the canonical/relaxed CSP, the `data-mdhtml-portable` attribute
/// against the content verdict (E-FMT-03), missing embedded assets (W-UI-04)
/// and the budgets read from the document itself.
pub fn check_artifact(html: &str) -> CheckReport {
    let elements = scan_elements(html);
    let root = elements.iter().find(|element| element.is("html"));
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
    let csp = elements
        .iter()
        .filter(|element| element.is("meta"))
        .find(|element| {
            attr(element, "http-equiv")
                .is_some_and(|value| value.eq_ignore_ascii_case("Content-Security-Policy"))
        })
        .and_then(|element| attr(element, "content"));

    let mut diagnostics = Vec::new();

    if !root.is_some_and(|element| attr(element, "data-mdhtml") == Some("1.0")) {
        diagnostics.push(Diagnostic::error(
            "E-FMT-01",
            "document root must declare data-mdhtml=\"1.0\"",
        ));
    }
    let source_ok = source_scripts.len() == 1
        && markdown_scripts.len() == 1
        && source_scripts[0]
            .attrs
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("type") && *value == "text/markdown");
    if !source_ok {
        diagnostics.push(Diagnostic::error(
            "E-FMT-01",
            "document must contain exactly one script#mdhtml-source[type=\"text/markdown\"]",
        ));
    }

    let stored = source_scripts
        .first()
        .and_then(|element| element.text)
        .map(analyze_stored_source);
    let mut origins = stored
        .as_ref()
        .map(|stored| stored.origins.clone())
        .unwrap_or_default();
    collect_html_origins(&elements, &mut origins);
    let portable = origins.is_empty();
    let requests = origins.len();

    if let Some(stored) = &stored {
        diagnostics.extend(stored.diagnostics.iter().cloned());
    }

    let fonts_url = stored
        .as_ref()
        .and_then(|stored| stored.fonts_url.as_deref());
    match (portable, csp, fonts_url) {
        (true, Some(actual), _) if actual == build::CSP => {}
        (true, _, _) => diagnostics.push(Diagnostic::error(
            "E-FMT-03",
            "portable content must carry the canonical CSP exactly",
        )),
        (false, Some(actual), Some(url)) if actual == relaxed_csp(url) => {}
        (false, Some(_), Some(_)) => diagnostics.push(Diagnostic::error(
            "E-FMT-03",
            "the CSP must be relaxed only for the declared fonts.url origins",
        )),
        (false, Some(_), None) => {}
        (false, None, _) => diagnostics.push(Diagnostic::error(
            "E-FMT-03",
            "non-portable content must carry a Content-Security-Policy meta",
        )),
    }

    let declared = root
        .and_then(|element| attr(element, "data-mdhtml-portable"))
        .and_then(|value| match value {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        });
    match declared {
        Some(actual) if actual == portable => {}
        Some(_) => diagnostics.push(Diagnostic::error(
            "E-FMT-03",
            "data-mdhtml-portable contradicts the content verdict",
        )),
        None => diagnostics.push(Diagnostic::error(
            "E-FMT-03",
            "document must declare data-mdhtml-portable=\"true\" or \"false\"",
        )),
    }

    let asset_blocks: Vec<(&Element<'_>, &str)> = scripts
        .iter()
        .filter(|element| attr(element, "type") == Some("application/octet-stream"))
        .filter_map(|element| attr(element, "data-path").map(|path| (*element, path)))
        .collect();
    if let Some(stored) = &stored {
        let embedded: Vec<&str> = asset_blocks.iter().map(|(_, path)| *path).collect();
        for path in &stored.referenced {
            if !embedded.contains(&path.as_str()) {
                diagnostics.push(Diagnostic::warning(
                    "W-UI-04",
                    format!("asset '{path}' is referenced but not embedded"),
                ));
            }
        }
    }

    let runtime = elements
        .iter()
        .find(|element| element.is("script") && attr(element, "id") == Some("mdhtml-runtime"))
        .and_then(|element| element.text)
        .map(|text| text.len())
        .unwrap_or(0);
    let fonts = elements
        .iter()
        .find(|element| element.is("style") && attr(element, "id") == Some("mdhtml-fonts"))
        .and_then(|element| element.text)
        .map(font_bytes)
        .unwrap_or(0);
    let images = asset_blocks
        .iter()
        .map(|(element, _)| {
            element
                .text
                .map(|text| decode_base64(text).len())
                .unwrap_or(0)
        })
        .sum();

    CheckReport {
        diagnostics,
        portable,
        requests,
        budgets: Budgets {
            content: stored.as_ref().map(|stored| stored.content).unwrap_or(0),
            runtime,
            fonts,
            images,
        },
    }
}

/// Everything derived from the stored canonical source of an artifact: the
/// accepted analysis diagnostics, the declared fonts.url, the content-derived
/// external origins, the referenced asset paths and the source byte length.
struct StoredAnalysis {
    diagnostics: Vec<Diagnostic>,
    fonts_url: Option<String>,
    origins: Vec<String>,
    referenced: Vec<String>,
    content: usize,
}

fn analyze_stored_source(source: &str) -> StoredAnalysis {
    let analysis = analyze_document(source);
    let body = crate::frontmatter::parse_front_matter(source)
        .map(|parsed| parsed.body.to_owned())
        .unwrap_or_default();
    let (mut origins, fonts_url) = fonts_origins(&analysis.config.fonts);
    for image in scan_document(&body).images {
        if let Some(origin) = external_origin_of(&image.destination) {
            push_unique(&mut origins, origin);
        }
    }
    let referenced = referenced_asset_paths(&analysis, &body);
    StoredAnalysis {
        diagnostics: analysis.diagnostics,
        fonts_url,
        origins,
        referenced,
        content: source.len(),
    }
}

/// The portability verdict of canonical content: portable iff no external
/// origins are declared or referenced; the request count is the number of
/// distinct external origins.
fn content_verdict(analysis: &Analysis, body: &str) -> (bool, usize) {
    let (mut origins, _) = fonts_origins(&analysis.config.fonts);
    for image in scan_document(body).images {
        if let Some(origin) = external_origin_of(&image.destination) {
            push_unique(&mut origins, origin);
        }
    }
    let portable = origins.is_empty();
    (portable, origins.len())
}

/// The external origins a `fonts.url` declaration relaxes the CSP for,
/// mirroring `build::assets::relaxed_csp` exactly.
fn fonts_origins(fonts: &Fonts) -> (Vec<String>, Option<String>) {
    let Fonts::Map { url: Some(url), .. } = fonts else {
        return (Vec::new(), None);
    };
    let stylesheet = origin_of(url);
    let mut origins = vec![stylesheet.clone()];
    if stylesheet == "https://fonts.googleapis.com" {
        origins.push("https://fonts.gstatic.com".to_string());
    }
    (origins, Some(url.clone()))
}

fn relaxed_csp(url: &str) -> String {
    build::assets::relaxed_csp(url)
}

/// Collect external origins from artifact-level subresource tags and style
/// contents in document order.
fn collect_html_origins(elements: &[Element<'_>], origins: &mut Vec<String>) {
    for element in elements {
        match element.name.to_ascii_lowercase().as_str() {
            "script" => collect_attr_origin(element, "src", origins),
            "img" => collect_attr_origin(element, "src", origins),
            "link" => {
                let stylesheet = attr(element, "rel").is_some_and(|rel| {
                    rel.split_ascii_whitespace()
                        .any(|token| token.eq_ignore_ascii_case("stylesheet"))
                });
                if stylesheet {
                    collect_attr_origin(element, "href", origins);
                }
            }
            _ => {}
        }
        if element.is("style") {
            if let Some(text) = element.text {
                collect_css_url_origins(text, origins);
            }
        }
    }
}

fn collect_attr_origin<'a>(element: &Element<'a>, name: &str, origins: &mut Vec<String>) {
    if let Some(value) = attr(element, name) {
        if let Some(origin) = external_origin_of(value) {
            push_unique(origins, origin);
        }
    }
}

fn collect_css_url_origins(css: &str, origins: &mut Vec<String>) {
    let mut rest = css;
    while let Some(start) = rest.find("url(") {
        let after = &rest[start + 4..];
        let value = after.trim_start_matches(|ch: char| ch.is_ascii_whitespace());
        let skipped = after.len() - value.len();
        let value = value
            .strip_prefix('"')
            .or_else(|| value.strip_prefix('\''))
            .unwrap_or(value);
        let end = value.find(['"', '\'', ')']).unwrap_or(value.len());
        if let Some(origin) = external_origin_of(&value[..end]) {
            push_unique(origins, origin);
        }
        let after_value = &value[end..];
        let consumed = match after_value.find(')') {
            Some(close) => 4 + skipped + end + close + 1,
            None => 4 + skipped + end,
        };
        rest = &rest[start + consumed..];
    }
}

/// The referenced asset paths of the stored source, mirroring the accepted
/// `build::assets` collection order: scanner images, `figures:` keys, then
/// `fonts.body`/`fonts.mono`.
fn referenced_asset_paths(analysis: &Analysis, body: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for image in scan_document(body).images {
        if is_relative_path(&image.destination) && seen.insert(image.destination.clone()) {
            paths.push(image.destination.clone());
        }
    }
    if let Value::Mapping(entries) = &analysis.config.figures {
        for (key, _) in entries {
            if seen.insert(key.clone()) {
                paths.push(key.clone());
            }
        }
    }
    if let Fonts::Map { body, mono, .. } = &analysis.config.fonts {
        if let Some(path) = body {
            if seen.insert(path.clone()) {
                paths.push(path.clone());
            }
        }
        if let Some(path) = mono {
            if seen.insert(path.clone()) {
                paths.push(path.clone());
            }
        }
    }
    paths
}

fn is_relative_path(path: &str) -> bool {
    !path.is_empty() && !path.starts_with('/') && !has_scheme(path)
}

fn has_scheme(path: &str) -> bool {
    let mut chars = path.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    let mut colon = false;
    for ch in chars {
        match ch {
            ':' => {
                colon = true;
                break;
            }
            'a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '-' | '.' => {}
            _ => break,
        }
    }
    colon
}

/// scheme://host[:port] of an external http(s) URL, or `None` for inline
/// data:/blob: payloads and relative paths.
fn external_origin_of(url: &str) -> Option<String> {
    for scheme in ["https://", "http://"] {
        if let Some(after) = url.strip_prefix(scheme) {
            let end = after
                .find(|ch: char| ch == '/' || ch == '?' || ch == '#')
                .unwrap_or(after.len());
            if !after[..end].is_empty() {
                return Some(format!("{scheme}{}", &after[..end]));
            }
        }
    }
    None
}

/// scheme://host[:port] of any URL, verbatim when it has no parseable scheme.
fn origin_of(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let after = &url[scheme_end + 3..];
    let end = after
        .find(|ch: char| ch == '/' || ch == '?' || ch == '#')
        .unwrap_or(after.len());
    url[..=scheme_end + 2 + end].to_string()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn selected_runtime_bytes(
    analysis: &Analysis,
    body: &str,
    runtime_dir: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    match selection::load(runtime_dir) {
        Ok(manifest) => selection::select_fragments(body, analysis, &manifest)
            .iter()
            .map(|id| {
                manifest
                    .fragments
                    .iter()
                    .find(|fragment| fragment.id == *id)
                    .map(|fragment| fragment.size as usize)
                    .unwrap_or(0)
            })
            .sum(),
        Err(error) => {
            let problem = error
                .problems
                .first()
                .expect("selection errors always carry problems");
            diagnostics.push(Diagnostic::error(problem.code, problem.message.clone()));
            0
        }
    }
}

fn selected_font_bytes(
    analysis: &Analysis,
    body: &str,
    fonts_dir: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    match selection::fonts::load(&fonts_dir.join("catalog.json")) {
        Ok(catalog) => selection::fonts::select_faces(analysis, body, &catalog)
            .iter()
            .map(|face| face.bytes as usize)
            .sum(),
        Err(error) => {
            let problem = error
                .problems
                .first()
                .expect("selection errors always carry problems");
            diagnostics.push(Diagnostic::error(problem.code, problem.message.clone()));
            0
        }
    }
}

/// Sum of the decoded `url(data:font/woff2;base64,…)` payloads in a fonts
/// style block.
fn font_bytes(css: &str) -> usize {
    let mut total = 0;
    let mut rest = css;
    while let Some(start) = rest.find("base64,") {
        let after = &rest[start + "base64,".len()..];
        let end = after.find(')').unwrap_or(after.len());
        total += decode_base64(&after[..end]).len();
        rest = &after[end..];
    }
    total
}

/// RFC 4648 decoding that stops at the first non-alphabet character,
/// matching the artifact payloads produced by the build.
fn decode_base64(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for ch in text.chars() {
        let value = match ch {
            'A'..='Z' => ch as u32 - 'A' as u32,
            'a'..='z' => ch as u32 - 'a' as u32 + 26,
            '0'..='9' => ch as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => break,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
        }
    }
    bytes
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

/// Scan every element of the built document in source order, extracting the
/// raw text of `script` and `style` elements up to their closing tag.
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
        rest = &rest[consumed.min(rest.len())..];
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
