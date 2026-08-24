//! Fuzz the HTML/URL guard (PRD §16 "HTML validator"): `validate_url` in
//! every context, `validate_identifier` and `validate_svg` must never panic,
//! and the executable-node invariant must hold — any destination whose
//! whitespace-and-control-stripped form starts with `javascript:` must ALWAYS
//! be rejected in the Link context (mirroring the guard's own normalization,
//! `strip_ascii_whitespace_and_controls`). A miss here is a sanitizer bypass.

#![no_main]

use libfuzzer_sys::fuzz_target;
use mdhtml::security::html::{UrlContext, validate_identifier, validate_svg, validate_url};

/// The guard's normalization, mirrored: ASCII whitespace and control
/// characters removed, matched case-insensitively.
fn normalized(destination: &str) -> String {
    destination
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace() && !ch.is_ascii_control())
        .collect::<String>()
        .to_ascii_lowercase()
}

fuzz_target!(|data: &str| {
    for context in [UrlContext::Link, UrlContext::Image, UrlContext::Metadata] {
        let _ = validate_url(data, context);
    }
    let _ = validate_identifier(data);
    let _ = validate_svg(data);

    // Executable scheme invariant: after the guard's own normalization, a
    // javascript: destination can never pass the Link allowlist.
    if normalized(data).starts_with("javascript:") {
        assert!(
            validate_url(data, UrlContext::Link).is_err(),
            "javascript: destination must never pass the Link allowlist: {data:?}"
        );
        assert!(
            validate_url(data, UrlContext::Image).is_err(),
            "javascript: destination must never pass the Image allowlist: {data:?}"
        );
    }
});
