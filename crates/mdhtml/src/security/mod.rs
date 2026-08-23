//! Safe-by-default policy modules (ADR 0006): fail-closed, validate-and-reject
//! guards over third-party parser engines. A guard never rewrites author
//! content — a violation fails the build and the artifact is left unwritten.

pub mod css;
pub mod html;

/// One policy violation carrying its frozen diagnostic code (Tech Spec
/// addendum "Frozen security diagnostic codes") and, when cheaply available,
/// the 1-based source position of the offending construct.
///
/// Cheap-position boundary: links, images, heading `{#id}` overrides,
/// section/class tokens, asset paths/SVG references (the reference position
/// in the document, never a position inside SVG asset bytes — a different
/// frame than the document) and CSS parse failures carry positions.
/// Front-matter-sourced values (`E-MDHSEC-005` metadata `url`, `-006`
/// `fonts.url`, `cover`) have no line map in `parse_front_matter` and stay
/// `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    pub code: &'static str,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl Violation {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Violation {
            code,
            message: message.into(),
            line: None,
            column: None,
        }
    }

    /// Attach the 1-based source position of the offending construct. Guards
    /// stay position-agnostic; the caller holding scanner evidence or a parser
    /// location attaches it. Document-level violations cite the canonical
    /// source; CSS violations cite the local `.theme.css`.
    pub fn at(mut self, line: usize, column: usize) -> Self {
        self.line = Some(line);
        self.column = Some(column);
        self
    }
}

/// Extraction-safe relative asset path predicate — the single definition used
/// by BOTH build and extract (SPEC §15 CLI-03 symmetry, E-MDHSEC-014). A safe
/// path is non-empty, relative (not absolute, no URL scheme prefix — which
/// also rejects drive letters like `C:\`), has no `..` path segment anywhere,
/// no backslash, and no NUL byte. Dotfile paths are allowed: SPEC CLI-03 does
/// not prohibit them, and rejecting them would make extract refuse artifacts
/// SPEC allows.
pub fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !has_scheme(path)
        && path.split('/').all(|segment| segment != "..")
        && !path.contains('\\')
        && !path.contains('\0')
}

/// Whether `path` carries a URL scheme prefix (RFC 3986: `ALPHA *( ALPHA /
/// DIGIT / "+" / "-" / "." )` before a colon). This also catches drive-letter
/// prefixes like `C:\` and `C:/`.
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
