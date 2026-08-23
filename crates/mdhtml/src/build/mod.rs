//! Document assembly (CLI-01): canonical source validation (FMT-02), derived
//! metadata (FMT-05), runtime fragment selection and embedding (§17), theme
//! and token styles, serialized as the FMT-01 skeleton.

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

/// The canonical portable CSP (SPEC §5, FMT-03). T12a emits it verbatim.
pub const CSP: &str = "default-src 'none'; script-src 'unsafe-inline'; \
style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:";

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
    assemble_document(source, source_dir, runtime_dir, themes_dir, fonts_dir, false)
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
    assemble_document(source, source_dir, runtime_dir, themes_dir, fonts_dir, true)
}

fn assemble_document(
    source: &str,
    source_dir: &Path,
    runtime_dir: &Path,
    themes_dir: &Path,
    fonts_dir: &Path,
    no_fonts: bool,
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

    let body = crate::frontmatter::parse_front_matter(source)
        .map_err(|error| BuildError::new(error.code(), error.message().to_string()))?
        .body
        .to_owned();

    guard_document(&analysis, &body)?;

    let runtime = embed_runtime(&body, &analysis, &manifest, runtime_dir)?;
    let (tokens_css, theme_css, user_css) = embed_styles(&analysis.config, source_dir, themes_dir)?;
    let fonts_css = assets::embed_fonts(&analysis, &body, &catalog, fonts_dir)?;
    let embedded = assets::embed_assets(&body, &analysis, source_dir)?;
    let image = assets::og_image(&analysis.config, source_dir)?;
    let (csp, portable) = match &analysis.config.fonts {
        Fonts::Map { url: Some(url), .. } => (assets::relaxed_csp(url), false),
        _ => (CSP.to_string(), true),
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
        &runtime,
    ))
}

/// Security guard (ADR 0006/0007): validate every author-controlled URL,
/// heading id override and section class token against the frozen policy,
/// failing the build with the first `E-MDHSEC-*` violation.
fn guard_document(
    analysis: &crate::analysis::Analysis,
    body: &str,
) -> Result<(), BuildError> {
    let evidence = scan_document(body);
    for link in &evidence.links {
        validate_url(&link.destination, UrlContext::Link).map_err(security_error)?;
    }
    for image in &evidence.images {
        validate_url(&image.destination, UrlContext::Image).map_err(security_error)?;
    }
    for heading in &evidence.headings {
        if let Some(id) = heading.explicit_id {
            validate_identifier(id).map_err(security_error)?;
        }
    }
    if let Value::Mapping(entries) = &analysis.config.sections {
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
                validate_identifier(token).map_err(security_error)?;
            }
        }
    }
    if let Some(url) = &analysis.config.url {
        validate_url(url, UrlContext::Metadata).map_err(security_error)?;
    }
    Ok(())
}

fn security_error(violation: Violation) -> BuildError {
    BuildError::new(violation.code, violation.message)
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
            let guarded = guard_author_css(&css).map_err(security_error)?;
            (!guarded.trim().is_empty()).then_some(guarded)
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
        "<html lang=\"{}\" data-mdhtml=\"1.0\" data-mdhtml-portable=\"{}\">\n",
        escape_html(lang),
        if portable { "true" } else { "false" },
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
