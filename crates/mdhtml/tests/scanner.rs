use mdhtml::frontmatter::parse_front_matter;
use mdhtml::scanner::{
    ContainerEvidence, HeadingEvidence, ImageEvidence, ImageKind, ScanEvidence, scan_document,
};
use std::fs;
use std::path::{Path, PathBuf};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/document-scan.md")
}

fn scan_body(source: &str) -> ScanEvidence<'_> {
    scan_document(
        &parse_front_matter(source)
            .expect("fixture front matter parses")
            .body,
    )
}

#[test]
fn fixture_document_scan_matches_expected_evidence() {
    let source = fs::read_to_string(fixture_path()).expect("read document-scan.md");
    let evidence = scan_body(&source);

    assert_eq!(evidence.headings.len(), 3);
    assert_eq!(
        evidence.headings[0],
        HeadingEvidence {
            level: 1,
            text: "Heading 1",
            explicit_id: Some("custom-h1"),
            offset: 1,
            line: 2,
        }
    );
    assert_eq!(evidence.headings[1].level, 2);
    assert_eq!(evidence.headings[1].text, "Heading with closing hashes");
    assert_eq!(evidence.headings[1].explicit_id, None);
    assert_eq!(evidence.headings[1].offset, 1172);
    assert_eq!(evidence.headings[1].line, 40);
    assert_eq!(
        evidence.headings[2].text,
        "Heading with inline *source* and `code`"
    );
    assert_eq!(evidence.headings[2].explicit_id, Some("h3-id"));
    assert_eq!(evidence.headings[2].offset, 1208);
    assert_eq!(evidence.headings[2].line, 42);

    assert_eq!(evidence.images.len(), 9);
    let expected_images = [
        ImageEvidence {
            kind: ImageKind::Markdown,
            destination: "images/diagram.png".to_string(),
            offset: 177,
            line: 6,
        },
        ImageEvidence {
            kind: ImageKind::Html,
            destination: "images/photo.jpg".to_string(),
            offset: 240,
            line: 6,
        },
        ImageEvidence {
            kind: ImageKind::Markdown,
            destination: "images/nested path/pic.png".to_string(),
            offset: 300,
            line: 7,
        },
        ImageEvidence {
            kind: ImageKind::Markdown,
            destination: "img/esc(1).png".to_string(),
            offset: 348,
            line: 7,
        },
        ImageEvidence {
            kind: ImageKind::Markdown,
            destination: "https://cdn.example.com/logo.png".to_string(),
            offset: 381,
            line: 7,
        },
        ImageEvidence {
            kind: ImageKind::Markdown,
            destination: "data:image/png;base64,iVBORw0KGgo=".to_string(),
            offset: 431,
            line: 7,
        },
        ImageEvidence {
            kind: ImageKind::Markdown,
            destination: "images/ref-target.png".to_string(),
            offset: 492,
            line: 8,
        },
        ImageEvidence {
            kind: ImageKind::Markdown,
            destination: "images/collapsed.png".to_string(),
            offset: 523,
            line: 8,
        },
        ImageEvidence {
            kind: ImageKind::Markdown,
            destination: "images/shortcut.png".to_string(),
            offset: 548,
            line: 8,
        },
    ];
    for (index, expected) in expected_images.iter().enumerate() {
        assert_eq!(&evidence.images[index], expected, "image {index}");
    }

    assert_eq!(evidence.containers.len(), 4);
    let expected_containers = [
        ContainerEvidence {
            name: "note",
            argument: Some("Optional Note Note"),
            offset: 944,
            line: 22,
            body_range: 974..995,
        },
        ContainerEvidence {
            name: "warning",
            argument: None,
            offset: 1000,
            line: 26,
            body_range: 1016..1079,
        },
        ContainerEvidence {
            name: "details",
            argument: Some("Summary Text"),
            offset: 1032,
            line: 28,
            body_range: 1059..1075,
        },
        ContainerEvidence {
            name: "quote",
            argument: None,
            offset: 1085,
            line: 33,
            body_range: 1097..1124,
        },
    ];
    for (index, expected) in expected_containers.iter().enumerate() {
        assert_eq!(&evidence.containers[index], expected, "container {index}");
    }

    assert!(evidence.has_emphasis);
    assert!(evidence.has_code);
}

#[test]
fn atx_headings_are_recognized_without_false_positives() {
    let evidence =
        scan_document("   ### Heading Title {#my-id}   \n# Heading 2 ###\n## Heading 3 #\n");
    assert_eq!(evidence.headings.len(), 3);
    assert_eq!(evidence.headings[0].level, 3);
    assert_eq!(evidence.headings[0].text, "Heading Title");
    assert_eq!(evidence.headings[0].explicit_id, Some("my-id"));
    assert_eq!(evidence.headings[0].offset, 0);
    assert_eq!(evidence.headings[0].line, 1);
    assert_eq!(evidence.headings[1].text, "Heading 2");
    assert_eq!(evidence.headings[1].explicit_id, None);
    assert_eq!(evidence.headings[2].text, "Heading 3");
    assert_eq!(evidence.headings[2].explicit_id, None);

    assert!(scan_document("####### seven\n").headings.is_empty());
    assert!(scan_document("#5 bolt\n").headings.is_empty());
    assert!(scan_document("#\n######\n").headings.len() == 2);

    // {#id} must be final and space-separated to be an override
    let not_final = scan_document("# Title {#id} extra\n");
    assert_eq!(not_final.headings[0].text, "Title {#id} extra");
    assert_eq!(not_final.headings[0].explicit_id, None);
    let glued = scan_document("# Title{#id}\n");
    assert_eq!(glued.headings[0].text, "Title{#id}");
    assert_eq!(glued.headings[0].explicit_id, None);

    // Inline source is preserved; only the override and closing hashes are split off
    let inline = scan_document("# *Inline* source `x` {#slug}\n");
    assert_eq!(inline.headings[0].text, "*Inline* source `x`");
    assert_eq!(inline.headings[0].explicit_id, Some("slug"));

    // Four-space indentation is indented code, not a heading
    let indented = scan_document("    # Not a heading\n");
    assert!(indented.headings.is_empty());
    assert!(indented.has_code);
}

#[test]
fn masked_regions_produce_no_other_evidence() {
    let comments = scan_document("<!-- *ignored* ![img](ignored.png) # Ignored -->\nreal text\n");
    assert!(!comments.has_emphasis);
    assert!(comments.images.is_empty());
    assert!(comments.headings.is_empty());
    assert!(!comments.has_code);

    let fenced =
        scan_document("```rust\n*fenced* ![img](not.png) # Not\n::: not-a-container\n```\nafter\n");
    assert!(!fenced.has_emphasis);
    assert!(fenced.images.is_empty());
    assert!(fenced.headings.is_empty());
    assert!(fenced.containers.is_empty());
    assert!(fenced.has_code);

    let unclosed = scan_document("~~~\n*fenced* ![img](not.png) # Not\n");
    assert!(!unclosed.has_emphasis);
    assert!(unclosed.images.is_empty());
    assert!(unclosed.headings.is_empty());
    assert!(unclosed.has_code);

    let indented = scan_document("    *code* ![img](not.png) # Not\n");
    assert!(!indented.has_emphasis);
    assert!(indented.images.is_empty());
    assert!(indented.headings.is_empty());
    assert!(indented.has_code);

    let span = scan_document("`*code*` and `![img](not.png)`\n");
    assert!(!span.has_emphasis);
    assert!(span.images.is_empty());
    assert!(span.has_code);
}

#[test]
fn emphasis_matches_single_delimiters_only() {
    assert!(scan_document("*valid*").has_emphasis);
    assert!(scan_document("_valid_").has_emphasis);
    assert!(scan_document("before *mid* after").has_emphasis);

    assert!(!scan_document("**strong**").has_emphasis);
    assert!(!scan_document("__strong__").has_emphasis);
    assert!(!scan_document("snake_case_id").has_emphasis);
    assert!(!scan_document("\\*escaped\\*").has_emphasis);
    assert!(!scan_document("\\_escaped\\_").has_emphasis);
    assert!(!scan_document("* unmatched").has_emphasis);
    assert!(!scan_document("unmatched *").has_emphasis);
    assert!(!scan_document("a*b").has_emphasis);
    assert!(!scan_document("*a\nb*").has_emphasis);
}

#[test]
fn markdown_images_record_destinations() {
    let evidence = scan_document(
        "![Alt](images/a.png \"Title\") and ![Angle](<images/nested path/b.png>) and ![Esc](img/esc\\(1\\).png)\n",
    );
    assert_eq!(evidence.images.len(), 3);
    assert_eq!(evidence.images[0].destination, "images/a.png");
    assert_eq!(evidence.images[1].destination, "images/nested path/b.png");
    assert_eq!(evidence.images[2].destination, "img/esc(1).png");

    // Remote, data, and unsafe destinations are recorded for later classification
    let unsafe_evidence = scan_document(
        "![R](https://cdn.example.com/x.png) ![D](data:image/png;base64,AA==) ![U](javascript:alert(1))\n",
    );
    assert_eq!(unsafe_evidence.images.len(), 3);
    assert_eq!(
        unsafe_evidence.images[0].destination,
        "https://cdn.example.com/x.png"
    );
    assert_eq!(
        unsafe_evidence.images[1].destination,
        "data:image/png;base64,AA=="
    );
    assert_eq!(unsafe_evidence.images[2].destination, "javascript:alert(1)");

    // Unbalanced alt brackets and missing references produce no image
    assert!(scan_document("![broken(not.png)\n").images.is_empty());
    assert!(scan_document("![unknown]\n").images.is_empty());
    assert!(scan_document("![alt] [not-ref]\n").images.is_empty());
    // Evidence never spans lines: the closing bracket must be on the same line
    assert!(scan_document("x ![a\nb](img.png)\n").images.is_empty());
}

#[test]
fn reference_images_resolve_case_folded_and_collapsed_labels() {
    let full = scan_document("[ReF-1]: images/ref.png \"Title\"\n![x][ref-1]\n");
    assert_eq!(full.images.len(), 1);
    assert_eq!(full.images[0].destination, "images/ref.png");

    let collapsed_label = scan_document("[a  b]: images/two.png\n![x][a b]\n");
    assert_eq!(collapsed_label.images[0].destination, "images/two.png");

    let collapsed_form = scan_document("[Alt]: images/collapsed.png\n![Alt][]\n");
    assert_eq!(collapsed_form.images[0].destination, "images/collapsed.png");

    let shortcut = scan_document("[Alt]: images/shortcut.png\n![Alt]\n");
    assert_eq!(shortcut.images[0].destination, "images/shortcut.png");

    let first_wins = scan_document("[a]: images/one.png\n[a]: images/two.png\n![x][a]\n");
    assert_eq!(first_wins.images[0].destination, "images/one.png");
}

#[test]
fn html_images_are_case_insensitive_and_record_src() {
    let evidence = scan_document(
        r#"<img src="double.png"><IMG SRC='single.png'><img alt="foo" src=unquoted.png><ImG SrC="up.PNG">"#,
    );
    assert_eq!(evidence.images.len(), 4);
    assert_eq!(evidence.images[0].destination, "double.png");
    assert_eq!(evidence.images[1].destination, "single.png");
    assert_eq!(evidence.images[2].destination, "unquoted.png");
    assert_eq!(evidence.images[3].destination, "up.PNG");

    assert!(scan_document("<img alt=\"no src\">\n").images.is_empty());
    assert!(scan_document("<imgx src=\"not.png\">\n").images.is_empty());
    assert!(
        scan_document("`<img src=\"masked.png\">`\n")
            .images
            .is_empty()
    );
}

#[test]
fn containers_follow_the_comp01_grammar() {
    let basic = scan_document("::: note |  arg  \nbody\n:::\n");
    assert_eq!(basic.containers.len(), 1);
    assert_eq!(basic.containers[0].name, "note");
    assert_eq!(basic.containers[0].argument, Some("arg"));
    assert_eq!(basic.containers[0].body_range, 18..23);

    let empty_argument = scan_document("::: quote |\nbody\n:::\n");
    assert_eq!(empty_argument.containers[0].name, "quote");
    assert_eq!(empty_argument.containers[0].argument, None);

    let no_argument = scan_document("::: note\nbody\n:::\n");
    assert_eq!(no_argument.containers[0].argument, None);

    let braced = scan_document(":::: {.warning}\nbody\n::::\n");
    assert_eq!(braced.containers[0].name, "warning");
    assert_eq!(braced.containers[0].argument, None);

    let nested = scan_document("::: outer\nA\n::: inner | x\nB\n:::\n:::\n");
    assert_eq!(nested.containers.len(), 2);
    assert_eq!(nested.containers[0].name, "outer");
    assert_eq!(nested.containers[1].name, "inner");
    assert_eq!(nested.containers[1].argument, Some("x"));

    // Trailing content after the name is not a valid opener
    let trailing = scan_document("::: note trailing garbage\nbody\n:::\n");
    assert!(trailing.containers.is_empty());

    // Unmatched openers and stray closers remain prose
    assert!(scan_document("::: note\nunclosed\n").containers.is_empty());
    assert!(scan_document(":::\nstray closer\n").containers.is_empty());
    assert!(scan_document("::\nnot a closer\n").containers.is_empty());

    // Masked opener/closer lines produce no containers
    let masked = scan_document("```\n::: note\n:::\n```\n");
    assert!(masked.containers.is_empty());
    assert!(masked.has_code);
}

#[test]
fn evidence_offsets_and_lines_are_byte_accurate() {
    let evidence = scan_document("# One\n![a](img.png)\n::: note\nbody\n:::\n");
    assert_eq!(
        evidence.headings[0],
        HeadingEvidence {
            level: 1,
            text: "One",
            explicit_id: None,
            offset: 0,
            line: 1,
        }
    );
    assert_eq!(evidence.images[0].offset, 6);
    assert_eq!(evidence.images[0].line, 2);
    assert_eq!(evidence.containers[0].offset, 20);
    assert_eq!(evidence.containers[0].line, 3);
    assert_eq!(evidence.containers[0].body_range, 29..34);
}

#[test]
fn empty_and_crlf_inputs_are_harmless() {
    assert_eq!(scan_document(""), ScanEvidence::default());
    let crlf = scan_document("# One\r\n![a](img.png)\r\n::: note\r\nbody\r\n:::\r\n");
    assert_eq!(crlf.headings[0].line, 1);
    assert_eq!(crlf.headings[0].text, "One");
    assert_eq!(crlf.images[0].line, 2);
    assert_eq!(crlf.containers[0].line, 3);
    assert_eq!(crlf.containers[0].body_range, 32..38);
}
