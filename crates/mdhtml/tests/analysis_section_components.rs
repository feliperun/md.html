use mdhtml::analysis::{Diagnostic, Severity, analyze_document};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/document-section-components.md")
}

fn binding_records(
    analysis: &mdhtml::analysis::Analysis,
) -> Vec<(&str, &str, Option<&str>, usize)> {
    analysis
        .bindings
        .iter()
        .map(|binding| {
            (
                binding.slug.as_str(),
                binding.component.as_str(),
                binding.class.as_deref(),
                binding.section_index,
            )
        })
        .collect()
}

fn degraded_records(analysis: &mdhtml::analysis::Analysis) -> Vec<(&str, &str)> {
    analysis
        .degraded
        .iter()
        .map(|binding| (binding.slug.as_str(), binding.component.as_str()))
        .collect()
}

fn diagnostic_records(
    analysis: &mdhtml::analysis::Analysis,
) -> Vec<(&str, Option<&str>, Option<&str>)> {
    analysis
        .diagnostics
        .iter()
        .map(|d| (d.code, d.name.as_deref(), d.target.as_deref()))
        .collect()
}

#[test]
fn fixture_section_components_matches_explicit_expectations() {
    let source = fs::read_to_string(fixture_path()).expect("read document-section-components.md");
    let analysis = analyze_document(&source);

    assert_eq!(
        binding_records(&analysis),
        [
            ("timeline", "timeline", None, 0),
            ("cards", "cards", None, 1),
            ("meters", "meters", None, 4),
            ("gallery", "gallery", None, 5),
            ("kv", "kv", None, 6),
            ("columns", "columns", None, 7),
            ("hero", "hero", None, 8),
            ("kv-table", "kv", None, 37),
        ]
    );

    assert_eq!(
        degraded_records(&analysis),
        [
            ("timeline-empty", "timeline"),
            ("cards-leading", "cards"),
            ("timeline-extra", "timeline"),
            ("meters-over", "meters"),
            ("timeline-paragraph", "timeline"),
            ("timeline-empty-item", "timeline"),
            ("cards-empty", "cards"),
            ("meters-paragraph", "meters"),
            ("meters-extra", "meters"),
            ("meters-cols", "meters"),
            ("meters-norows", "meters"),
            ("meters-negative", "meters"),
            ("meters-nonfinite", "meters"),
            ("meters-unit", "meters"),
            ("gallery-paragraph", "gallery"),
            ("gallery-empty", "gallery"),
            ("kv-extra", "kv"),
            ("kv-task", "kv"),
            ("kv-ordered", "kv"),
            ("kv-space", "kv"),
            ("kv-empty", "kv"),
            ("columns-one", "columns"),
            ("columns-empty", "columns"),
            ("hero-two-images", "hero"),
            ("hero-empty", "hero"),
            ("kv-table-cols", "kv"),
            ("meters-missing", "meters"),
        ]
    );

    assert_eq!(
        diagnostic_records(&analysis),
        [
            ("W-COMP-02", Some("timeline"), Some("timeline-empty")),
            ("E-SECT-01", None, Some("orphan-a")),
            ("W-COMP-02", Some("cards"), Some("cards-leading")),
            ("W-COMP-02", Some("cards"), Some("broken")),
            ("W-COMP-02", Some("timeline"), Some("timeline-extra")),
            ("E-SECT-01", None, Some("orphan-b")),
            ("W-COMP-02", Some("meters"), Some("meters-over")),
            ("W-COMP-02", Some("mystery"), Some("unknown")),
            ("W-COMP-02", Some("timeline"), Some("timeline-paragraph")),
            ("W-COMP-02", Some("timeline"), Some("timeline-empty-item")),
            ("W-COMP-02", Some("cards"), Some("cards-empty")),
            ("W-COMP-02", Some("meters"), Some("meters-paragraph")),
            ("W-COMP-02", Some("meters"), Some("meters-extra")),
            ("W-COMP-02", Some("meters"), Some("meters-cols")),
            ("W-COMP-02", Some("meters"), Some("meters-norows")),
            ("W-COMP-02", Some("meters"), Some("meters-negative")),
            ("W-COMP-02", Some("meters"), Some("meters-nonfinite")),
            ("W-COMP-02", Some("meters"), Some("meters-unit")),
            ("W-COMP-02", Some("gallery"), Some("gallery-paragraph")),
            ("W-COMP-02", Some("gallery"), Some("gallery-empty")),
            ("W-COMP-02", Some("kv"), Some("kv-extra")),
            ("W-COMP-02", Some("kv"), Some("kv-task")),
            ("W-COMP-02", Some("kv"), Some("kv-ordered")),
            ("W-COMP-02", Some("kv"), Some("kv-space")),
            ("W-COMP-02", Some("kv"), Some("kv-empty")),
            ("W-COMP-02", Some("columns"), Some("columns-one")),
            ("W-COMP-02", Some("columns"), Some("columns-empty")),
            ("W-COMP-02", Some("hero"), Some("hero-two-images")),
            ("W-COMP-02", Some("hero"), Some("hero-empty")),
            ("W-COMP-02", Some("kv"), Some("kv-table-cols")),
            ("W-COMP-02", Some("meters"), Some("meters-missing")),
        ]
    );

    for diagnostic in &analysis.diagnostics {
        assert_eq!(
            diagnostic.severity,
            if diagnostic.code == "E-SECT-01" {
                Severity::Error
            } else {
                Severity::Warning
            }
        );
        assert!(!diagnostic.message.contains('\n'));
        assert!(!diagnostic.message.contains('/'));
    }
}

#[test]
fn shape_warnings_carry_component_name_and_slug_with_one_line_messages() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  timeline: { component: timeline }\n  cards: { component: cards }\n  meters: { component: meters }\n  gallery: { component: gallery }\n  kv: { component: kv }\n  columns: { component: columns }\n  hero: { component: hero }\n---\n# Timeline\nText.\n\n# Cards\nText.\n\n# Meters\nText.\n\n# Gallery\nText.\n\n# Kv\nText.\n\n# Columns\nOnly.\n\n# Hero\n![One](one.png)\n\n![Two](two.png)\n",
    );

    let expected: [(&str, &str, &str); 7] = [
        (
            "timeline",
            "timeline",
            "section body must be a single nonempty list",
        ),
        (
            "cards",
            "cards",
            "section body must contain only child headings",
        ),
        (
            "meters",
            "meters",
            "section body must be a two-column table with values from 0 through 100",
        ),
        (
            "gallery",
            "gallery",
            "section body must contain only standalone image paragraphs",
        ),
        (
            "kv",
            "kv",
            "section body must be a two-column table or a strong-key list",
        ),
        (
            "columns",
            "columns",
            "section body needs at least two blocks",
        ),
        (
            "hero",
            "hero",
            "section body must be nonempty with at most one standalone image paragraph",
        ),
    ];
    assert_eq!(analysis.bindings.len(), 0);
    assert_eq!(analysis.diagnostics.len(), expected.len());
    for (index, (name, target, message)) in expected.iter().enumerate() {
        assert_eq!(
            &analysis.diagnostics[index],
            &Diagnostic {
                code: "W-COMP-02",
                severity: Severity::Warning,
                message: (*message).to_string(),
                name: Some((*name).to_string()),
                target: Some((*target).to_string()),
            },
            "diagnostic {index}"
        );
    }
    assert_eq!(
        degraded_records(&analysis),
        expected
            .iter()
            .map(|(name, target, _)| (*target, *name))
            .collect::<Vec<_>>()
    );
}

#[test]
fn timeline_accepts_ordered_lists_and_rejects_empty_items() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  ordered: { component: timeline }\n  empty-item: { component: timeline }\n---\n# Ordered\n1. One\n2. Two\n\n# Empty Item\n- One\n- \n",
    );
    assert_eq!(
        binding_records(&analysis),
        [("ordered", "timeline", None, 0)]
    );
    assert_eq!(degraded_records(&analysis), [("empty-item", "timeline")]);
    assert_eq!(
        diagnostic_records(&analysis),
        [("W-COMP-02", Some("timeline"), Some("empty-item"))]
    );
}

#[test]
fn kv_accepts_colon_only_and_continuation_blocks() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  colon: { component: kv }\n  continuation: { component: kv }\n  space: { component: kv }\n---\n# Colon\n- **Key**:\n\n# Continuation\n- **Key**: value\n  More.\n\n# Space\n- **Key** : value\n",
    );
    assert_eq!(
        binding_records(&analysis),
        [("colon", "kv", None, 0), ("continuation", "kv", None, 1),]
    );
    assert_eq!(degraded_records(&analysis), [("space", "kv")]);
    assert_eq!(
        diagnostic_records(&analysis),
        [("W-COMP-02", Some("kv"), Some("space"))]
    );
}

#[test]
fn meters_accepts_zero_and_one_hundred_boundaries() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  bounds: { component: meters }\n---\n# Bounds\n| Label | Value |\n| --- | --- |\n| Min | 0 |\n| Max | 100 |\n",
    );
    assert_eq!(binding_records(&analysis), [("bounds", "meters", None, 0)]);
    assert!(analysis.diagnostics.is_empty());
    assert!(analysis.degraded.is_empty());
}

#[test]
fn meters_rejects_tables_with_missing_source_cells() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  missing-row: { component: meters }\n  missing-header: { component: meters }\n---\n# Missing Row\n| Label | Value |\n| --- | --- |\n| CPU |\n\n# Missing Header\n| Only |\n| --- |\n| CPU | 80 |\n",
    );
    assert!(binding_records(&analysis).is_empty());
    assert_eq!(
        degraded_records(&analysis),
        [("missing-row", "meters"), ("missing-header", "meters")]
    );
    assert_eq!(
        diagnostic_records(&analysis),
        [
            ("W-COMP-02", Some("meters"), Some("missing-row")),
            ("W-COMP-02", Some("meters"), Some("missing-header")),
        ]
    );
}

#[test]
fn kv_accepts_two_column_table_and_rejects_wide_table() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  table: { component: kv }\n  wide: { component: kv }\n---\n# Table\n| Mode | Safe |\n| --- | --- |\n| Owner | Team |\n\n# Wide\n| A | B | C |\n| --- | --- | --- |\n| 1 | 2 | 3 |\n",
    );
    assert_eq!(binding_records(&analysis), [("table", "kv", None, 0)]);
    assert_eq!(degraded_records(&analysis), [("wide", "kv")]);
    assert_eq!(
        diagnostic_records(&analysis),
        [("W-COMP-02", Some("kv"), Some("wide"))]
    );
}
