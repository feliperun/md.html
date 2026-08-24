//! Fuzz the document parsers (PRD §16 "parser"): `analyze_document`,
//! `parse_front_matter` and `scan_document` must never panic, and the
//! analysis verdict must be deterministic across identical runs. A real
//! violation becomes a bug fix plus a regression fixture (ADR 0006), never a
//! disabled target.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|source: &str| {
    let diagnostics = |analysis: &mdhtml::Analysis| {
        analysis
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>()
    };
    let analysis = mdhtml::analyze_document(source);
    assert_eq!(
        diagnostics(&analysis),
        diagnostics(&mdhtml::analyze_document(source)),
        "analysis must be deterministic for the same source"
    );
    if let Ok(parsed) = mdhtml::frontmatter::parse_front_matter(source) {
        let _ = mdhtml::scan_document(parsed.body);
    }
});
