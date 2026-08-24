//! HTML/URL guard (ADR 0007): a validation-only, reject-don't-mutate
//! allowlist over `html5ever`. The engine parses; the policy in this module
//! decides. Every rejection uses a frozen `E-MDHSEC-*` code.

use std::cell::RefCell;

use crate::security::Violation;
use html5ever::buffer_queue::BufferQueue;
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts};

/// Where a URL appears; each context carries its own scheme allowlist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UrlContext {
    /// Markdown link/reference destinations (`href` semantics).
    Link,
    /// Markdown image destinations (`src` semantics; `data:` allowed).
    Image,
    /// Front-matter `url` (canonical / `og:url`): absolute http/https only.
    /// `cover` is a local asset path (SPEC.md FMT-05), not a URL, and is
    /// never validated in this context.
    Metadata,
}

/// Validate one URL destination against the context's scheme allowlist
/// (ADR 0007): `http`, `https`, `mailto`, `tel`, relative, and fragment-only
/// pass; `javascript`, `vbscript`, `data` in href, `file`, `blob` in href,
/// unknown schemes, and protocol-relative `//host` destinations are
/// `E-MDHSEC-012`; metadata URLs outside absolute `http`/`https` are
/// `E-MDHSEC-005`.
pub fn validate_url(destination: &str, context: UrlContext) -> Result<(), Violation> {
    let cleaned = strip_ascii_whitespace_and_controls(destination);
    let allowed = match split_scheme(&cleaned) {
        Some(scheme) => match scheme.to_ascii_lowercase().as_str() {
            "http" | "https" => true,
            "mailto" | "tel" => context != UrlContext::Metadata,
            "data" => context == UrlContext::Image,
            _ => false,
        },
        // Protocol-relative `//host/path` carries no scheme of its own but
        // resolves against the host's — a UNC path under `file://` on
        // Windows; a static document has no legitimate use for it.
        None => context != UrlContext::Metadata && !cleaned.starts_with("//"),
    };
    if allowed {
        return Ok(());
    }
    if context == UrlContext::Metadata {
        Err(Violation::new(
            "E-MDHSEC-005",
            format!("metadata URL {destination:?} must be absolute http or https"),
        ))
    } else if cleaned.starts_with("//") {
        Err(Violation::new(
            "E-MDHSEC-012",
            format!(
                "protocol-relative destination {destination:?} must declare an explicit scheme"
            ),
        ))
    } else {
        Err(Violation::new(
            "E-MDHSEC-012",
            format!("unsafe URI scheme in destination {destination:?}"),
        ))
    }
}

/// Validate one author-controlled identifier token — heading `{#id}`
/// overrides and section/class tokens — against `[A-Za-z0-9_-]+`
/// (`E-MDHSEC-004`).
pub fn validate_identifier(token: &str) -> Result<(), Violation> {
    let valid = !token.is_empty()
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    if valid {
        Ok(())
    } else {
        Err(Violation::new(
            "E-MDHSEC-004",
            format!("identifier {token:?} must match [A-Za-z0-9_-]+"),
        ))
    }
}

/// Elements that execute code or embed foreign documents inside SVG; any
/// occurrence is `E-MDHSEC-011`.
const EXECUTABLE_ELEMENTS: [&str; 4] = ["foreignobject", "iframe", "object", "embed"];

/// SMIL animation elements: `animate`/`set` rebind another element's
/// attribute at runtime (`attributeName="onload"` or `"href"` with an
/// attacker-chosen value), and a static document has no use for SMIL at all.
/// Any occurrence is `E-MDHSEC-011` — rejecting the elements outright is
/// simpler and safer than validating each `attributeName` value.
const SMIL_ELEMENTS: [&str; 4] = ["animate", "set", "animatetransform", "animatemotion"];

/// Validate embedded SVG markup structurally (ADR 0006 asset policy):
/// script or other executable content is `E-MDHSEC-011`, event handlers are
/// `E-MDHSEC-001`, external references are `E-MDHSEC-013`. Clean SVG passes.
pub fn validate_svg(markup: &str) -> Result<(), Violation> {
    let guard = SvgGuard {
        violation: RefCell::new(None),
    };
    let tokenizer = Tokenizer::new(guard, TokenizerOpts::default());
    let queue = BufferQueue::default();
    queue.push_back(StrTendril::from(markup));
    let _ = tokenizer.feed(&queue);
    tokenizer.end();
    match tokenizer.sink.violation.into_inner() {
        Some(violation) => Err(violation),
        None => Ok(()),
    }
}

/// The tokenizer sink applying the SVG policy: record the first violation.
/// `script` switches the tokenizer to raw text exactly like the tree builder,
/// so markup-shaped text inside a script payload is not a false positive.
struct SvgGuard {
    violation: RefCell<Option<Violation>>,
}

impl TokenSink for SvgGuard {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<Self::Handle> {
        if self.violation.borrow().is_some() {
            return TokenSinkResult::Continue;
        }
        let Token::TagToken(tag) = token else {
            return TokenSinkResult::Continue;
        };
        if tag.kind != TagKind::StartTag {
            return TokenSinkResult::Continue;
        }
        if (&*tag.name).eq_ignore_ascii_case("script") {
            *self.violation.borrow_mut() = Some(Violation::new(
                "E-MDHSEC-011",
                "SVG must not contain script elements",
            ));
            return TokenSinkResult::RawData(RawKind::Rawtext);
        }
        if (&*tag.name).eq_ignore_ascii_case("style") {
            return TokenSinkResult::RawData(RawKind::Rcdata);
        }
        if is_executable_element(&tag.name) {
            *self.violation.borrow_mut() = Some(Violation::new(
                "E-MDHSEC-011",
                "SVG must not contain executable content",
            ));
            return TokenSinkResult::Continue;
        }
        if is_smil_element(&tag.name) {
            *self.violation.borrow_mut() = Some(Violation::new(
                "E-MDHSEC-011",
                "SVG must not contain SMIL animation elements",
            ));
            return TokenSinkResult::Continue;
        }
        if let Some(violation) = tag
            .attrs
            .iter()
            .find_map(|attribute| attribute_violation(&attribute.name.local, &attribute.value))
        {
            *self.violation.borrow_mut() = Some(violation);
        }
        TokenSinkResult::Continue
    }
}

/// Whether `name` is one of the SVG elements that execute code or embed a
/// foreign document (`E-MDHSEC-011`).
fn is_executable_element(name: &str) -> bool {
    EXECUTABLE_ELEMENTS
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

/// Whether `name` is one of the SMIL animation elements (`E-MDHSEC-011`).
fn is_smil_element(name: &str) -> bool {
    SMIL_ELEMENTS
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

/// The violation, if any, one SVG attribute contributes: an event handler
/// (`E-MDHSEC-001`) or an external `href`/`xlink:href` reference
/// (`E-MDHSEC-013`).
fn attribute_violation(name: &str, value: &str) -> Option<Violation> {
    if name.starts_with("on") {
        return Some(Violation::new(
            "E-MDHSEC-001",
            "SVG must not contain event handler attributes",
        ));
    }
    if (name == "href" || name == "xlink:href")
        && split_scheme(&strip_ascii_whitespace_and_controls(value)).is_some()
    {
        return Some(Violation::new(
            "E-MDHSEC-013",
            "SVG must not reference external resources",
        ));
    }
    None
}

/// The destination with ASCII whitespace and control characters removed, so
/// `java\tscript:` and `jaVasCript:` cannot smuggle past scheme matching.
fn strip_ascii_whitespace_and_controls(destination: &str) -> String {
    destination
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && !ch.is_ascii_control())
        .collect()
}

/// The URI scheme of a destination (RFC 3986: `ALPHA *( ALPHA / DIGIT / "+"
/// / "-" / "." )`) when one precedes the first colon; no scheme means the
/// value is relative or fragment-only.
fn split_scheme(destination: &str) -> Option<&str> {
    let colon = destination.find(':')?;
    let candidate = &destination[..colon];
    let mut chars = candidate.chars();
    if !chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
    {
        return None;
    }
    chars
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
        .then_some(candidate)
}
