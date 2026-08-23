//! Safe-by-default policy modules (ADR 0006): fail-closed, validate-and-reject
//! guards over third-party parser engines. A guard never rewrites author
//! content — a violation fails the build and the artifact is left unwritten.

pub mod css;
pub mod html;

/// One policy violation carrying its frozen diagnostic code (Tech Spec
/// addendum "Frozen security diagnostic codes") and, when cheaply available,
/// the 1-based source position of the offending construct.
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
}

/// Extraction-safe relative asset path predicate shared by build and extract
/// (SPEC §15 CLI-03 symmetry). A safe path is non-empty, relative, has no
/// `..` segment, no backslash, and no drive-letter prefix.
pub fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.starts_with('.')
        && !path.contains("/../")
        && !path.ends_with("/..")
        && !path.contains('\0')
}
