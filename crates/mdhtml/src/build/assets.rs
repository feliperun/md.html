//! Asset embedding (CLI-01): the closed extension → MIME table (SPEC §14),
//! RFC 4648 base64 payloads of the exact file bytes, embedded `@font-face`
//! faces (SPEC §18), og:image derivation (FMT-05) and the non-portable CSP
//! relaxation (FMT-03).

use std::fs;
use std::path::Path;

use crate::analysis::{Analysis, Fonts, NormalizedConfig};
use crate::build::BuildError;
use crate::frontmatter::Value;
use crate::scanner::scan_document;
use crate::selection::{Catalog, select_faces};

/// Closed extension → MIME mapping (SPEC §14). Anything else is E-CLI-01.
pub const MIME_BY_EXTENSION: [(&str, &str); 8] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
    ("woff2", "font/woff2"),
    ("css", "text/css"),
];

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// One embedded asset block: the original relative path, the closed MIME type
/// and the standard padded base64 payload (RFC 4648).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub path: String,
    pub mime: &'static str,
    pub payload: String,
}

/// Collect and embed every asset the document references (SPEC §14): scanner
/// image evidence in document order, then `figures:` mapping keys, then
/// `fonts.body`/`fonts.mono` when fonts is a map. Distinct `data-path` values
/// embed exactly once, in first-reference order. Missing files and
/// out-of-table extensions fail with E-CLI-01 before any output.
pub fn embed_assets(
    body: &str,
    analysis: &Analysis,
    source_dir: &Path,
) -> Result<Vec<Asset>, BuildError> {
    let mut assets = Vec::new();
    for path in collect_asset_paths(body, analysis) {
        let mime = mime_for_path(&path)?;
        let bytes = fs::read(source_dir.join(&path)).map_err(|error| {
            BuildError::new(
                "E-CLI-01",
                format!("asset '{path}' is unresolvable: {error}"),
            )
        })?;
        assets.push(Asset {
            path,
            mime,
            payload: base64_encode(&bytes),
        });
    }
    Ok(assets)
}

fn collect_asset_paths(body: &str, analysis: &Analysis) -> Vec<String> {
    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let evidence = scan_document(body);
    for image in &evidence.images {
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

/// Scanner image destinations may be data:/https:/… references; only normal
/// relative paths are embeddable assets (SPEC §14).
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

fn mime_for_path(path: &str) -> Result<&'static str, BuildError> {
    let extension = path
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    match MIME_BY_EXTENSION
        .iter()
        .find(|(candidate, _)| *candidate == extension.as_str())
    {
        Some((_, mime)) => Ok(*mime),
        None => Err(BuildError::new(
            "E-CLI-01",
            format!("asset '{path}' has an extension outside the closed MIME table"),
        )),
    }
}

/// Select and embed the built-in faces the document needs (SPEC §18): one
/// `@font-face` per selected face in selection order, with `font-family` from
/// the catalog family name, `font-style` from the face style, `font-weight`
/// from the catalog weight range and the exact face file bytes as a base64
/// `font/woff2` data URI. `None` when no face is selected.
pub fn embed_fonts(
    analysis: &Analysis,
    body: &str,
    catalog: &Catalog,
    fonts_dir: &Path,
) -> Result<Option<String>, BuildError> {
    let faces = select_faces(analysis, body, catalog);
    if faces.is_empty() {
        return Ok(None);
    }
    let mut css = String::new();
    for face in faces {
        let family = catalog
            .families
            .iter()
            .find(|family| family.key == face.family)
            .expect("selected faces always resolve to a catalog family");
        let bytes = fs::read(fonts_dir.join(&face.file)).map_err(|error| {
            BuildError::new(
                "E-CLI-01",
                format!("font face '{}' is unresolvable: {error}", face.file),
            )
        })?;
        css.push_str(&format!(
            "@font-face {{\n  font-family: \"{}\";\n  font-style: {};\n  font-weight: {} {};\n  src: url(data:font/woff2;base64,{});\n}}\n",
            escape_css_string(&family.name),
            face.style,
            face.axes.min,
            face.axes.max,
            base64_encode(&bytes),
        ));
    }
    Ok(Some(css))
}

/// Derive `og:image` (FMT-05): present only when both `url` and `cover` are
/// declared and the cover resolves relative to the source file. The value is
/// the absolute URL formed from `url` and the cover path; the cover itself is
/// not embedded.
pub fn og_image(
    config: &NormalizedConfig,
    source_dir: &Path,
) -> Result<Option<String>, BuildError> {
    let (Some(url), Some(cover)) = (&config.url, &config.cover) else {
        return Ok(None);
    };
    fs::metadata(source_dir.join(cover)).map_err(|error| {
        BuildError::new(
            "E-CLI-01",
            format!("cover '{cover}' is unresolvable: {error}"),
        )
    })?;
    Ok(Some(absolute_url(url, cover)))
}

/// The non-portable CSP (FMT-03): the canonical directives with `style-src`
/// and `font-src` relaxed only for the declared `fonts.url` origins, following
/// the SPEC example (Google Fonts stylesheet/font origins). Scripts remain
/// inline-only; no other network capability is enabled.
pub fn relaxed_csp(url: &str) -> String {
    let stylesheet_origin = origin_of(url);
    let font_origin = if stylesheet_origin == "https://fonts.googleapis.com" {
        "https://fonts.gstatic.com".to_string()
    } else {
        stylesheet_origin.clone()
    };
    format!(
        "default-src 'none'; script-src 'unsafe-inline'; \
         style-src 'unsafe-inline' {stylesheet_origin}; img-src data: blob:; \
         font-src data: {font_origin}; media-src data: blob:"
    )
}

fn absolute_url(url: &str, cover: &str) -> String {
    match url.rfind('/') {
        Some(index) => format!("{}{cover}", &url[..=index]),
        None => format!("{url}/{cover}"),
    }
}

/// scheme://host[:port] of a URL, verbatim when it has no parseable scheme.
fn origin_of(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let after = &url[scheme_end + 3..];
    let end = after
        .find(|ch| ch == '/' || ch == '?' || ch == '#')
        .unwrap_or(after.len());
    url[..=scheme_end + 2 + end].to_string()
}

fn escape_css_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn base64_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(((bytes.len() + 2) / 3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let triple = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        encoded.push(BASE64_ALPHABET[(triple >> 18) as usize & 63] as char);
        encoded.push(BASE64_ALPHABET[(triple >> 12) as usize & 63] as char);
        encoded.push(if chunk.len() > 1 {
            BASE64_ALPHABET[(triple >> 6) as usize & 63] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            BASE64_ALPHABET[triple as usize & 63] as char
        } else {
            '='
        });
    }
    encoded
}
