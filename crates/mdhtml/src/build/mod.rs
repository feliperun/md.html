//! Document assembly (CLI-01): canonical source validation (FMT-02), derived
//! metadata (FMT-05), runtime fragment selection and embedding (§17), theme
//! and token styles, serialized as the FMT-01 skeleton.
//!
//! Safe vs unsafe mode (ADR 0009): the safe profile runs the full security
//! pipeline — `guard_document` (E-MDHSEC-004/005/012 URL and identifier
//! guards), `guard_author_css` (E-MDHSEC-007..010), `validate_svg` on asset
//! bytes (E-MDHSEC-011/001/013) and `validate_fonts_url` (E-MDHSEC-006).
//! `--unsafe` disables exactly those content-security guards while keeping
//! the format, toolchain and asset-integrity validations: the `</script`
//! terminator (E-FMT-02), the extraction-safe asset path predicate
//! (E-MDHSEC-014 — an unsafe artifact must still extract safely), all
//! analysis/front-matter validations (E-FMT-05 etc.), selection/manifest
//! errors, and the runtime-hash CSP computation (the hash pins the
//! toolchain's own runtime, not author content). Every artifact carries the
//! `data-mdhtml-safe` attestation: `"true"` for safe builds, `"false"` for
//! `--unsafe` builds.

use std::fmt;
use std::fs;
use std::path::Path;

use crate::analysis::{Fonts, NormalizedConfig, Severity, Theme, analyze_document};
use crate::cli::CliError;
use crate::frontmatter::Value;
use crate::scanner::scan_document;
use crate::security::Violation;
use crate::security::css::guard_author_css;
use crate::security::html::{UrlContext, validate_identifier, validate_url};
use crate::selection::{self, Manifest};

pub mod assets;

/// The canonical portable CSP (SPEC §5, FMT-03, ADR 0010): the frozen
/// directives with `script-src` pinned to the SHA-256 of the exact runtime
/// bytes the artifact embeds (`sha256-<BASE64>`). `style-src 'unsafe-inline'`
/// stays because the sanitizer owns style security (ADR 0008).
pub fn canonical_csp(runtime_hash: &str) -> String {
    format!(
        "default-src 'none'; script-src 'sha256-{runtime_hash}'; \
         style-src 'unsafe-inline'; img-src data: blob:; font-src data:; \
         media-src data: blob:"
    )
}

const NOSCRIPT: &str = "<noscript><style>#mdhtml-source{display:block;white-space:pre-wrap;font-family:ui-monospace;padding:2rem}</style></noscript>";

/// A build failure with a stable diagnostic code (SPEC §16). `Cli` carries
/// argument and dispatch errors unchanged; `Build` carries document
/// diagnostics with their own codes (E-FMT-02, E-FMT-05, E-CLI-01, …).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    Cli(CliError),
    Build { code: &'static str, message: String },
}

impl BuildError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        BuildError::Build {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            BuildError::Cli(_) => "E-CLI-05",
            BuildError::Build { code, .. } => code,
        }
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Cli(error) => error.fmt(formatter),
            BuildError::Build { code, message } => write!(formatter, "mdhtml: {code}: {message}"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Assemble the complete portable document from the canonical source.
///
/// `source_dir` resolves local themes relative to the source file; `runtime_dir`
/// holds the committed fragment manifest and fragment files; `themes_dir` holds
/// `base.css` and the preset theme files; `fonts_dir` holds the committed font
/// catalog and face files.
pub fn build(
    source: &str,
    source_dir: &Path,
    runtime_dir: &Path,
    themes_dir: &Path,
    fonts_dir: &Path,
) -> Result<String, BuildError> {
    assemble_document(source, source_dir, runtime_dir, themes_dir, fonts_dir, false, false)
}

/// CLI-04: build with `--no-fonts`. Font embedding is skipped and the
/// document is assembled exactly as `fonts: system` would: no font bytes and
/// no `mdhtml-fonts` block; everything else is unchanged.
pub fn build_no_fonts(
    source: &str,
    source_dir: &Path,
    runtime_dir: &Path,
    themes_dir: &Path,
    fonts_dir: &Path,
) -> Result<String, BuildError> {
    assemble_document(source, source_dir, runtime_dir, themes_dir, fonts_dir, true, false)
}

/// ADR 0009: the explicit `--unsafe` profile. Content-security guards (HTML,
/// CSS, URL, resource) are disabled; format, toolchain and asset-integrity
/// validations still run, and the artifact carries
/// `data-mdhtml-safe="false"`.
pub fn build_unsafe(
    source: &str,
    source_dir: &Path,
    runtime_dir: &Path,
    themes_dir: &Path,
    fonts_dir: &Path,
) -> Result<String, BuildError> {
    assemble_document(source, source_dir, runtime_dir, themes_dir, fonts_dir, false, true)
}

/// ADR 0009: the `--unsafe` profile combined with `--no-fonts`; font
/// embedding is skipped exactly as in `build_no_fonts`.
pub fn build_unsafe_no_fonts(
    source: &str,
    source_dir: &Path,
    runtime_dir: &Path,
    themes_dir: &Path,
    fonts_dir: &Path,
) -> Result<String, BuildError> {
    assemble_document(source, source_dir, runtime_dir, themes_dir, fonts_dir, true, true)
}

fn assemble_document(
    source: &str,
    source_dir: &Path,
    runtime_dir: &Path,
    themes_dir: &Path,
    fonts_dir: &Path,
    no_fonts: bool,
    unsafe_mode: bool,
) -> Result<String, BuildError> {
    if contains_script_terminator(source) {
        return Err(BuildError::new(
            "E-FMT-02",
            "input contains the forbidden sequence </script",
        ));
    }

    let mut analysis = analyze_document(source);
    if let Some(error) = analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return Err(BuildError::new(error.code, error.message.clone()));
    }
    if no_fonts {
        analysis.config.fonts = Fonts::System;
    }

    let manifest = selection::load(runtime_dir).map_err(selection_error)?;
    let catalog =
        selection::fonts::load(&fonts_dir.join("catalog.json")).map_err(selection_error)?;

    let parsed = crate::frontmatter::parse_front_matter(source)
        .map_err(|error| BuildError::new(error.code(), error.message().to_string()))?;
    let body = parsed.body.to_owned();
    let line_offset = source[..parsed.body_offset]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count();

    if !unsafe_mode {
        guard_document(&analysis, source, &body, line_offset)?;
    }

    let runtime = embed_runtime(&body, &analysis, &manifest, runtime_dir)?;
    let (tokens_css, theme_css, user_css) = embed_styles(
        &analysis.config,
        source_dir,
        themes_dir,
        unsafe_mode,
    )?;
    let fonts_css = assets::embed_fonts(&analysis, &body, &catalog, fonts_dir)?;
    let embedded = assets::embed_assets(
        source,
        &body,
        line_offset,
        &analysis,
        source_dir,
        unsafe_mode,
    )?;
    let image = assets::og_image(&analysis.config, source_dir)?;
    let runtime_hash = crate::selection::sha256::digest_base64(runtime.as_bytes());
    let (csp, portable) = match &analysis.config.fonts {
        Fonts::Map { url: Some(url), .. } => (assets::relaxed_csp(url, &runtime_hash), false),
        _ => (canonical_csp(&runtime_hash), true),
    };

    Ok(assemble(
        &analysis.config,
        source,
        &tokens_css,
        &theme_css,
        user_css.as_deref(),
        fonts_css.as_deref(),
        &embedded,
        image.as_deref(),
        &csp,
        portable,
        !unsafe_mode,
        &runtime,
    ))
}

/// Security guard (ADR 0006/0007): validate every author-controlled URL,
/// heading id override and section class token against the frozen policy,
/// failing the build with the first `E-MDHSEC-*` violation. Violations cite
/// canonical-source positions when the caller holds scanner evidence or can
/// locate the token; guards stay position-agnostic.
pub(crate) fn guard_document(
    analysis: &crate::analysis::Analysis,
    source: &str,
    body: &str,
    line_offset: usize,
) -> Result<(), BuildError> {
    let evidence = scan_document(body);
    for link in &evidence.links {
        guard_url(
            &link.destination,
            UrlContext::Link,
            link.line,
            link.offset,
            line_offset,
            body,
            source,
        )?;
    }
    for image in &evidence.images {
        guard_url(
            &image.destination,
            UrlContext::Image,
            image.line,
            image.offset,
            line_offset,
            body,
            source,
        )?;
    }
    for heading in &evidence.headings {
        if let Some(id) = heading.explicit_id {
            guard_identifier(id, heading.line, heading.offset, line_offset, body, source)?;
        }
    }
    guard_section_classes(&analysis.config.sections, source)?;
    if let Some(url) = &analysis.config.url {
        validate_url(url, UrlContext::Metadata).map_err(|violation| {
            security_error(violation, source, None)
        })?;
    }
    if let Fonts::Map { url: Some(url), .. } = &analysis.config.fonts {
        validate_fonts_url(url).map_err(|violation| security_error(violation, source, None))?;
    }
    Ok(())
}

/// Validate one URL destination against its context, returning the located
/// security error (canonical-source line, column inside the body) when it
/// fails the allowlist.
fn guard_url(
    destination: &str,
    context: UrlContext,
    evidence_line: usize,
    offset: usize,
    line_offset: usize,
    body: &str,
    source: &str,
) -> Result<(), BuildError> {
    match validate_url(destination, context) {
        Ok(()) => Ok(()),
        Err(violation) => Err(located_error(
            violation,
            evidence_line,
            offset,
            line_offset,
            body,
            source,
            destination,
        )),
    }
}

/// Validate one heading `{#id}` override, returning the located security
/// error when it fails the identifier contract.
fn guard_identifier(
    token: &str,
    evidence_line: usize,
    offset: usize,
    line_offset: usize,
    body: &str,
    source: &str,
) -> Result<(), BuildError> {
    match validate_identifier(token) {
        Ok(()) => Ok(()),
        Err(violation) => Err(located_error(
            violation,
            evidence_line,
            offset,
            line_offset,
            body,
            source,
            token,
        )),
    }
}

/// Validate the section class tokens, returning the located security error
/// when one fails the identifier contract.
fn guard_section_classes(sections: &Value, source: &str) -> Result<(), BuildError> {
    let Value::Mapping(entries) = sections else {
        return Ok(());
    };
    for (_, spec) in entries {
        let Value::Mapping(fields) = spec else {
            continue;
        };
        let Some(Value::String(class)) = fields
            .iter()
            .find(|(key, _)| key == "class")
            .map(|(_, value)| value)
        else {
            continue;
        };
        for token in class.split_whitespace() {
            if let Err(violation) = validate_identifier(token) {
                return Err(class_token_error(source, class, token, violation));
            }
        }
    }
    Ok(())
}

/// The located security error for an evidence-positioned violation: the
/// canonical-source line is the body-relative evidence line plus the front
/// matter line offset; the column is measured inside the body.
fn located_error(
    violation: Violation,
    evidence_line: usize,
    offset: usize,
    line_offset: usize,
    body: &str,
    source: &str,
    construct: &str,
) -> BuildError {
    security_error(
        violation.at(evidence_line + line_offset, column_of(body, offset)),
        source,
        Some(construct),
    )
}

/// The located security error for a violated section/class token, found in
/// the canonical source when the class value can be located there.
fn class_token_error(source: &str, class: &str, token: &str, violation: Violation) -> BuildError {
    let violation = match locate_class_token(source, class, token) {
        Some((line, column)) => violation.at(line, column),
        None => violation,
    };
    security_error(violation, source, Some(token))
}

/// Validate `fonts.url` (FMT-03) before it may relax any CSP directive: the
/// value must be an absolute `https:` URL whose origin is well formed and
/// free of control characters. Anything else is `E-MDHSEC-006` and fails the
/// build before the CSP is assembled.
fn validate_fonts_url(url: &str) -> Result<(), Violation> {
    if url.chars().any(|ch| ch.is_ascii_control()) {
        return Err(fonts_url_violation(url, "must not contain control characters"));
    }
    let Some(scheme) = scheme_of(url) else {
        return Err(fonts_url_violation(url, "must be an absolute https URL"));
    };
    if !scheme.eq_ignore_ascii_case("https") {
        return Err(fonts_url_violation(url, "must be an absolute https URL"));
    }
    let rest = &url[scheme.len() + 3..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    if !is_well_formed_authority(authority) {
        return Err(fonts_url_violation(url, "must carry a well-formed https origin"));
    }
    Ok(())
}

fn fonts_url_violation(url: &str, problem: &str) -> Violation {
    Violation::new("E-MDHSEC-006", format!("fonts.url {url:?} {problem}"))
}

/// The RFC 3986 scheme of a URL (`ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`
/// before the first colon), mirroring the `security::html` splitter.
fn scheme_of(url: &str) -> Option<&str> {
    let colon = url.find(':')?;
    let candidate = &url[..colon];
    let mut chars = candidate.chars();
    if !chars.next().is_some_and(|first| first.is_ascii_alphabetic()) {
        return None;
    }
    chars
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
        .then_some(candidate)
}

/// Whether `authority` is a well-formed `host[:port]`: a non-empty host of
/// RFC 3986 host characters and an optional all-digit port. Exotic forms
/// (bracketed IPv6 literals and the like) fail closed.
fn is_well_formed_authority(authority: &str) -> bool {
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let host_ok = !host.is_empty()
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '%'));
    let port_ok = port.is_none_or(|port| {
        !port.is_empty() && port.chars().all(|ch| ch.is_ascii_digit())
    });
    host_ok && port_ok
}

/// The frozen CLI-05 first line with the PRD §14 location suffix and excerpt
/// block composed into the message, so BuildError, commands.rs and main.rs
/// keep their shapes. `excerpt_source` is the whole text the cited line
/// belongs to (the canonical source for document violations, the author theme
/// CSS for CSS violations); `construct` is the offending substring within the
/// cited line that the caret run spans (`None` for point locations such as
/// CSS parse errors).
pub(crate) fn security_error(
    violation: Violation,
    excerpt_source: &str,
    construct: Option<&str>,
) -> BuildError {
    BuildError::new(violation.code, render_security_message(&violation, excerpt_source, construct))
}

/// Append the location suffix and, when the cited line and construct can be
/// located, the indented excerpt and caret line to the violation message.
fn render_security_message(
    violation: &Violation,
    excerpt_source: &str,
    construct: Option<&str>,
) -> String {
    let mut message = violation.message.clone();
    if let (Some(line), Some(column)) = (violation.line, violation.column) {
        message.push_str(&format!(" (line {line}, column {column})"));
        if let Some(excerpt) = render_excerpt(excerpt_source, line, column, construct) {
            message.push('\n');
            message.push_str(&excerpt);
        }
    }
    message
}

/// The PRD §14 excerpt block: the cited line indented by four spaces, then a
/// caret line aligned under the offending construct. The caret run spans the
/// construct substring when one is given; otherwise it marks the reported
/// column with a fixed three-caret run (CSS point locations). `None` when the
/// line or construct cannot be located — the location suffix is still emitted,
/// never a wrong caret.
fn render_excerpt(
    text: &str,
    line: usize,
    column: usize,
    construct: Option<&str>,
) -> Option<String> {
    let line_text = nth_line(text, line)?;
    let (caret_index, caret_span) = match construct {
        Some(construct) => {
            let byte_index = line_text.find(construct)?;
            (line_text[..byte_index].chars().count(), construct.chars().count())
        }
        None => {
            let caret_index = column.checked_sub(1)?;
            if caret_index > line_text.chars().count() {
                return None;
            }
            (caret_index, 3)
        }
    };
    let mut excerpt = String::with_capacity(line_text.len() + caret_index + caret_span + 6);
    excerpt.push_str("    ");
    excerpt.push_str(line_text);
    excerpt.push('\n');
    excerpt.push_str("    ");
    for _ in 0..caret_index {
        excerpt.push(' ');
    }
    for _ in 0..caret_span {
        excerpt.push('^');
    }
    Some(excerpt)
}

/// The `line`-th (1-based) line of `text` without its trailing newline or
/// carriage return; `None` when `line` is out of range.
fn nth_line(text: &str, line: usize) -> Option<&str> {
    let mut start = 0;
    let mut current = 1;
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            if current == line {
                let end = if index > start && text.as_bytes()[index - 1] == b'\r' {
                    index - 1
                } else {
                    index
                };
                return Some(&text[start..end]);
            }
            start = index + 1;
            current += 1;
        }
    }
    if current == line {
        let mut end = text.len();
        if end > start && text.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        return Some(&text[start..end]);
    }
    None
}

/// The 1-based line of `offset` within `text` (newline count plus one).
pub(crate) fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].bytes().filter(|&byte| byte == b'\n').count() + 1
}

/// The 1-based column of `offset` within its line in `text`: one plus the
/// char count from the line start to `offset`.
pub(crate) fn column_of(text: &str, offset: usize) -> usize {
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    1 + text[line_start..offset].chars().count()
}

/// The 1-based position of the first occurrence of `needle` in `text`.
pub(crate) fn locate_in_source(text: &str, needle: &str) -> Option<(usize, usize)> {
    let offset = text.find(needle)?;
    Some((line_of(text, offset), column_of(text, offset)))
}

/// Locate one violated section/class token in the canonical source. The class
/// value is a front-matter scalar with no line map, so the token is found by
/// locating the class value and measuring the token's offset inside it; a
/// value that cannot be found yields `None` (no position).
fn locate_class_token(source: &str, class: &str, token: &str) -> Option<(usize, usize)> {
    let value_offset = source.find(class)?;
    let offset = value_offset + token_offset_in_class(class, token)?;
    Some((line_of(source, offset), column_of(source, offset)))
}

/// The byte offset of `token` within a whitespace-separated class value: the
/// first occurrence among the value's whitespace-split tokens.
fn token_offset_in_class(class: &str, token: &str) -> Option<usize> {
    let mut search_from = 0;
    for split in class.split_whitespace() {
        let start = class[search_from..].find(split)? + search_from;
        if split == token {
            return Some(start);
        }
        search_from = start + split.len();
    }
    None
}

fn selection_error(error: selection::SelectionError) -> BuildError {
    let problem = error
        .problems
        .first()
        .expect("selection errors always carry problems");
    BuildError::new(problem.code, problem.message.clone())
}

fn contains_script_terminator(source: &str) -> bool {
    source.to_ascii_lowercase().contains("</script")
}

fn embed_runtime(
    body: &str,
    analysis: &crate::analysis::Analysis,
    manifest: &Manifest,
    runtime_dir: &Path,
) -> Result<String, BuildError> {
    let selected = selection::select_fragments(body, analysis, manifest);
    let mut bytes = Vec::new();
    for id in selected {
        let fragment = manifest
            .fragments
            .iter()
            .find(|fragment| fragment.id == id)
            .expect("selection only yields manifest fragments");
        let file = fs::read(runtime_dir.join(&fragment.file)).map_err(|error| {
            BuildError::new(
                "E-CLI-05",
                format!("runtime fragment {} is unreadable: {error}", fragment.file),
            )
        })?;
        bytes.extend_from_slice(&file);
    }
    String::from_utf8(bytes)
        .map_err(|_| BuildError::new("E-CLI-05", "runtime fragments are not valid UTF-8"))
}

fn embed_styles(
    config: &NormalizedConfig,
    source_dir: &Path,
    themes_dir: &Path,
    unsafe_mode: bool,
) -> Result<(String, String, Option<String>), BuildError> {
    let base = read_theme(themes_dir, "base.css")?;
    let preset = match config.theme {
        Theme::Editorial => "editorial.theme.css",
        _ => "technical.theme.css",
    };
    let preset_css = read_theme(themes_dir, preset)?;

    let user_css = match &config.theme {
        Theme::Local(name) => {
            if !is_plain_file_name(name) {
                return Err(BuildError::new(
                    "E-CLI-01",
                    format!("local theme {name} is not a plain file name"),
                ));
            }
            let path = source_dir.join(name);
            let css = fs::read_to_string(&path).map_err(|error| {
                BuildError::new(
                    "E-CLI-01",
                    format!("local theme {name} is unreadable: {error}"),
                )
            })?;
            let guarded = if unsafe_mode {
                (!css.trim().is_empty()).then_some(css)
            } else {
                let approved = guard_author_css(&css)
                    .map_err(|violation| security_error(violation, &css, None))?;
                (!approved.trim().is_empty()).then_some(approved)
            };
            guarded
        }
        _ => None,
    };

    Ok((
        render_tokens(&config.tokens),
        format!("{base}{preset_css}"),
        user_css,
    ))
}

fn read_theme(themes_dir: &Path, file: &str) -> Result<String, BuildError> {
    fs::read_to_string(themes_dir.join(file)).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("theme file {file} is unreadable: {error}"),
        )
    })
}

fn is_plain_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && Path::new(name).file_name().is_some_and(|file| file == name)
}

fn render_tokens(tokens: &Value) -> String {
    let mut css = String::new();
    if let Value::Mapping(entries) = tokens {
        for (key, value) in entries {
            let Some(rendered) = render_token_value(value) else {
                continue;
            };
            css.push_str(&format!(
                "{}: {};\n",
                escape_css(key),
                escape_css(&rendered)
            ));
        }
    }
    css
}

fn render_token_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) if number.is_finite() => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn escape_css(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            ';' => escaped.push_str("\\;"),
            '{' => escaped.push_str("\\{"),
            '}' => escaped.push_str("\\}"),
            '<' => escaped.push_str("\\3c "),
            '\n' | '\r' => escaped.push_str("\\a "),
            '\0' => escaped.push_str("\\0 "),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn assemble(
    config: &NormalizedConfig,
    source: &str,
    tokens_css: &str,
    theme_css: &str,
    user_css: Option<&str>,
    fonts_css: Option<&str>,
    assets: &[assets::Asset],
    og_image: Option<&str>,
    csp: &str,
    portable: bool,
    safe: bool,
    runtime: &str,
) -> String {
    let title = config
        .title
        .as_deref()
        .expect("title is required (E-FMT-05)");
    let lang = config.lang.as_deref().unwrap_or("en");
    let mut html = String::new();
    html.push_str("<!doctype html>\n");
    html.push_str(&format!(
        "<html lang=\"{}\" data-mdhtml=\"1.0\" data-mdhtml-portable=\"{}\" data-mdhtml-safe=\"{}\">\n",
        escape_html(lang),
        if portable { "true" } else { "false" },
        if safe { "true" } else { "false" },
    ));
    html.push_str("<head>\n");
    html.push_str("  <meta charset=\"utf-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n");
    html.push_str(&format!(
        "  <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\n"
    ));
    html.push_str(&format!("  <title>{}</title>\n", escape_html(title)));
    html.push_str(&format!(
        "  <meta property=\"og:title\" content=\"{}\">\n",
        escape_html(title)
    ));
    html.push_str("  <meta property=\"og:type\" content=\"article\">\n");
    if let Some(summary) = config.summary.as_deref() {
        html.push_str(&format!(
            "  <meta name=\"description\" content=\"{}\">\n",
            escape_html(summary)
        ));
        html.push_str(&format!(
            "  <meta property=\"og:description\" content=\"{}\">\n",
            escape_html(summary)
        ));
    }
    if let Some(url) = config.url.as_deref() {
        html.push_str(&format!(
            "  <link rel=\"canonical\" href=\"{}\">\n",
            escape_html(url)
        ));
        html.push_str(&format!(
            "  <meta property=\"og:url\" content=\"{}\">\n",
            escape_html(url)
        ));
    }
    if let Some(image) = og_image {
        html.push_str(&format!(
            "  <meta property=\"og:image\" content=\"{}\">\n",
            escape_html(image)
        ));
    }
    html.push_str(&format!(
        "  <style id=\"mdhtml-tokens\">{tokens_css}</style>\n"
    ));
    html.push_str(&format!(
        "  <style id=\"mdhtml-theme\">{theme_css}</style>\n"
    ));
    if let Some(user) = user_css {
        html.push_str(&format!("  <style id=\"mdhtml-user\">{user}</style>\n"));
    }
    if let Some(fonts) = fonts_css {
        html.push_str(&format!("  <style id=\"mdhtml-fonts\">{fonts}</style>\n"));
    }
    html.push_str("</head>\n");
    html.push_str("<body>\n");
    html.push_str("  <div id=\"mdhtml-app\"></div>\n");
    html.push_str(&format!("  {NOSCRIPT}\n"));
    html.push_str(&format!(
        "  <script id=\"mdhtml-source\" type=\"text/markdown\">{source}</script>\n"
    ));
    for asset in assets {
        html.push_str(&format!(
            "  <script type=\"application/octet-stream\" data-path=\"{}\" data-type=\"{}\">{}</script>\n",
            escape_html(&asset.path),
            escape_html(asset.mime),
            asset.payload,
        ));
    }
    html.push_str(&format!(
        "  <script id=\"mdhtml-runtime\">{runtime}</script>\n"
    ));
    html.push_str("</body>\n");
    html.push_str("</html>\n");
    html
}
