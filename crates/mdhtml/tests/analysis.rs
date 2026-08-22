use mdhtml::analysis::{
    AnalyzedSection, DegradedBinding, Diagnostic, PendingBinding, Severity, analyze_document,
    slugify,
};
use mdhtml::frontmatter::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/document-analysis.md")
}

fn mapping_keys(value: &Value) -> Vec<String> {
    match value {
        Value::Mapping(entries) => entries.iter().map(|(key, _)| key.clone()).collect(),
        other => panic!("expected mapping, got {other:?}"),
    }
}

#[test]
fn fixture_analysis_matches_explicit_expectations() {
    let source = fs::read_to_string(fixture_path()).expect("read document-analysis.md");
    let analysis = analyze_document(&source);

    let config = &analysis.config;
    assert_eq!(config.title.as_deref(), Some("Analysis fixture"));
    assert_eq!(config.summary, None);
    assert_eq!(config.lang.as_deref(), Some("en"));
    assert_eq!(config.theme, mdhtml::analysis::Theme::Technical);
    assert_eq!(config.url, None);
    assert_eq!(config.cover, None);
    assert_eq!(config.fonts, mdhtml::analysis::Fonts::Auto);
    assert_eq!(
        config.toc,
        mdhtml::analysis::TocSetting::Enabled(mdhtml::analysis::Toc {
            depth: 2,
            position: mdhtml::analysis::TocPosition::Inline
        })
    );
    assert_eq!(config.tokens, Value::Mapping(Vec::new()));
    assert_eq!(config.figures, Value::Mapping(Vec::new()));
    assert_eq!(
        mapping_keys(&config.sections),
        [
            "results",
            "quarterly-results-2",
            "orphan",
            "broken",
            "unknown"
        ]
    );

    let expected_sections = [
        (
            "results",
            1u8,
            "Crème brûlée / 2",
            true,
            33..263,
            0usize,
            1usize,
        ),
        (
            "quarterly-results",
            2,
            "Quarterly Results",
            false,
            73..152,
            50,
            5,
        ),
        (
            "quarterly-results-2",
            2,
            "Quarterly Results",
            false,
            173..191,
            152,
            9,
        ),
        (
            "quarterly-results-3",
            2,
            "Quarterly Results",
            true,
            233..263,
            191,
            13,
        ),
        (
            "editorial-notes",
            1,
            "Éditorial Notes",
            false,
            286..300,
            263,
            17,
        ),
        (
            "reference-note",
            1,
            "Référence note",
            false,
            325..343,
            300,
            22,
        ),
        (
            "alt-cover-text",
            1,
            "Alt Cover text",
            false,
            381..399,
            343,
            26,
        ),
        ("broken", 1, "Broken", false, 408..419, 399, 30),
        ("unknown", 1, "Unknown", false, 429..473, 419, 34),
        (
            "code-span-shields-emphasis",
            1,
            "Code *span* shields emphasis",
            false,
            506..544,
            473,
            39,
        ),
        (
            "reference-nested-inside-a-quote",
            1,
            "Référence nested inside a quote",
            false,
            586..600,
            544,
            43,
        ),
    ];
    assert_eq!(analysis.sections.len(), expected_sections.len());
    for (index, (id, level, text, explicit, body_range, offset, line)) in
        expected_sections.iter().enumerate()
    {
        let section = &analysis.sections[index];
        assert_eq!(
            section,
            &AnalyzedSection {
                id: (*id).to_string(),
                level: *level,
                text: (*text).to_string(),
                explicit: *explicit,
                body_range: body_range.clone(),
                offset: *offset,
                line: *line,
            },
            "section {index}"
        );
    }

    let expected_bindings: [(&str, &str, Option<&str>, usize); 0] = [];
    assert_eq!(analysis.bindings.len(), expected_bindings.len());
    for (index, (slug, component, class, section_index)) in expected_bindings.iter().enumerate() {
        assert_eq!(
            &analysis.bindings[index],
            &PendingBinding {
                slug: (*slug).to_string(),
                component: (*component).to_string(),
                class: class.map(|value| value.to_string()),
                section_index: *section_index,
            },
            "binding {index}"
        );
    }

    let expected_degraded: [(&str, &str); 2] =
        [("results", "cards"), ("quarterly-results-2", "timeline")];
    assert_eq!(analysis.degraded.len(), expected_degraded.len());
    for (index, (slug, component)) in expected_degraded.iter().enumerate() {
        assert_eq!(
            &analysis.degraded[index],
            &DegradedBinding {
                slug: (*slug).to_string(),
                component: (*component).to_string(),
            },
            "degraded {index}"
        );
    }

    let expected_diagnostics = [
        (
            "W-SECT-01",
            Severity::Warning,
            "duplicate explicit heading id",
            None,
            None,
        ),
        (
            "W-COMP-02",
            Severity::Warning,
            "section body must contain only child headings",
            Some("cards"),
            Some("results"),
        ),
        (
            "W-COMP-02",
            Severity::Warning,
            "section body must be a single nonempty list",
            Some("timeline"),
            Some("quarterly-results-2"),
        ),
        (
            "E-SECT-01",
            Severity::Error,
            "sections key has no matching heading slug",
            None,
            Some("orphan"),
        ),
        (
            "W-COMP-02",
            Severity::Warning,
            "section binding class must be a valid CSS identifier list",
            Some("cards"),
            Some("broken"),
        ),
        (
            "W-COMP-02",
            Severity::Warning,
            "unknown section component",
            Some("mystery"),
            Some("unknown"),
        ),
    ];
    assert_eq!(analysis.diagnostics.len(), expected_diagnostics.len());
    for (index, (code, severity, message, name, target)) in expected_diagnostics.iter().enumerate()
    {
        assert_eq!(
            &analysis.diagnostics[index],
            &Diagnostic {
                code: *code,
                severity: *severity,
                message: (*message).to_string(),
                name: name.map(|value| value.to_string()),
                target: target.map(|value| value.to_string()),
            },
            "diagnostic {index}"
        );
    }
}

#[test]
fn slugify_matches_sect01_normative_cases() {
    assert_eq!(slugify("Crème brûlée / 2"), "creme-brulee--2");
    assert_eq!(slugify("  A\tB\nC  "), "-a-b-c-");
    assert_eq!(slugify("Symbols: * & ?"), "symbols---");
    assert_eq!(slugify(""), "");
    assert_eq!(slugify(" "), "-");
    assert_eq!(slugify("-a_b-"), "-a_b-");
    assert_eq!(slugify("a\u{0301}b"), "ab");
    assert_eq!(slugify("éèêë"), "eeee");
}

#[test]
fn slugify_maps_latin_layers_and_angstrom() {
    assert_eq!(slugify("Ångström"), "angstrom");
    assert_eq!(slugify("ǺA"), "aa");
    assert_eq!(slugify("İstanbul"), "istanbul");
    assert_eq!(slugify("\u{212A} (Kelvin)"), "k-kelvin");
    assert_eq!(slugify("\u{212B}"), "a");
    assert_eq!(slugify("Crème brûlée / 2"), "creme-brulee--2");
}

#[test]
fn heading_text_projection_matches_inline_text_conventions() {
    let text_of = |source: &str| analyze_document(source).sections[0].text.clone();

    assert_eq!(text_of("# A \\*x\\* B\n"), "A *x* B");
    assert_eq!(text_of("# Code `a**b**c`\n"), "Code a**b**c");
    assert_eq!(
        text_of("# *em* **strong** ~~strike~~ _under_\n"),
        "em strong strike under"
    );
    assert_eq!(
        text_of("# [label](https://example.test/path \"Title\")\n"),
        "label"
    );
    assert_eq!(text_of("# ![alt *raw*](img.png)\n"), "alt *raw*");
    assert_eq!(text_of("# Target[^note]\n"), "Target");
    assert_eq!(
        text_of("# A [text][n]\n[n]: https://example.test/x\n"),
        "A text"
    );
    assert_eq!(
        text_of("# A [text][]\n[text]: https://example.test/x\n"),
        "A text"
    );
    assert_eq!(
        text_of("# A [text]\n[text]: https://example.test/x\n"),
        "A text"
    );
    assert_eq!(text_of("# A [TEXT  X][]\n[text x]: /t\n"), "A TEXT  X");
    assert_eq!(text_of("# A [Text]\n[TEXT]: /t\n"), "A Text");
    assert_eq!(text_of("# A [text][]\n[c]: /c\n"), "A [text][]");
    assert_eq!(text_of("# A [text]\n[s]: /s\n"), "A [text]");
    assert_eq!(text_of("# A [text][missing]\n"), "A [text][missing]");
    assert_eq!(text_of("# ![alt][]\n[alt]: img.png\n"), "![alt][]");
    assert_eq!(
        text_of("# A [text](unquoted title)\n"),
        "A [text](unquoted title)"
    );
    assert_eq!(
        text_of("# `code *x*` and [a [b] c](u)\n"),
        "code *x* and a [b] c"
    );
}

#[test]
fn code_spans_shield_delimiters_in_projection() {
    let text_of = |source: &str| analyze_document(source).sections[0].text.clone();

    assert_eq!(text_of("# _a `_b` c_\n"), "_a _b c_");
    assert_eq!(text_of("# *a `*b` c*\n"), "*a *b c*");
    assert_eq!(text_of("# **a `**b` c**\n"), "**a **b c**");
    assert_eq!(text_of("# ~~a `~~b` c~~\n"), "~~a ~~b c~~");
    assert_eq!(text_of("# *a `b` c*\n"), "*a b c*");
    assert_eq!(text_of("# `x` *a* `y`\n"), "x a y");
    assert_eq!(text_of("# _a_ `_`\n"), "a _");
    assert_eq!(
        text_of("# `code *x*` and [a [b] c](u)\n"),
        "code *x* and a [b] c"
    );

    let analysis = analyze_document("# _a `_b` c_\n");
    assert_eq!(analysis.sections[0].id, "_a-_b-c_");
    assert_eq!(slugify(&analysis.sections[0].text), "_a-_b-c_");
}

#[test]
fn reference_definitions_nested_in_containers_resolve() {
    let text_of = |source: &str| analyze_document(source).sections[0].text.clone();

    assert_eq!(text_of("# A [text][x]\n\n> [x]: /url\n"), "A text");
    assert_eq!(text_of("# A [text][x]\n\n- [x]: /url\n"), "A text");
    assert_eq!(
        text_of("# A [text][x]\n\n::: note\n[x]: /url\n:::\n"),
        "A text"
    );
    assert_eq!(
        text_of("# A [text][x]\n\n> deep\n> > [x]: /url\n"),
        "A text"
    );
    assert_eq!(text_of("# A [text][x]\n\n> - [x]: /url\n"), "A text");
    assert_eq!(text_of("# A [text][x]\n\n[^f]: [x]: /url\n"), "A text");

    assert_eq!(text_of("# A [text][x]\n\npara\n[x]: /url\n"), "A [text][x]");
    assert_eq!(
        text_of("# A [text][x]\n\n> para\n> [x]: /url\n"),
        "A [text][x]"
    );

    let analysis = analyze_document("# A [text][x]\n\n> [x]: /url\n");
    assert_eq!(analysis.sections[0].id, "a-text");
}

#[test]
fn explicit_duplicate_ids_warn_and_suffix() {
    let analysis = analyze_document("---\ntitle: T\n---\n# One {#Same ID}\n\n# Two {#same-id}\n");
    let ids: Vec<&str> = analysis
        .sections
        .iter()
        .map(|section| section.id.as_str())
        .collect();
    assert_eq!(ids, ["same-id", "same-id-2"]);
    assert_eq!(analysis.sections[0].explicit, true);
    assert_eq!(analysis.sections[1].explicit, true);
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.diagnostics[0].code, "W-SECT-01");
    assert_eq!(analysis.diagnostics[0].severity, Severity::Warning);
    assert_eq!(analysis.diagnostics[0].name, None);
    assert_eq!(analysis.diagnostics[0].target, None);
}

#[test]
fn collision_suffixes_follow_document_order() {
    let analysis = analyze_document("---\ntitle: T\n---\n# A\n\n# A\n\n# A\n");
    let ids: Vec<&str> = analysis
        .sections
        .iter()
        .map(|section| section.id.as_str())
        .collect();
    assert_eq!(ids, ["a", "a-2", "a-3"]);
    assert!(analysis.diagnostics.is_empty());
}

#[test]
fn section_body_ranges_follow_the_level_rule() {
    let analysis = analyze_document("# A\n## B\n### C\n## D\n# E\n");
    let ranges: Vec<_> = analysis
        .sections
        .iter()
        .map(|section| section.body_range.clone())
        .collect();
    assert_eq!(ranges, [4..20, 9..15, 15..15, 20..20, 24..24]);
}

#[test]
fn config_normalization_applies_defaults_and_diagnostics() {
    let analysis = analyze_document("# Heading\n");
    let config = &analysis.config;
    assert_eq!(config.title, None);
    assert_eq!(config.lang.as_deref(), Some("en"));
    assert_eq!(config.theme, mdhtml::analysis::Theme::Technical);
    assert_eq!(config.fonts, mdhtml::analysis::Fonts::Auto);
    assert_eq!(
        config.toc,
        mdhtml::analysis::TocSetting::Enabled(mdhtml::analysis::Toc {
            depth: 3,
            position: mdhtml::analysis::TocPosition::Side
        })
    );
    assert_eq!(config.sections, Value::Mapping(Vec::new()));
    assert_eq!(config.figures, Value::Mapping(Vec::new()));
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.diagnostics[0].code, "E-FMT-05");
    assert_eq!(analysis.diagnostics[0].severity, Severity::Error);
    assert_eq!(
        analysis.diagnostics[0].message,
        "front matter title is required and must be a nonempty string"
    );

    let analysis = analyze_document("---\ntitle: \"\"\n---\n# Heading\n");
    assert_eq!(analysis.config.title, None);
    assert_eq!(analysis.diagnostics[0].code, "E-FMT-05");

    let analysis = analyze_document("---\ntitle: 42\n---\n# Heading\n");
    assert_eq!(analysis.config.title, None);
    assert_eq!(analysis.diagnostics[0].code, "E-FMT-05");

    let analysis = analyze_document(
        "---\ntitle: T\nsummary: S\nlang: pt\ntheme: editorial\nurl: https://example.test\ncover: images/cover.png\n---\n# H\n",
    );
    assert_eq!(analysis.config.title.as_deref(), Some("T"));
    assert_eq!(analysis.config.summary.as_deref(), Some("S"));
    assert_eq!(analysis.config.lang.as_deref(), Some("pt"));
    assert_eq!(analysis.config.theme, mdhtml::analysis::Theme::Editorial);
    assert_eq!(analysis.config.url.as_deref(), Some("https://example.test"));
    assert_eq!(analysis.config.cover.as_deref(), Some("images/cover.png"));
    assert!(analysis.diagnostics.is_empty());

    let analysis = analyze_document("---\ntitle: T\ntheme: custom.theme.css\n---\n# H\n");
    assert_eq!(
        analysis.config.theme,
        mdhtml::analysis::Theme::Local("custom.theme.css".to_string())
    );
    assert!(analysis.diagnostics.is_empty());

    let analysis = analyze_document("---\ntitle: T\nfonts: system\n---\n# H\n");
    assert_eq!(analysis.config.fonts, mdhtml::analysis::Fonts::System);
    let analysis = analyze_document("---\ntitle: T\nfonts: auto\n---\n# H\n");
    assert_eq!(analysis.config.fonts, mdhtml::analysis::Fonts::Auto);
    let analysis = analyze_document(
        "---\ntitle: T\nfonts: { body: body.woff2, mono: mono.woff2 }\n---\n# H\n",
    );
    assert_eq!(
        analysis.config.fonts,
        mdhtml::analysis::Fonts::Map {
            body: Some("body.woff2".to_string()),
            mono: Some("mono.woff2".to_string()),
            url: None,
        }
    );
    let analysis = analyze_document(
        "---\ntitle: T\nfonts: { url: https://fonts.example.test/css }\n---\n# H\n",
    );
    assert_eq!(
        analysis.config.fonts,
        mdhtml::analysis::Fonts::Map {
            body: None,
            mono: None,
            url: Some("https://fonts.example.test/css".to_string()),
        }
    );

    let analysis = analyze_document("---\ntitle: T\ntoc: false\n---\n# H\n");
    assert_eq!(analysis.config.toc, mdhtml::analysis::TocSetting::Disabled);
    let analysis = analyze_document("---\ntitle: T\ntoc: { depth: 6, position: side }\n---\n# H\n");
    assert_eq!(
        analysis.config.toc,
        mdhtml::analysis::TocSetting::Enabled(mdhtml::analysis::Toc {
            depth: 6,
            position: mdhtml::analysis::TocPosition::Side
        })
    );
    let analysis = analyze_document("---\ntitle: T\ntoc: { position: inline }\n---\n# H\n");
    assert_eq!(
        analysis.config.toc,
        mdhtml::analysis::TocSetting::Enabled(mdhtml::analysis::Toc {
            depth: 3,
            position: mdhtml::analysis::TocPosition::Inline
        })
    );
}

#[test]
fn config_rejects_invalid_reserved_values() {
    let cases = [
        (
            "theme: bogus\n",
            "config key theme names an unknown preset; using technical",
        ),
        (
            "theme: 3\n",
            "config key theme must be a string; using technical",
        ),
        ("lang: 3\n", "config key lang must be a string; ignored"),
        (
            "summary: []\n",
            "config key summary must be a string; ignored",
        ),
        ("url: true\n", "config key url must be a string; ignored"),
        ("cover: {}\n", "config key cover must be a string; ignored"),
        (
            "tokens: [a]\n",
            "config key tokens must be a mapping; ignored",
        ),
        (
            "figures: 3\n",
            "config key figures must be a mapping; ignored",
        ),
        (
            "sections: [a]\n",
            "config key sections must be a mapping; ignored",
        ),
        (
            "fonts: true\n",
            "config key fonts must be auto, system, or a mapping; using auto",
        ),
        (
            "fonts: { body: 3 }\n",
            "config key fonts.body must be a string; ignored",
        ),
        (
            "fonts: { mono: [] }\n",
            "config key fonts.mono must be a string; ignored",
        ),
        (
            "fonts: { url: 1 }\n",
            "config key fonts.url must be a string; ignored",
        ),
        (
            "fonts: { extra: x }\n",
            "config key fonts contains unknown key extra; ignored",
        ),
        (
            "toc: 3\n",
            "config key toc must be false or a mapping; using default",
        ),
        (
            "toc: { depth: 0 }\n",
            "config key toc.depth must be an integer from 1 to 6; using default",
        ),
        (
            "toc: { depth: 7 }\n",
            "config key toc.depth must be an integer from 1 to 6; using default",
        ),
        (
            "toc: { depth: 2.5 }\n",
            "config key toc.depth must be an integer from 1 to 6; using default",
        ),
        (
            "toc: { depth: \"3\" }\n",
            "config key toc.depth must be an integer from 1 to 6; using default",
        ),
        (
            "toc: { position: top }\n",
            "config key toc.position must be side or inline; using default",
        ),
        (
            "toc: { position: 1 }\n",
            "config key toc.position must be side or inline; using default",
        ),
        (
            "toc: { extra: 1 }\n",
            "config key toc contains unknown key extra; ignored",
        ),
    ];
    for (front, message) in cases {
        let analysis = analyze_document(&format!("---\ntitle: T\n{front}---\n# H\n"));
        assert_eq!(analysis.config.title.as_deref(), Some("T"), "{front}");
        assert_eq!(
            analysis.config.theme,
            mdhtml::analysis::Theme::Technical,
            "{front}"
        );
        let expected_fonts = if front.starts_with("fonts: {") {
            mdhtml::analysis::Fonts::Map {
                body: None,
                mono: None,
                url: None,
            }
        } else {
            mdhtml::analysis::Fonts::Auto
        };
        assert_eq!(analysis.config.fonts, expected_fonts, "{front}");
        assert_eq!(
            analysis.config.toc,
            mdhtml::analysis::TocSetting::Enabled(mdhtml::analysis::Toc {
                depth: 3,
                position: mdhtml::analysis::TocPosition::Side
            }),
            "{front}"
        );
        let config_diagnostics: Vec<&Diagnostic> = analysis
            .diagnostics
            .iter()
            .filter(|d| d.code == "W-CONFIG-01")
            .collect();
        assert_eq!(config_diagnostics.len(), 1, "{front}");
        assert_eq!(config_diagnostics[0].severity, Severity::Warning, "{front}");
        assert_eq!(config_diagnostics[0].message, message, "{front}");
        assert_eq!(config_diagnostics[0].name, None, "{front}");
        assert_eq!(config_diagnostics[0].target, None, "{front}");
    }
}

#[test]
fn bindings_follow_mapping_order_and_report_names() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  target: { component: timeline, class: \"one two\" }\n  orphan: { component: cards }\n  bad-shape: []\n  missing-component: { class: good }\n  non-string-component: { component: [timeline] }\n  empty-component: { component: \"\" }\n  bad-class: { component: timeline, class: \"good!\" }\n  unknown: { component: mystery }\n  bad-class-leading: { component: cards, class: \" one\" }\n---\n# Target\n- One\n\n# Bad Shape\nText.\n\n# Missing Component\nText.\n\n# Non String Component\nText.\n\n# Empty Component\nText.\n\n# Bad Class\n- One\n\n# Unknown\nText.\n\n# Bad Class Leading\nText.\n",
    );

    let bindings: Vec<(&str, &str, Option<&str>, usize)> = analysis
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
        .collect();
    assert_eq!(bindings, [("target", "timeline", Some("one two"), 0)]);

    let records: Vec<(String, Option<String>, Option<String>)> = analysis
        .diagnostics
        .iter()
        .map(|d| (d.code.to_string(), d.name.clone(), d.target.clone()))
        .collect();
    assert_eq!(
        records,
        [
            ("E-SECT-01".to_string(), None, Some("orphan".to_string())),
            (
                "W-COMP-02".to_string(),
                Some(String::new()),
                Some("bad-shape".to_string())
            ),
            (
                "W-COMP-02".to_string(),
                Some(String::new()),
                Some("missing-component".to_string())
            ),
            (
                "W-COMP-02".to_string(),
                Some(String::new()),
                Some("non-string-component".to_string())
            ),
            (
                "W-COMP-02".to_string(),
                Some(String::new()),
                Some("empty-component".to_string())
            ),
            (
                "W-COMP-02".to_string(),
                Some("timeline".to_string()),
                Some("bad-class".to_string())
            ),
            (
                "W-COMP-02".to_string(),
                Some("mystery".to_string()),
                Some("unknown".to_string())
            ),
            (
                "W-COMP-02".to_string(),
                Some("cards".to_string()),
                Some("bad-class-leading".to_string())
            ),
        ]
    );
}

#[test]
fn numeric_section_keys_preserve_binding_and_orphan_order() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  2: { component: timeline }\n  4: { component: cards }\n  1: { component: hero }\n  3: { component: cards }\n---\n# Alpha {#1}\nText.\n\n# Beta {#2}\nText.\n",
    );

    let bindings: Vec<(&str, &str, usize)> = analysis
        .bindings
        .iter()
        .map(|binding| {
            (
                binding.slug.as_str(),
                binding.component.as_str(),
                binding.section_index,
            )
        })
        .collect();
    assert_eq!(bindings, [("1", "hero", 0)]);

    let degraded: Vec<(&str, &str)> = analysis
        .degraded
        .iter()
        .map(|binding| (binding.slug.as_str(), binding.component.as_str()))
        .collect();
    assert_eq!(degraded, [("2", "timeline")]);

    let records: Vec<(String, Option<String>)> = analysis
        .diagnostics
        .iter()
        .map(|d| (d.code.to_string(), d.target.clone()))
        .collect();
    assert_eq!(
        records,
        [
            ("W-COMP-02".to_string(), Some("2".to_string())),
            ("E-SECT-01".to_string(), Some("4".to_string())),
            ("E-SECT-01".to_string(), Some("3".to_string())),
        ]
    );
}

#[test]
fn binding_matches_final_collision_ids() {
    let analysis = analyze_document(
        "---\ntitle: T\nsections:\n  a-2: { component: cards }\n---\n# A\n\n# A\n## Child\n",
    );
    assert_eq!(analysis.bindings.len(), 1);
    assert_eq!(analysis.bindings[0].slug, "a-2");
    assert_eq!(analysis.bindings[0].section_index, 1);
    assert!(analysis.diagnostics.is_empty());
}

#[test]
fn invalid_front_matter_reports_a_parse_error() {
    let analysis = analyze_document("---\na: 1\n");
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.diagnostics[0].code, "E-PARSE-01");
    assert_eq!(analysis.diagnostics[0].severity, Severity::Error);
    assert_eq!(analysis.config.title, None);
    assert!(analysis.sections.is_empty());
    assert!(analysis.bindings.is_empty());
}
