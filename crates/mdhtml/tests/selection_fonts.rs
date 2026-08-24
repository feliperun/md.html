use mdhtml::analysis::{Analysis, Fonts, NormalizedConfig, Theme};
use mdhtml::selection::fonts::{load, select_faces};
use mdhtml::selection::{Catalog, Family, SelectedFace, SelectionError, WghtRange};
use std::path::{Path, PathBuf};

fn fonts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fonts")
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn analysis_with(fonts: Fonts, theme: Theme) -> Analysis {
    Analysis {
        config: NormalizedConfig {
            fonts,
            theme,
            ..NormalizedConfig::default()
        },
        sections: Vec::new(),
        bindings: Vec::new(),
        degraded: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn family<'a>(catalog: &'a Catalog, key: &str) -> &'a Family {
    catalog
        .families
        .iter()
        .find(|family| family.key == key)
        .expect("family present")
}

fn face_files(faces: &[SelectedFace]) -> Vec<&str> {
    faces.iter().map(|face| face.file.as_str()).collect()
}

fn problem_codes(error: &SelectionError) -> Vec<&str> {
    error.problems.iter().map(|problem| problem.code).collect()
}

fn problem_messages(error: &SelectionError) -> Vec<String> {
    error
        .problems
        .iter()
        .map(|problem| problem.message.clone())
        .collect()
}

#[test]
fn load_committed_catalog_verifies_schema_and_declared_bytes() {
    let catalog = load(&fonts_dir().join("catalog.json")).expect("committed catalog loads clean");

    let presets: Vec<(&str, Option<&str>, Option<&str>)> = catalog
        .presets
        .iter()
        .map(|preset| {
            (
                preset.name.as_str(),
                preset.body.as_deref(),
                preset.mono.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        presets,
        [
            ("technical", Some("instrument-sans"), Some("geist-mono")),
            ("editorial", Some("newsreader"), Some("geist-mono")),
            ("system", None, None),
        ]
    );

    let family_keys: Vec<&str> = catalog
        .families
        .iter()
        .map(|family| family.key.as_str())
        .collect();
    assert_eq!(family_keys, ["instrument-sans", "newsreader", "geist-mono"]);

    let instrument = family(&catalog, "instrument-sans");
    assert_eq!(instrument.name, "Instrument Sans");
    assert_eq!(instrument.role, "body");
    assert_eq!(instrument.license, "OFL-1.1");
    assert_eq!(instrument.license_file, "InstrumentSans-OFL.txt");
    assert_eq!(instrument.notice, "NOTICE.md");

    let faces: Vec<(&str, &str, u64, u64, u64, &str, &str, &str, &str)> = instrument
        .faces
        .iter()
        .map(|face| {
            (
                face.name.as_str(),
                face.style.as_str(),
                face.axes.min,
                face.axes.max,
                face.bytes,
                face.file.as_str(),
                face.sha256.as_str(),
                face.license_file.as_str(),
                face.notice.as_str(),
            )
        })
        .collect();
    assert_eq!(
        faces,
        [
            (
                "normal",
                "normal",
                400,
                700,
                30092,
                "InstrumentSans-latin-wght-normal.woff2",
                "2ee17598a98d8a59e4df8152d015bec9ab8e4d5672cc0ab42bef806b568e3971",
                "InstrumentSans-OFL.txt",
                "NOTICE.md",
            ),
            (
                "italic",
                "italic",
                400,
                700,
                31828,
                "InstrumentSans-latin-wght-italic.woff2",
                "77210cdde0281b5ecb0d592e063a98656f1bc36993a3b98f506eb91ff4a433a5",
                "InstrumentSans-OFL.txt",
                "NOTICE.md",
            ),
        ]
    );

    let geist = family(&catalog, "geist-mono");
    assert_eq!(geist.role, "mono");
    assert_eq!(geist.faces.len(), 1);
    assert_eq!(geist.faces[0].axes, WghtRange { min: 100, max: 900 });
    assert_eq!(geist.faces[0].bytes, 71596);
    assert_eq!(
        geist.faces[0].sha256,
        "afaacc4c5fbba89d2ebf7a02dc4070208540874592a5504d57175782fe893101"
    );
}

#[test]
fn select_faces_orders_body_normal_italic_then_mono() {
    let catalog = load(&fonts_dir().join("catalog.json")).expect("committed catalog loads");
    let analysis = analysis_with(Fonts::Auto, Theme::Technical);

    let faces = select_faces(&analysis, "Body with *emphasis* and `code`.", &catalog);
    assert_eq!(
        faces,
        [
            SelectedFace {
                family: "instrument-sans".to_string(),
                style: "normal".to_string(),
                file: "InstrumentSans-latin-wght-normal.woff2".to_string(),
                bytes: 30092,
                sha256: "2ee17598a98d8a59e4df8152d015bec9ab8e4d5672cc0ab42bef806b568e3971"
                    .to_string(),
                axes: WghtRange { min: 400, max: 700 },
                license_file: "InstrumentSans-OFL.txt".to_string(),
                notice: "NOTICE.md".to_string(),
            },
            SelectedFace {
                family: "instrument-sans".to_string(),
                style: "italic".to_string(),
                file: "InstrumentSans-latin-wght-italic.woff2".to_string(),
                bytes: 31828,
                sha256: "77210cdde0281b5ecb0d592e063a98656f1bc36993a3b98f506eb91ff4a433a5"
                    .to_string(),
                axes: WghtRange { min: 400, max: 700 },
                license_file: "InstrumentSans-OFL.txt".to_string(),
                notice: "NOTICE.md".to_string(),
            },
            SelectedFace {
                family: "geist-mono".to_string(),
                style: "normal".to_string(),
                file: "GeistMono-wght-normal.woff2".to_string(),
                bytes: 71596,
                sha256: "afaacc4c5fbba89d2ebf7a02dc4070208540874592a5504d57175782fe893101"
                    .to_string(),
                axes: WghtRange { min: 100, max: 900 },
                license_file: "GeistMono-OFL.txt".to_string(),
                notice: "NOTICE.md".to_string(),
            },
        ]
    );
}

#[test]
fn select_faces_italic_only_with_emphasis_and_mono_only_with_code() {
    let catalog = load(&fonts_dir().join("catalog.json")).expect("committed catalog loads");
    let analysis = analysis_with(Fonts::Auto, Theme::Technical);

    let code_only = select_faces(&analysis, "Body with `code`.", &catalog);
    assert_eq!(
        face_files(&code_only),
        [
            "InstrumentSans-latin-wght-normal.woff2",
            "GeistMono-wght-normal.woff2",
        ]
    );

    let emphasis_only = select_faces(&analysis, "Body with *emphasis*.", &catalog);
    assert_eq!(
        face_files(&emphasis_only),
        [
            "InstrumentSans-latin-wght-normal.woff2",
            "InstrumentSans-latin-wght-italic.woff2",
        ]
    );

    let plain = select_faces(&analysis, "Plain prose.", &catalog);
    assert_eq!(
        face_files(&plain),
        ["InstrumentSans-latin-wght-normal.woff2"]
    );
}

#[test]
fn select_faces_resolves_editorial_for_auto_editorial_theme() {
    let catalog = load(&fonts_dir().join("catalog.json")).expect("committed catalog loads");
    let analysis = analysis_with(Fonts::Auto, Theme::Editorial);

    let faces = select_faces(&analysis, "Body with *emphasis* and `code`.", &catalog);
    assert_eq!(
        face_files(&faces),
        [
            "Newsreader-latin-wght-normal.woff2",
            "Newsreader-latin-wght-italic.woff2",
            "GeistMono-wght-normal.woff2",
        ]
    );
}

#[test]
fn select_faces_resolves_technical_for_local_themes() {
    let catalog = load(&fonts_dir().join("catalog.json")).expect("committed catalog loads");
    let analysis = analysis_with(Fonts::Auto, Theme::Local("custom".to_string()));

    let faces = select_faces(&analysis, "Plain prose.", &catalog);
    assert_eq!(
        face_files(&faces),
        ["InstrumentSans-latin-wght-normal.woff2"]
    );
}

#[test]
fn select_faces_resolves_system_to_no_builtin_faces() {
    let catalog = load(&fonts_dir().join("catalog.json")).expect("committed catalog loads");
    let analysis = analysis_with(Fonts::System, Theme::Editorial);

    assert_eq!(
        select_faces(&analysis, "Body with *emphasis* and `code`.", &catalog),
        Vec::<SelectedFace>::new()
    );
}

#[test]
fn select_faces_returns_no_builtin_faces_for_map_fonts() {
    let catalog = load(&fonts_dir().join("catalog.json")).expect("committed catalog loads");
    let analysis = analysis_with(
        Fonts::Map {
            body: Some("serif".to_string()),
            mono: Some("monospace".to_string()),
            url: None,
        },
        Theme::Technical,
    );

    assert_eq!(
        select_faces(&analysis, "Body with *emphasis* and `code`.", &catalog),
        Vec::<SelectedFace>::new()
    );
}

#[test]
fn invalid_keys_catalog_reports_every_schema_problem() {
    let error = load(&fixtures_dir().join("catalog-invalid-keys.json"))
        .expect_err("invalid-keys catalog must fail closed");
    let codes = problem_codes(&error);
    assert!(
        codes.iter().all(|code| *code == "E-FONTS-02"),
        "schema problems only: {codes:?}"
    );

    let messages = problem_messages(&error);
    let closure = messages
        .iter()
        .find(|m| m.contains("top-level keys are not closed"))
        .expect("top-level closure problem is reported");
    assert!(
        closure.contains("id") && closure.contains("source"),
        "top-level closure rejects harness metadata keys: {closure}"
    );
    assert!(
        closure.contains("extra"),
        "top-level closure names every unknown key: {closure}"
    );
    assert!(messages.iter().any(|m| m.contains("format")));
    assert!(messages.iter().any(|m| m.contains("presets are invalid")));
    assert!(messages.iter().any(|m| m.contains("families are invalid")));
    assert!(
        messages
            .iter()
            .any(|m| m.contains("instrument-sans: name is invalid"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("instrument-sans: source is invalid"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("instrument-sans: faces are not closed"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("instrument-sans/normal: face keys are not closed"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("instrument-sans/normal: axes are invalid"))
    );
    assert!(messages.iter().any(|m| m.contains("catalog declares opsz")));
    assert_eq!(
        error.problems.len(),
        10,
        "every schema problem is an entry: {messages:?}"
    );
}

#[test]
fn invalid_face_catalog_reports_size_hash_and_missing_license_files() {
    let error = load(&fixtures_dir().join("catalog-invalid-face.json"))
        .expect_err("invalid-face catalog must fail closed");

    let codes = problem_codes(&error);
    assert!(
        codes.contains(&"E-FONTS-04"),
        "size mismatch reported: {codes:?}"
    );
    assert!(
        codes.contains(&"E-FONTS-05"),
        "hash mismatch reported: {codes:?}"
    );
    assert!(
        codes.contains(&"E-FONTS-03"),
        "license files are not reachable from the fixtures dir: {codes:?}"
    );

    let messages = problem_messages(&error);
    assert!(
        messages
            .iter()
            .any(|m| m.contains("newsreader/normal: size mismatch"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("instrument-sans/normal: sha256 mismatch"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("license file 'InstrumentSans-OFL.txt' is unreadable")),
        "every declared license file must exist beside the catalog: {messages:?}"
    );
    assert_eq!(
        error.problems.len(),
        5,
        "every byte problem is an entry: {messages:?}"
    );
}
