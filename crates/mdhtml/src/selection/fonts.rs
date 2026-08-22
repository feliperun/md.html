//! Strict committed font catalog loading (SPEC §18) and deterministic face
//! selection from accepted analysis/scanner evidence.
//!
//! `load` fails closed: schema violations (closed keys, the exact accepted
//! identity/legal/provenance/axes values, and any `opsz` declaration) and
//! byte violations (every declared face and license file must exist beside
//! the catalog with the exact committed bytes, SHA-256, and WOFF2 magic)
//! each become an entry in `SelectionError`. Byte verification runs only
//! over a schema-clean catalog.

use std::fs;
use std::path::Path;

use crate::analysis::{Analysis, Fonts, Theme};
use crate::scanner::scan_document;

use super::json::{self, JsonValue};
use super::manifest::{SelectionError, SelectionProblem};
use super::sha256;

const FORMAT: &str = "mdhtml/fonts/1.0";
const TOP_KEYS: [&str; 3] = ["families", "format", "presets"];
// Top-level keys are closed exactly like fonts/check.mjs checkCatalog: a
// catalog must carry only `families`, `format`, and `presets` — harness
// metadata keys such as `id`/`source` are rejected with the same problem.
const FAMILY_KEYS: [&str; 7] = [
    "faces",
    "license",
    "licenseFile",
    "name",
    "notice",
    "provenance",
    "role",
];
const FACE_KEYS: [&str; 9] = [
    "axes",
    "bytes",
    "file",
    "license",
    "licenseFile",
    "notice",
    "provenance",
    "sha256",
    "style",
];
const PROVENANCE_KEYS: [&str; 4] = ["commit", "integrity", "source", "version"];
const PROVENANCE_PACKAGE_KEYS: [&str; 4] = ["integrity", "package", "source", "version"];

const LICENSE_HASHES: [(&str, &str); 3] = [
    (
        "InstrumentSans-OFL.txt",
        "c27a3c53c3beed7f5c26853afa15991478ff7145d3754a36b0382f84e10c0d03",
    ),
    (
        "Newsreader-OFL.txt",
        "26028ec4e13b650065fa525a09532176f8a668b76ff849ea01c564a7480f91e7",
    ),
    (
        "GeistMono-OFL.txt",
        "c683bfbcc7e087f5d37a54ef628f10387c451a83ddc459b151403a164ac46c90",
    ),
];

/// The exact accepted face record shape (SPEC §18): closed face set and the
/// accepted style and weight range per face, mirroring the committed
/// `fonts/check.mjs` `FONT_SPEC` entries. Declared file/bytes/sha256 are
/// enforced against the real files during byte verification.
struct ExpectedFace {
    name: &'static str,
    style: &'static str,
    min: u64,
    max: u64,
}

/// The exact accepted family record, mirroring `fonts/check.mjs` `FONT_SPEC`.
struct ExpectedFamily {
    key: &'static str,
    name: &'static str,
    role: &'static str,
    license_file: &'static str,
    source: &'static str,
    version: &'static str,
    integrity: &'static str,
    package: Option<&'static str>,
    commit: Option<&'static str>,
    faces: &'static [ExpectedFace],
}

const INSTRUMENT_SANS_FACES: [ExpectedFace; 2] = [
    ExpectedFace {
        name: "normal",
        style: "normal",
        min: 400,
        max: 700,
    },
    ExpectedFace {
        name: "italic",
        style: "italic",
        min: 400,
        max: 700,
    },
];

const NEWSREADER_FACES: [ExpectedFace; 2] = [
    ExpectedFace {
        name: "normal",
        style: "normal",
        min: 200,
        max: 800,
    },
    ExpectedFace {
        name: "italic",
        style: "italic",
        min: 200,
        max: 800,
    },
];

const GEIST_MONO_FACES: [ExpectedFace; 1] = [ExpectedFace {
    name: "normal",
    style: "normal",
    min: 100,
    max: 900,
}];

const EXPECTED_FAMILIES: [ExpectedFamily; 3] = [
    ExpectedFamily {
        key: "instrument-sans",
        name: "Instrument Sans",
        role: "body",
        license_file: "InstrumentSans-OFL.txt",
        source: "https://registry.npmjs.org/@fontsource-variable/instrument-sans/-/instrument-sans-5.3.0.tgz",
        version: "5.3.0",
        integrity: "sha512-u4gKbDBTNFGkg997tfQn3eHOhHuquWUFTRT/rwzuKtrxX5P2ekfs2x+LgBPP4P32+cC+vUwF1Cr+IdRoPQbrGw==",
        package: Some("@fontsource-variable/instrument-sans"),
        commit: None,
        faces: &INSTRUMENT_SANS_FACES,
    },
    ExpectedFamily {
        key: "newsreader",
        name: "Newsreader",
        role: "body",
        license_file: "Newsreader-OFL.txt",
        source: "https://registry.npmjs.org/@fontsource-variable/newsreader/-/newsreader-5.3.0.tgz",
        version: "5.3.0",
        integrity: "sha512-rrzYi43qMpbzwuFtf9OkWH8sxAPVPcQQQEwXpPtwaKYeJ8yVg5aLs5kawmo1f2Q1t1M38TLmEKCkGVDsYwgdFw==",
        package: Some("@fontsource-variable/newsreader"),
        commit: None,
        faces: &NEWSREADER_FACES,
    },
    ExpectedFamily {
        key: "geist-mono",
        name: "Geist Mono",
        role: "mono",
        license_file: "GeistMono-OFL.txt",
        source: "https://raw.githubusercontent.com/vercel/geist-font/v1.7.1/fonts/GeistMono/webfonts/GeistMono%5Bwght%5D.woff2",
        version: "v1.7.1",
        integrity: "commit:8b8b75fa63e339db10a3cd52fb28536615b5cc63",
        package: None,
        commit: Some("8b8b75fa63e339db10a3cd52fb28536615b5cc63"),
        faces: &GEIST_MONO_FACES,
    },
];

/// A validated, byte-verified font catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub presets: Vec<Preset>,
    pub families: Vec<Family>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub body: Option<String>,
    pub mono: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Family {
    pub key: String,
    pub name: String,
    pub role: String,
    pub license: String,
    pub license_file: String,
    pub notice: String,
    pub faces: Vec<Face>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub name: String,
    pub style: String,
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    pub axes: WghtRange,
    pub license: String,
    pub license_file: String,
    pub notice: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WghtRange {
    pub min: u64,
    pub max: u64,
}

/// One face the current document needs, in selection order: body normal,
/// body italic (emphasis only), mono normal (code only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedFace {
    pub family: String,
    pub style: String,
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
    pub axes: WghtRange,
    pub license_file: String,
    pub notice: String,
}

/// Load and verify the catalog at `catalog_path`, resolving every declared
/// face and license file relative to its parent directory. Any schema, size,
/// hash, or magic violation fails the load closed.
pub fn load(catalog_path: &Path) -> Result<Catalog, SelectionError> {
    let mut problems = Vec::new();
    let source = match fs::read_to_string(catalog_path) {
        Ok(source) => source,
        Err(error) => {
            problems.push(problem(
                "E-FONTS-01",
                format!("catalog.json is unreadable: {error}"),
            ));
            return Err(SelectionError { problems });
        }
    };
    if source.to_ascii_lowercase().contains("opsz") {
        problems.push(problem("E-FONTS-02", "catalog declares opsz"));
    }
    let value = match json::parse(&source) {
        Ok(value) => value,
        Err(error) => {
            problems.push(problem(
                "E-FONTS-01",
                format!(
                    "catalog.json is not valid JSON: {} (line {}, column {})",
                    error.message, error.line, error.column
                ),
            ));
            return Err(SelectionError { problems });
        }
    };
    let catalog = validate_schema(&value, &mut problems);
    if !problems.is_empty() {
        return Err(SelectionError { problems });
    }
    let catalog = catalog.expect("schema-clean catalogs always produce a Catalog");
    verify_bytes(catalog_path, &catalog, &mut problems);
    if problems.is_empty() {
        Ok(catalog)
    } else {
        Err(SelectionError { problems })
    }
}

/// Select the built-in faces a document needs (SPEC §18). `system` selects
/// no file and `fonts: map` selects no built-in faces; `auto` resolves to
/// `editorial` only for the editorial theme and `technical` otherwise.
pub fn select_faces(analysis: &Analysis, body: &str, catalog: &Catalog) -> Vec<SelectedFace> {
    let preset_name = match &analysis.config.fonts {
        Fonts::Auto => match &analysis.config.theme {
            Theme::Editorial => "editorial",
            _ => "technical",
        },
        Fonts::System => "system",
        Fonts::Map { .. } => return Vec::new(),
    };
    let preset = catalog
        .presets
        .iter()
        .find(|preset| preset.name == preset_name)
        .expect("validated catalogs contain every preset");
    let Some(body_family) = &preset.body else {
        return Vec::new();
    };
    let evidence = scan_document(body);
    let mut faces = Vec::new();
    faces.push(select_face(catalog, body_family, "normal"));
    if evidence.has_emphasis {
        faces.push(select_face(catalog, body_family, "italic"));
    }
    if evidence.has_code {
        if let Some(mono_family) = &preset.mono {
            faces.push(select_face(catalog, mono_family, "normal"));
        }
    }
    faces
}

fn select_face(catalog: &Catalog, family_key: &str, face_name: &str) -> SelectedFace {
    let family = catalog
        .families
        .iter()
        .find(|family| family.key == family_key)
        .expect("validated catalogs contain every preset family");
    let face = family
        .faces
        .iter()
        .find(|face| face.name == face_name)
        .expect("validated catalogs contain every family face");
    SelectedFace {
        family: family.key.clone(),
        style: face.style.clone(),
        file: face.file.clone(),
        bytes: face.bytes,
        sha256: face.sha256.clone(),
        axes: face.axes,
        license_file: face.license_file.clone(),
        notice: face.notice.clone(),
    }
}

fn problem(code: &'static str, message: impl Into<String>) -> SelectionProblem {
    SelectionProblem {
        code,
        message: message.into(),
    }
}

fn validate_schema(value: &JsonValue, problems: &mut Vec<SelectionProblem>) -> Option<Catalog> {
    let object = match value {
        JsonValue::Object(_) => value,
        _ => {
            problems.push(problem("E-FONTS-02", "catalog.json must be a JSON object"));
            return None;
        }
    };
    if !same_keys(object, &TOP_KEYS) {
        problems.push(problem(
            "E-FONTS-02",
            format!(
                "catalog top-level keys are not closed: {}",
                unknown_keys(object, &TOP_KEYS).join(", ")
            ),
        ));
    }
    match object_get(object, "format") {
        Some(JsonValue::String(format)) if format == FORMAT => {}
        Some(JsonValue::String(format)) => problems.push(problem(
            "E-FONTS-02",
            format!("catalog format must be \"{FORMAT}\", got \"{format}\""),
        )),
        Some(_) => problems.push(problem("E-FONTS-02", "'format' must be a string")),
        None => problems.push(problem("E-FONTS-02", "catalog is missing 'format'")),
    }
    let presets = match object_get(object, "presets") {
        Some(presets) => validate_presets(presets, problems),
        None => {
            problems.push(problem("E-FONTS-02", "catalog presets are invalid"));
            Vec::new()
        }
    };
    let families = match object_get(object, "families") {
        Some(families) => validate_families(families, problems),
        None => {
            problems.push(problem("E-FONTS-02", "catalog families are invalid"));
            Vec::new()
        }
    };
    if problems.is_empty() {
        Some(Catalog { presets, families })
    } else {
        None
    }
}

fn validate_presets(value: &JsonValue, problems: &mut Vec<SelectionProblem>) -> Vec<Preset> {
    if !presets_match(value) {
        problems.push(problem("E-FONTS-02", "catalog presets are invalid"));
    }
    build_presets(value)
}

fn presets_match(value: &JsonValue) -> bool {
    if !same_keys(value, &["editorial", "system", "technical"]) {
        return false;
    }
    preset_matches(
        object_get(value, "technical"),
        Some("instrument-sans"),
        Some("geist-mono"),
    ) && preset_matches(
        object_get(value, "editorial"),
        Some("newsreader"),
        Some("geist-mono"),
    ) && preset_matches(object_get(value, "system"), None, None)
}

fn preset_matches(value: Option<&JsonValue>, body: Option<&str>, mono: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    if !same_keys(value, &["body", "mono"]) {
        return false;
    }
    let actual_body = match object_get(value, "body") {
        Some(JsonValue::String(actual)) => Some(actual.as_str()),
        Some(JsonValue::Null) => None,
        _ => return false,
    };
    let actual_mono = match object_get(value, "mono") {
        Some(JsonValue::String(actual)) => Some(actual.as_str()),
        Some(JsonValue::Null) => None,
        _ => return false,
    };
    actual_body == body && actual_mono == mono
}

fn build_presets(value: &JsonValue) -> Vec<Preset> {
    match value {
        JsonValue::Object(entries) => entries
            .iter()
            .map(|(name, entry)| Preset {
                name: name.clone(),
                body: string_or_null(entry, "body"),
                mono: string_or_null(entry, "mono"),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn validate_families(value: &JsonValue, problems: &mut Vec<SelectionProblem>) -> Vec<Family> {
    let mut families = Vec::new();
    if !same_keys(value, &["geist-mono", "instrument-sans", "newsreader"]) {
        problems.push(problem("E-FONTS-02", "catalog families are invalid"));
    }
    for expected in &EXPECTED_FAMILIES {
        let Some(family_value) = object_get(value, expected.key) else {
            problems.push(problem(
                "E-FONTS-02",
                format!("{}: family is missing", expected.key),
            ));
            continue;
        };
        if !same_keys(family_value, &FAMILY_KEYS) {
            problems.push(problem(
                "E-FONTS-02",
                format!("{}: family keys are not closed", expected.key),
            ));
        }
        require_string_eq(
            family_value,
            "name",
            expected.name,
            format!("{}: name is invalid", expected.key),
            problems,
        );
        require_string_eq(
            family_value,
            "role",
            expected.role,
            format!("{}: role is invalid", expected.key),
            problems,
        );
        require_string_eq(
            family_value,
            "license",
            "OFL-1.1",
            format!("{}: license is invalid", expected.key),
            problems,
        );
        require_string_eq(
            family_value,
            "licenseFile",
            expected.license_file,
            format!("{}: license file is invalid", expected.key),
            problems,
        );
        require_string_eq(
            family_value,
            "notice",
            "NOTICE.md",
            format!("{}: notice is invalid", expected.key),
            problems,
        );
        validate_provenance(family_value, expected, problems);
        let faces = match object_get(family_value, "faces") {
            Some(faces) => validate_faces(faces, expected, family_value, problems),
            None => {
                problems.push(problem(
                    "E-FONTS-02",
                    format!("{}: faces are not closed", expected.key),
                ));
                Vec::new()
            }
        };
        families.push(Family {
            key: expected.key.to_string(),
            name: expected.name.to_string(),
            role: expected.role.to_string(),
            license: "OFL-1.1".to_string(),
            license_file: expected.license_file.to_string(),
            notice: "NOTICE.md".to_string(),
            faces,
        });
    }
    families
}

fn validate_provenance(
    value: &JsonValue,
    expected: &ExpectedFamily,
    problems: &mut Vec<SelectionProblem>,
) {
    let key = expected.key;
    let Some(provenance) = object_get(value, "provenance") else {
        problems.push(problem(
            "E-FONTS-02",
            format!("{key}: provenance keys are not closed"),
        ));
        return;
    };
    let expected_keys: &[&str] = if expected.commit.is_some() {
        &PROVENANCE_KEYS
    } else {
        &PROVENANCE_PACKAGE_KEYS
    };
    if !same_keys(provenance, expected_keys) {
        problems.push(problem(
            "E-FONTS-02",
            format!("{key}: provenance keys are not closed"),
        ));
    }
    require_string_eq(
        provenance,
        "source",
        expected.source,
        format!("{key}: source is invalid"),
        problems,
    );
    require_string_eq(
        provenance,
        "version",
        expected.version,
        format!("{key}: version is invalid"),
        problems,
    );
    require_string_eq(
        provenance,
        "integrity",
        expected.integrity,
        format!("{key}: integrity is invalid"),
        problems,
    );
    if let Some(commit) = expected.commit {
        require_string_eq(
            provenance,
            "commit",
            commit,
            format!("{key}: commit is invalid"),
            problems,
        );
    }
    if let Some(package) = expected.package {
        require_string_eq(
            provenance,
            "package",
            package,
            format!("{key}: package is invalid"),
            problems,
        );
    }
}

fn validate_faces(
    value: &JsonValue,
    expected: &ExpectedFamily,
    family_value: &JsonValue,
    problems: &mut Vec<SelectionProblem>,
) -> Vec<Face> {
    let expected_face_keys: Vec<&str> = expected.faces.iter().map(|face| face.name).collect();
    if !same_keys(value, &expected_face_keys) {
        problems.push(problem(
            "E-FONTS-02",
            format!("{}: faces are not closed", expected.key),
        ));
    }
    let mut faces = Vec::new();
    for expected_face in expected.faces {
        let prefix = format!("{}/{}", expected.key, expected_face.name);
        let Some(face_value) = object_get(value, expected_face.name) else {
            problems.push(problem("E-FONTS-02", format!("{prefix}: face is missing")));
            continue;
        };
        if !same_keys(face_value, &FACE_KEYS) {
            problems.push(problem(
                "E-FONTS-02",
                format!("{prefix}: face keys are not closed"),
            ));
        }
        let style = require_string_eq(
            face_value,
            "style",
            expected_face.style,
            format!("{prefix}: style is invalid"),
            problems,
        );
        let axes = validate_axes(face_value, expected_face, &prefix, problems);
        let file = require_file(
            face_value,
            "file",
            format!("{prefix}: file is invalid"),
            problems,
        );
        let bytes = require_byte_count(
            face_value,
            "bytes",
            format!("{prefix}: byte count must be a non-negative integer"),
            problems,
        );
        let sha256 = require_sha256(
            face_value,
            "sha256",
            format!("{prefix}: sha256 must be 64 hex characters"),
            problems,
        );
        require_string_eq(
            face_value,
            "license",
            "OFL-1.1",
            format!("{prefix}: license is invalid"),
            problems,
        );
        require_string_eq(
            face_value,
            "licenseFile",
            expected.license_file,
            format!("{prefix}: license file is invalid"),
            problems,
        );
        require_string_eq(
            face_value,
            "notice",
            "NOTICE.md",
            format!("{prefix}: notice is invalid"),
            problems,
        );
        face_provenance_matches(face_value, family_value, &prefix, problems);
        if let (Some(style), Some(axes), Some(file), Some(bytes), Some(sha256)) =
            (style, axes, file, bytes, sha256)
        {
            faces.push(Face {
                name: expected_face.name.to_string(),
                style,
                file,
                bytes,
                sha256,
                axes,
                license: "OFL-1.1".to_string(),
                license_file: expected.license_file.to_string(),
                notice: "NOTICE.md".to_string(),
            });
        }
    }
    faces
}

fn validate_axes(
    face_value: &JsonValue,
    expected: &ExpectedFace,
    prefix: &str,
    problems: &mut Vec<SelectionProblem>,
) -> Option<WghtRange> {
    let axes = match object_get(face_value, "axes") {
        Some(axes) => axes,
        None => {
            problems.push(problem("E-FONTS-02", format!("{prefix}: axes are invalid")));
            return None;
        }
    };
    let wght = match object_get(axes, "wght") {
        Some(wght) => wght,
        None => {
            problems.push(problem("E-FONTS-02", format!("{prefix}: axes are invalid")));
            return None;
        }
    };
    if !same_keys(axes, &["wght"]) || !same_keys(wght, &["min", "max"]) {
        problems.push(problem("E-FONTS-02", format!("{prefix}: axes are invalid")));
        return None;
    }
    let min = match object_get(wght, "min") {
        Some(JsonValue::Number(value))
            if value.fract() == 0.0 && *value >= 0.0 && *value as u64 == expected.min =>
        {
            *value as u64
        }
        _ => {
            problems.push(problem("E-FONTS-02", format!("{prefix}: axes are invalid")));
            return None;
        }
    };
    let max = match object_get(wght, "max") {
        Some(JsonValue::Number(value))
            if value.fract() == 0.0 && *value >= 0.0 && *value as u64 == expected.max =>
        {
            *value as u64
        }
        _ => {
            problems.push(problem("E-FONTS-02", format!("{prefix}: axes are invalid")));
            return None;
        }
    };
    Some(WghtRange { min, max })
}

fn face_provenance_matches(
    face_value: &JsonValue,
    family_value: &JsonValue,
    prefix: &str,
    problems: &mut Vec<SelectionProblem>,
) {
    match (
        object_get(face_value, "provenance"),
        object_get(family_value, "provenance"),
    ) {
        (Some(face_provenance), Some(family_provenance))
            if face_provenance == family_provenance => {}
        _ => problems.push(problem(
            "E-FONTS-02",
            format!("{prefix}: provenance is invalid"),
        )),
    }
}

fn require_string_eq(
    value: &JsonValue,
    key: &str,
    expected: &str,
    message: String,
    problems: &mut Vec<SelectionProblem>,
) -> Option<String> {
    match object_get(value, key) {
        Some(JsonValue::String(actual)) if actual == expected => Some(actual.clone()),
        _ => {
            problems.push(problem("E-FONTS-02", message));
            None
        }
    }
}

fn require_file(
    value: &JsonValue,
    key: &str,
    message: String,
    problems: &mut Vec<SelectionProblem>,
) -> Option<String> {
    match object_get(value, key) {
        Some(JsonValue::String(actual)) if !actual.is_empty() => Some(actual.clone()),
        _ => {
            problems.push(problem("E-FONTS-02", message));
            None
        }
    }
}

fn require_byte_count(
    value: &JsonValue,
    key: &str,
    message: String,
    problems: &mut Vec<SelectionProblem>,
) -> Option<u64> {
    match object_get(value, key) {
        Some(JsonValue::Number(actual)) if actual.fract() == 0.0 && *actual >= 0.0 => {
            Some(*actual as u64)
        }
        _ => {
            problems.push(problem("E-FONTS-02", message));
            None
        }
    }
}

fn require_sha256(
    value: &JsonValue,
    key: &str,
    message: String,
    problems: &mut Vec<SelectionProblem>,
) -> Option<String> {
    match object_get(value, key) {
        Some(JsonValue::String(actual)) if is_hex_64(actual) => Some(actual.clone()),
        _ => {
            problems.push(problem("E-FONTS-02", message));
            None
        }
    }
}

fn is_hex_64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn string_or_null(value: &JsonValue, key: &str) -> Option<String> {
    match object_get(value, key) {
        Some(JsonValue::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn verify_bytes(catalog_path: &Path, catalog: &Catalog, problems: &mut Vec<SelectionProblem>) {
    let fonts_dir = catalog_path.parent().unwrap_or_else(|| Path::new("."));
    for family in &catalog.families {
        let license_path = fonts_dir.join(&family.license_file);
        match fs::read(&license_path) {
            Ok(bytes) => {
                let actual = sha256::digest_hex(&bytes);
                let expected = expected_license_hash(&family.license_file)
                    .expect("validated families always use an accepted license file");
                if actual != expected {
                    problems.push(problem(
                        "E-FONTS-05",
                        format!(
                            "{}: license hash mismatch: declared {}, actual {}",
                            family.key, expected, actual
                        ),
                    ));
                }
            }
            Err(error) => problems.push(problem(
                "E-FONTS-03",
                format!(
                    "license file '{}' is unreadable: {error}",
                    family.license_file
                ),
            )),
        }
        for face in &family.faces {
            let face_path = fonts_dir.join(&face.file);
            match fs::read(&face_path) {
                Ok(bytes) => {
                    if bytes.len() as u64 != face.bytes {
                        problems.push(problem(
                            "E-FONTS-04",
                            format!(
                                "{}/{}: size mismatch: declared {}, actual {}",
                                family.key,
                                face.name,
                                face.bytes,
                                bytes.len()
                            ),
                        ));
                    }
                    let actual_hash = sha256::digest_hex(&bytes);
                    if actual_hash != face.sha256 {
                        problems.push(problem(
                            "E-FONTS-05",
                            format!(
                                "{}/{}: sha256 mismatch: declared {}, actual {}",
                                family.key, face.name, face.sha256, actual_hash
                            ),
                        ));
                    }
                    if !bytes.starts_with(b"wOF2") {
                        problems.push(problem(
                            "E-FONTS-06",
                            format!("{}/{}: missing WOFF2 magic", family.key, face.name),
                        ));
                    }
                }
                Err(error) => problems.push(problem(
                    "E-FONTS-03",
                    format!(
                        "face '{}' file '{}' is unreadable: {error}",
                        face.name, face.file
                    ),
                )),
            }
        }
    }
}

fn expected_license_hash(license_file: &str) -> Option<&'static str> {
    LICENSE_HASHES
        .iter()
        .find(|(file, _)| *file == license_file)
        .map(|(_, hash)| *hash)
}

fn unknown_keys(value: &JsonValue, known: &[&str]) -> Vec<String> {
    match value {
        JsonValue::Object(entries) => entries
            .iter()
            .map(|(key, _)| key.clone())
            .filter(|key| !known.contains(&key.as_str()))
            .collect(),
        _ => Vec::new(),
    }
}

fn same_keys(value: &JsonValue, expected: &[&str]) -> bool {
    let mut actual: Vec<&str> = match value {
        JsonValue::Object(entries) => entries.iter().map(|(key, _)| key.as_str()).collect(),
        _ => Vec::new(),
    };
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    actual == expected
}

fn object_get<'a>(value: &'a JsonValue, key: &str) -> Option<&'a JsonValue> {
    match value {
        JsonValue::Object(entries) => entries
            .iter()
            .find(|(existing, _)| existing == key)
            .map(|(_, value)| value),
        _ => None,
    }
}
