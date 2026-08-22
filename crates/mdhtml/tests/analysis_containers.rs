use mdhtml::analysis::{Diagnostic, Severity, analyze_document};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/document-containers.md")
}

fn container_warnings(analysis: &mdhtml::analysis::Analysis) -> Vec<(String, String)> {
    analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "W-COMP-02")
        .map(|diagnostic| {
            (
                diagnostic.name.clone().unwrap_or_default(),
                diagnostic.message.clone(),
            )
        })
        .collect()
}

#[test]
fn fixture_document_containers_matches_explicit_warnings() {
    let source = fs::read_to_string(fixture_path()).expect("read document-containers.md");
    let analysis = analyze_document(&source);

    let expected: &[(&str, &str)] = &[
        ("note", "container argument is not allowed"),
        ("quote", "container body is empty"),
        ("details", "container body is empty"),
        ("columns", "container body needs at least two blocks"),
        ("columns", "container argument is not allowed"),
        ("stats", "container table row must have exactly two cells"),
        ("stats", "container table row must have exactly two cells"),
        ("stats", "container body must be a single two-column table"),
        (
            "bars",
            "container bar value must be a finite number not less than zero",
        ),
        ("kv", "container kv list must be unordered"),
        (
            "kv",
            "container kv item must start with a nonempty strong key",
        ),
        (
            "kv",
            "container kv item must start with a nonempty strong key",
        ),
        (
            "steps",
            "container body must be a single nonempty ordered list",
        ),
        (
            "steps",
            "container body must be a single nonempty ordered list",
        ),
        ("grid", "container body must start with a level-3 heading"),
        ("grid", "container body must start with a level-3 heading"),
        ("mystery", "unknown container name"),
        ("stats", "container body must be a single two-column table"),
        ("mystery", "unknown container name"),
    ];

    let warnings = container_warnings(&analysis);
    assert_eq!(
        warnings,
        expected
            .iter()
            .map(|(name, message)| ((*name).to_string(), (*message).to_string()))
            .collect::<Vec<_>>()
    );
    assert_eq!(analysis.diagnostics.len(), warnings.len());
    for diagnostic in &analysis.diagnostics {
        assert_eq!(diagnostic.code, "W-COMP-02");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.target, None);
    }
}

#[test]
fn container_warnings_append_after_heading_and_binding_diagnostics() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  orphan: { component: cards }\n---\n# Alpha {#same}\n\n# Alpha {#same}\n\n::: note\n:::\n",
    );
    let records: Vec<(&str, Option<&str>)> = analysis
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.name.as_deref()))
        .collect();
    assert_eq!(
        records,
        [
            ("W-SECT-01", None),
            ("E-SECT-01", None),
            ("W-COMP-02", Some("note")),
        ]
    );
}

#[test]
fn callout_argument_and_empty_body_rules() {
    let cases = [
        ("::: note\nbody\n:::\n", 0),
        ("::: note | wrong\nbody\n:::\n", 1),
        ("::: note\n:::\n", 1),
        ("::: quote\n:::\n", 1),
        ("::: details\n:::\n", 1),
        ("::: columns\nonly\n:::\n", 1),
        ("::: columns | wrong\nA\n\nB\n:::\n", 1),
    ];
    for (source, expected) in cases {
        let analysis = analyze_document(&format!("---\ntitle: T\n---\n{source}"));
        let warnings = container_warnings(&analysis);
        assert_eq!(warnings.len(), expected, "{source}");
        assert_eq!(analysis.diagnostics.len(), expected, "{source}");
        for diagnostic in &analysis.diagnostics {
            assert_eq!(diagnostic.target, None, "{source}");
        }
    }
}

#[test]
fn table_cardinality_uses_original_source_cells() {
    let cases = [
        ("::: stats\n| A | 1 |\n| --- | --- |\n:::\n", 1), // no body rows
        ("::: stats\n| A | 1 |\n| --- | --- |\n| B |\n:::\n", 1), // missing cell
        (
            "::: stats\n| A | 1 |\n| --- | --- |\n| B | 2 | 3 |\n:::\n",
            1,
        ), // extra cell
        (
            "::: stats\n| A | 1 |\n| --- | --- |\n| B | 2 |\n\nOther.\n:::\n",
            1,
        ), // extra block
        (
            "::: stats\n| A | 1 |\n| --- | --- |\n| B | **2** |\n:::\n",
            0,
        ),
    ];
    for (source, expected) in cases {
        let analysis = analyze_document(&format!("---\ntitle: T\n---\n{source}"));
        let warnings = container_warnings(&analysis);
        assert_eq!(warnings.len(), expected, "{source}");
        if expected == 1 {
            assert_eq!(warnings[0].0, "stats", "{source}");
        }
    }
}

#[test]
fn bars_accept_finite_numbers_and_reject_hostile_values() {
    let valid = ["0", "12.50", "0.0", ".5", "5.", "+3", "-0", "1e5", "1E-2"];
    for value in valid {
        let source = format!(
            "---\ntitle: T\n---\n::: bars\n| Label | Value |\n| --- | --- |\n| A | {value} |\n:::\n"
        );
        let analysis = analyze_document(&source);
        assert_eq!(container_warnings(&analysis).len(), 0, "{value}");
    }
    let hostile = [
        "", "NaN", "Infinity", "-1", "1,000", "2px", "1e400", "1.2.3", "e5", "--1",
    ];
    for value in hostile {
        let source = format!(
            "---\ntitle: T\n---\n::: bars\n| Label | Value |\n| --- | --- |\n| A | {value} |\n:::\n"
        );
        let analysis = analyze_document(&source);
        let warnings = container_warnings(&analysis);
        assert_eq!(warnings.len(), 1, "{value}");
        assert_eq!(warnings[0].0, "bars", "{value}");
    }
}

#[test]
fn kv_list_conventions() {
    let valid = [
        "- **Mode**: Safe\n- **Owner**: Team\n",
        "- **Mode**: Safe\n  - nested\n- **Owner**: Team\n",
        "- **Mode**: Safe\n  continuation\n",
    ];
    for body in valid {
        let source = format!("---\ntitle: T\n---\n::: kv\n{body}:::\n");
        let analysis = analyze_document(&source);
        assert_eq!(container_warnings(&analysis).len(), 0, "{body}");
    }
    let invalid = [
        "1. **Key**: value\n",    // ordered
        "- **Key** : value\n",    // space before colon
        "- [ ] **Key**: value\n", // task item
        "- Plain text\n",         // no strong key
        "- **Key**\n",            // missing colon
        "- - **Key**: value\n",   // first block is a nested list
        "- \n",                   // empty item
    ];
    for body in invalid {
        let source = format!("---\ntitle: T\n---\n::: kv\n{body}:::\n");
        let analysis = analyze_document(&source);
        let warnings = container_warnings(&analysis);
        assert_eq!(warnings.len(), 1, "{body}");
        assert_eq!(warnings[0].0, "kv", "{body}");
    }
}

#[test]
fn steps_require_one_ordered_list_with_nonempty_items() {
    let valid = ["1. First\n2. Second\n", "3. [ ] First\n4. Second\n"];
    for body in valid {
        let source = format!("---\ntitle: T\n---\n::: steps\n{body}:::\n");
        let analysis = analyze_document(&source);
        assert_eq!(container_warnings(&analysis).len(), 0, "{body}");
    }
    let invalid = [
        "- First\n- Second\n", // unordered
        "1. \n2. Second\n",    // empty item
        "1. First\n\nText.\n", // extra block
    ];
    for body in invalid {
        let source = format!("---\ntitle: T\n---\n::: steps\n{body}:::\n");
        let analysis = analyze_document(&source);
        let warnings = container_warnings(&analysis);
        assert_eq!(warnings.len(), 1, "{body}");
        assert_eq!(warnings[0].0, "steps", "{body}");
    }
}

#[test]
fn grid_requires_a_leading_level_three_heading() {
    let valid = ["### One\nAlpha.\n\n### Two\nBeta.\n", "### One\n"];
    for body in valid {
        let source = format!("---\ntitle: T\n---\n::: grid\n{body}:::\n");
        let analysis = analyze_document(&source);
        assert_eq!(container_warnings(&analysis).len(), 0, "{body}");
    }
    let invalid = [
        "Lead.\n\n### Card\nBody.\n", // leading paragraph
        "## Card\nBody.\n",           // wrong heading level
        "# Card\nBody.\n",            // wrong heading level
        "Text only.\n",               // no heading
    ];
    for body in invalid {
        let source = format!("---\ntitle: T\n---\n::: grid\n{body}:::\n");
        let analysis = analyze_document(&source);
        let warnings = container_warnings(&analysis);
        assert_eq!(warnings.len(), 1, "{body}");
        assert_eq!(warnings[0].0, "grid", "{body}");
    }
}

#[test]
fn unknown_names_and_nested_units_warn_in_source_order() {
    let analysis = analyze_document(
        "---\ntitle: T\n---\n::: stats\n::: mystery\nInner.\n:::\n:::\n\n::: note\nOuter.\n\n::: mystery\nInner.\n:::\n:::\n",
    );
    let warnings = container_warnings(&analysis);
    let names: Vec<&str> = warnings.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        ["stats", "mystery", "mystery"],
        "outer invalid first, then nested; valid outer keeps only the nested warning"
    );
    for (name, message) in &warnings {
        assert!(name == "stats" || name == "mystery", "{name}: {message}");
    }
}

#[test]
fn container_warnings_carry_the_container_name_and_null_target() {
    let analysis = analyze_document("---\ntitle: T\n---\n::: mystery\nx\n:::\n");
    assert_eq!(analysis.diagnostics.len(), 1);
    let diagnostic = &analysis.diagnostics[0];
    assert_eq!(
        diagnostic,
        &Diagnostic {
            code: "W-COMP-02",
            severity: Severity::Warning,
            message: "unknown container name".to_string(),
            name: Some("mystery".to_string()),
            target: None,
        }
    );
}
