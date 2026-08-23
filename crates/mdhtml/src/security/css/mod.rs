//! Author CSS policy guard (ADR 0008): a fail-closed allowlist over
//! `lightningcss`. Malformed CSS is `E-MDHSEC-007`; denied at-rules are
//! `E-MDHSEC-008`; a `url()` that is not a `data:` URI is `E-MDHSEC-009`;
//! an author `@font-face` without a `data:` `src` is `E-MDHSEC-010`.
//! An out-of-position `@import`/`@namespace` surfaces as the parser's typed
//! `UnexpectedImportRule`/`UnexpectedNamespaceRule` error and stays
//! `E-MDHSEC-008` instead of collapsing into the parse-failure code; the
//! invisible `NestedDeclarations` wrapper CSS Nesting produces for bare
//! declarations is not an at-rule and is allowed, with its `url()`s still
//! checked against the `data:` allowlist. Approved CSS is re-serialized with
//! fixed `PrinterOptions` (no minification) for byte-stable derived CSS; raw
//! author text is never embedded. The legacy `@charset` declaration is
//! consumed by the parser's encoding detection before the typed AST exists;
//! it is inert on an already-decoded UTF-8 stylesheet and cannot reach the
//! output.
//!
//! Location boundary: parse failures and `@import`/`@namespace` rules carry
//! their parser-computed location (the alpha.72 `Error.loc` and
//! `ImportRule.loc`/`NamespaceRule.loc`, 1-based line/column relative to the
//! local `.theme.css`) and violations attach it. Visitor-classified
//! violations without a location field in the typed AST — unknown at-rules,
//! `url()` and `@font-face` — stay `None`.

use std::convert::Infallible;

use lightningcss::error::{ErrorLocation, ParserError};
use lightningcss::rules::CssRule;
use lightningcss::rules::font_face::{FontFaceProperty, FontFaceRule};
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::values::url::Url;
use lightningcss::visit_types;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};

use crate::security::Violation;

/// Parse, validate, and re-serialize the author-controlled local theme
/// stylesheet. The approved stylesheet is returned as deterministic
/// re-serialized bytes; the first policy violation fails the guard.
pub fn guard_author_css(css: &str) -> Result<String, Violation> {
    let mut stylesheet = StyleSheet::parse(
        css,
        ParserOptions {
            error_recovery: false,
            ..ParserOptions::default()
        },
    )
    .map_err(|error| parse_violation(error.kind, error.loc))?;

    let mut guard = CssGuard::default();
    guard
        .visit_stylesheet(&mut stylesheet)
        .expect("the css guard is infallible");
    if let Some(violation) = guard.violation {
        return Err(violation);
    }

    let output = stylesheet
        .to_css(PrinterOptions {
            minify: false,
            ..PrinterOptions::default()
        })
        .map_err(|_| Violation::new("E-MDHSEC-007", "author CSS fails to re-serialize"))?;
    Ok(output.code)
}

/// Map a lightningcss parse failure onto the frozen codes: an out-of-position
/// `@import`/`@namespace` is reported by the parser as `UnexpectedImportRule`/
/// `UnexpectedNamespaceRule` and must classify as `E-MDHSEC-008`; every other
/// failure is a genuinely malformed stylesheet and is `E-MDHSEC-007`. The
/// parse location (`loc`, 0-based line / 1-based column) is attached when
/// present, relative to the local `.theme.css`.
fn parse_violation(error: ParserError<'_>, loc: Option<ErrorLocation>) -> Violation {
    let violation = match error {
        ParserError::UnexpectedImportRule => {
            Violation::new("E-MDHSEC-008", "author CSS must not contain @import")
        }
        ParserError::UnexpectedNamespaceRule => {
            Violation::new("E-MDHSEC-008", "author CSS must not contain @namespace")
        }
        _ => Violation::new("E-MDHSEC-007", "author CSS fails to parse (fail closed)"),
    };
    match loc {
        Some(loc) => violation.at(loc.line as usize + 1, loc.column as usize),
        None => violation,
    }
}

/// The at-rule allowlist (frozen contract): `@media`, `@container`,
/// `@supports`, `@layer`, `@scope`, `@keyframes`, `@page`, `@counter-style`,
/// and `@font-face` with a `data:` `src`. Style rules are the base case.
/// Every other at-rule — `@import`, `@namespace`, and anything unknown or
/// unlisted — is `E-MDHSEC-008`; lightningcss parses unknown at-rules into
/// the typed AST rather than failing, so classification stays on the frozen
/// table instead of collapsing into a parse error.
fn rule_violation(rule: &CssRule) -> Option<Violation> {
    match rule {
        CssRule::Style(_)
        | CssRule::Media(_)
        | CssRule::Container(_)
        | CssRule::Supports(_)
        | CssRule::LayerStatement(_)
        | CssRule::LayerBlock(_)
        | CssRule::Scope(_)
        | CssRule::Keyframes(_)
        | CssRule::Page(_)
        | CssRule::CounterStyle(_)
        | CssRule::NestedDeclarations(_) => None,
        CssRule::FontFace(face) => font_face_violation(face),
        CssRule::Import(import) => Some(
            Violation::new("E-MDHSEC-008", "author CSS must not contain @import")
                .at(import.loc.line as usize + 1, import.loc.column as usize),
        ),
        CssRule::Namespace(namespace) => Some(
            Violation::new("E-MDHSEC-008", "author CSS must not contain @namespace")
                .at(namespace.loc.line as usize + 1, namespace.loc.column as usize),
        ),
        CssRule::Unknown(unknown) => Some(Violation::new(
            "E-MDHSEC-008",
            format!("author CSS must not contain the @{} at-rule", unknown.name),
        )),
        _ => Some(Violation::new(
            "E-MDHSEC-008",
            "author CSS must not contain denied at-rules",
        )),
    }
}

/// An author `@font-face` is approved only when every `src` source is a
/// `data:` URI (`E-MDHSEC-010`); `local()`, external URLs, and an absent
/// `src` descriptor are denied.
fn font_face_violation(face: &FontFaceRule) -> Option<Violation> {
    let mut has_src = false;
    for property in &face.properties {
        let FontFaceProperty::Source(sources) = property else {
            continue;
        };
        has_src = true;
        for source in sources {
            let url = match source {
                lightningcss::rules::font_face::Source::Url(source) => &source.url.url,
                lightningcss::rules::font_face::Source::Local(_) => {
                    return Some(Violation::new(
                        "E-MDHSEC-010",
                        "author @font-face src must be a data: URI",
                    ));
                }
            };
            if !is_data_url(url) {
                return Some(Violation::new(
                    "E-MDHSEC-010",
                    "author @font-face src must be a data: URI",
                ));
            }
        }
    }
    if !has_src {
        return Some(Violation::new(
            "E-MDHSEC-010",
            "author @font-face must declare a data: src",
        ));
    }
    None
}

/// The single `data:` scheme allowed for author CSS URLs (case-insensitive
/// per RFC 3986); everything else is a network-capable or external
/// reference and is denied.
fn is_data_url(url: &str) -> bool {
    url.get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

/// A visitor over the typed stylesheet AST: every rule is classified against
/// the at-rule allowlist and every `url()` — including inside custom-property
/// token streams and unparsed property values — is checked, never by string
/// matching. The first violation is recorded; traversal is otherwise
/// infallible.
#[derive(Default)]
struct CssGuard {
    violation: Option<Violation>,
}

impl<'i> Visitor<'i> for CssGuard {
    type Error = Infallible;

    fn visit_types(&self) -> VisitTypes {
        visit_types!(RULES | URLS)
    }

    fn visit_rule(&mut self, rule: &mut CssRule<'i>) -> Result<(), Self::Error> {
        if self.violation.is_none() {
            self.violation = rule_violation(rule);
        }
        rule.visit_children(self)
    }

    fn visit_url(&mut self, url: &mut Url<'i>) -> Result<(), Self::Error> {
        if self.violation.is_none() && !is_data_url(&url.url) {
            self.violation = Some(Violation::new(
                "E-MDHSEC-009",
                "author CSS url() must be a data: URI",
            ));
        }
        Ok(())
    }
}
