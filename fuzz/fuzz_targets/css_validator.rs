//! Fuzz the author CSS guard (PRD §16 "CSS validator"): `guard_author_css`
//! must never panic, and approval must be idempotent — the re-serialized
//! stylesheet the guard emits must re-guard to byte-identical output. A
//! non-idempotent approval would mean raw author text can reach the artifact
//! through a second pass (ADR 0008's "raw author text is never embedded").

#![no_main]

use libfuzzer_sys::fuzz_target;
use mdhtml::security::css::guard_author_css;

fuzz_target!(|css: &str| {
    if let Ok(approved) = guard_author_css(css) {
        let reapproved = guard_author_css(&approved)
            .unwrap_or_else(|violation| panic!("approved CSS must re-guard clean: {violation:?}"));
        assert_eq!(
            approved, reapproved,
            "guard_author_css must be idempotent over its own output"
        );
    }
});
