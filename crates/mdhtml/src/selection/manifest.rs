//! Strict runtime fragment manifest loading (SPEC §17) and deterministic
//! fragment selection from accepted analysis/scanner evidence.
//!
//! `load` fails closed: every schema, size, hash, and concatenation violation
//! becomes an entry in `SelectionError`, and byte verification runs only over
//! a schema-clean manifest. `runtime.min.js` must be the byte-for-byte
//! concatenation of every fragment file in manifest order.

use std::fmt;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use crate::analysis::{Analysis, Toc, TocSetting};
use crate::scanner::scan_document;

use super::json::{self, JsonValue};
use super::sha256;

const FORMAT: &str = "mdhtml/manifest/1.0";
const FRAGMENT_KEYS: [&str; 5] = ["id", "file", "size", "sha256", "requires"];

/// The closed executable fragment id set (SPEC §17), in manifest order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FragmentId {
    Core,
    Copy,
    Toc,
    Lightbox,
}

impl FragmentId {
    pub const ALL: [FragmentId; 4] = [
        FragmentId::Core,
        FragmentId::Copy,
        FragmentId::Toc,
        FragmentId::Lightbox,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FragmentId::Core => "core",
            FragmentId::Copy => "copy",
            FragmentId::Toc => "toc",
            FragmentId::Lightbox => "lightbox",
        }
    }
}

impl fmt::Display for FragmentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for FragmentId {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "core" => Ok(FragmentId::Core),
            "copy" => Ok(FragmentId::Copy),
            "toc" => Ok(FragmentId::Toc),
            "lightbox" => Ok(FragmentId::Lightbox),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragment {
    pub id: FragmentId,
    pub file: String,
    pub size: u64,
    pub sha256: String,
    pub requires: Vec<FragmentId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub fragments: Vec<Fragment>,
}

/// One violation found while loading a manifest. `code` is a stable
/// machine-readable record; `message` is a one-line human description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionProblem {
    pub code: &'static str,
    pub message: String,
}

impl SelectionProblem {
    fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Every problem found during a failed `load`. Byte verification runs only
/// when the manifest is schema-clean, so this never mixes schema and I/O
/// classes on the same load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionError {
    pub problems: Vec<SelectionProblem>,
}

/// Load and verify `manifest_dir/manifest.json` plus every referenced
/// fragment file and `runtime.min.js`, failing closed on any violation.
pub fn load(manifest_dir: &Path) -> Result<Manifest, SelectionError> {
    let mut problems = Vec::new();
    let manifest_path = manifest_dir.join("manifest.json");
    let source = match fs::read_to_string(&manifest_path) {
        Ok(source) => source,
        Err(error) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-01",
                format!("manifest.json is unreadable: {error}"),
            ));
            return Err(SelectionError { problems });
        }
    };
    let value = match json::parse(&source) {
        Ok(value) => value,
        Err(error) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-01",
                format!(
                    "manifest.json is not valid JSON: {} (line {}, column {})",
                    error.message, error.line, error.column
                ),
            ));
            return Err(SelectionError { problems });
        }
    };

    let fragments = validate_schema(&value, &mut problems);
    if !problems.is_empty() {
        return Err(SelectionError { problems });
    }
    let fragments = fragments.expect("schema-clean manifests always produce fragments");

    let mut fragment_bytes = Vec::with_capacity(fragments.len());
    for fragment in &fragments {
        let path = manifest_dir.join(&fragment.file);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                problems.push(SelectionProblem::error(
                    "E-MANIFEST-03",
                    format!(
                        "fragment '{}' file '{}' is unreadable: {error}",
                        fragment.id, fragment.file
                    ),
                ));
                continue;
            }
        };
        if bytes.len() as u64 != fragment.size {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-04",
                format!(
                    "fragment '{}' size mismatch: declared {}, actual {}",
                    fragment.id,
                    fragment.size,
                    bytes.len()
                ),
            ));
        }
        let actual_hash = sha256::digest_hex(&bytes);
        if actual_hash != fragment.sha256 {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-05",
                format!(
                    "fragment '{}' sha256 mismatch: declared {}, actual {}",
                    fragment.id, fragment.sha256, actual_hash
                ),
            ));
        }
        fragment_bytes.push(bytes);
    }

    if fragment_bytes.len() == fragments.len() {
        let runtime_path = manifest_dir.join("runtime.min.js");
        match fs::read(&runtime_path) {
            Ok(bytes) => {
                let mut concatenated = Vec::new();
                for bytes in &fragment_bytes {
                    concatenated.extend_from_slice(bytes);
                }
                if bytes != concatenated {
                    problems.push(SelectionProblem::error(
                        "E-MANIFEST-06",
                        "runtime.min.js is not the byte-for-byte concatenation of \
                         the fragment files in manifest order",
                    ));
                }
            }
            Err(error) => problems.push(SelectionProblem::error(
                "E-MANIFEST-03",
                format!("runtime.min.js is unreadable: {error}"),
            )),
        }
    }

    if problems.is_empty() {
        Ok(Manifest { fragments })
    } else {
        Err(SelectionError { problems })
    }
}

/// Select the fragment ids a document needs: `core` and `copy` always, `toc`
/// when the config enables it and at least one heading is within the
/// normalized depth, `lightbox` when the body contains an image. The result
/// keeps manifest order, filtered to the selection.
pub fn select_fragments(body: &str, analysis: &Analysis, manifest: &Manifest) -> Vec<FragmentId> {
    let evidence = scan_document(body);
    let wants_toc = match analysis.config.toc {
        TocSetting::Enabled(Toc { depth, .. }) => evidence
            .headings
            .iter()
            .any(|heading| heading.level <= depth),
        TocSetting::Disabled => false,
    };
    let wants_lightbox = !evidence.images.is_empty();
    manifest
        .fragments
        .iter()
        .filter(|fragment| match fragment.id {
            FragmentId::Core | FragmentId::Copy => true,
            FragmentId::Toc => wants_toc,
            FragmentId::Lightbox => wants_lightbox,
        })
        .map(|fragment| fragment.id)
        .collect()
}

fn validate_schema(
    value: &JsonValue,
    problems: &mut Vec<SelectionProblem>,
) -> Option<Vec<Fragment>> {
    let object = match value {
        JsonValue::Object(_) => value,
        _ => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                "manifest.json must be a JSON object",
            ));
            return None;
        }
    };

    validate_format(object, problems);

    let fragments = match object_get(object, "fragments") {
        Some(JsonValue::Array(fragments)) => fragments,
        Some(_) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                "'fragments' must be an array",
            ));
            return None;
        }
        None => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                "manifest is missing 'fragments'",
            ));
            return None;
        }
    };

    let mut validated = Vec::new();
    let mut seen: Vec<FragmentId> = Vec::new();
    for (index, fragment) in fragments.iter().enumerate() {
        let fragment_object = match fragment {
            JsonValue::Object(_) => fragment,
            _ => {
                problems.push(SelectionProblem::error(
                    "E-MANIFEST-02",
                    format!("fragments[{index}] must be an object"),
                ));
                continue;
            }
        };
        check_fragment_keys(fragment_object, index, problems);

        let id = match parse_fragment_id(fragment_object, index, problems) {
            Some(id) => id,
            None => continue,
        };
        if seen.contains(&id) {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}] duplicates fragment id '{id}'"),
            ));
        }
        seen.push(id);

        if let (Some(file), Some(size), Some(sha256), Some(requires)) = (
            parse_fragment_file(fragment_object, index, problems),
            parse_fragment_size(fragment_object, index, problems),
            parse_fragment_sha256(fragment_object, index, problems),
            parse_fragment_requires(fragment_object, index, &seen, problems),
        ) {
            validated.push(Fragment {
                id,
                file,
                size,
                sha256,
                requires,
            });
        }
    }

    for expected in FragmentId::ALL {
        if !seen.contains(&expected) {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("manifest is missing fragment '{expected}'"),
            ));
        }
    }

    Some(validated)
}

fn validate_format(object: &JsonValue, problems: &mut Vec<SelectionProblem>) {
    match object_get(object, "format") {
        Some(JsonValue::String(format)) if format == FORMAT => {}
        Some(JsonValue::String(format)) => problems.push(SelectionProblem::error(
            "E-MANIFEST-02",
            format!("manifest format must be \"{FORMAT}\", got \"{format}\""),
        )),
        Some(_) => problems.push(SelectionProblem::error(
            "E-MANIFEST-02",
            "'format' must be a string",
        )),
        None => problems.push(SelectionProblem::error(
            "E-MANIFEST-02",
            "manifest is missing 'format'",
        )),
    }
}

fn check_fragment_keys(
    fragment_object: &JsonValue,
    index: usize,
    problems: &mut Vec<SelectionProblem>,
) {
    if let JsonValue::Object(entries) = fragment_object {
        for (key, _) in entries {
            if !FRAGMENT_KEYS.contains(&key.as_str()) {
                problems.push(SelectionProblem::error(
                    "E-MANIFEST-02",
                    format!("fragments[{index}] has unknown key '{key}'"),
                ));
            }
        }
    }
}

fn parse_fragment_id(
    fragment_object: &JsonValue,
    index: usize,
    problems: &mut Vec<SelectionProblem>,
) -> Option<FragmentId> {
    match object_get(fragment_object, "id") {
        Some(JsonValue::String(id)) => match id.parse::<FragmentId>() {
            Ok(id) => Some(id),
            Err(()) => {
                problems.push(SelectionProblem::error(
                    "E-MANIFEST-02",
                    format!("fragments[{index}] has unknown id '{id}'"),
                ));
                None
            }
        },
        Some(_) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}].id must be a string"),
            ));
            None
        }
        None => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}] is missing 'id'"),
            ));
            None
        }
    }
}

fn parse_fragment_file(
    fragment_object: &JsonValue,
    index: usize,
    problems: &mut Vec<SelectionProblem>,
) -> Option<String> {
    match object_get(fragment_object, "file") {
        Some(JsonValue::String(value)) => Some(value.clone()),
        Some(_) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}].file must be a string"),
            ));
            None
        }
        None => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}] is missing 'file'"),
            ));
            None
        }
    }
}

fn parse_fragment_size(
    fragment_object: &JsonValue,
    index: usize,
    problems: &mut Vec<SelectionProblem>,
) -> Option<u64> {
    match object_get(fragment_object, "size") {
        Some(JsonValue::Number(value)) if value.fract() == 0.0 && *value >= 0.0 => {
            Some(*value as u64)
        }
        Some(JsonValue::Number(_)) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}].size must be a non-negative integer"),
            ));
            None
        }
        Some(_) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}].size must be a number"),
            ));
            None
        }
        None => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}] is missing 'size'"),
            ));
            None
        }
    }
}

fn parse_fragment_sha256(
    fragment_object: &JsonValue,
    index: usize,
    problems: &mut Vec<SelectionProblem>,
) -> Option<String> {
    match object_get(fragment_object, "sha256") {
        Some(JsonValue::String(value)) if is_hex_64(value) => {
            Some(value.to_ascii_lowercase())
        }
        Some(JsonValue::String(_)) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}].sha256 must be 64 hex characters"),
            ));
            None
        }
        Some(_) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}].sha256 must be a string"),
            ));
            None
        }
        None => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}] is missing 'sha256'"),
            ));
            None
        }
    }
}

fn parse_fragment_requires(
    fragment_object: &JsonValue,
    index: usize,
    seen: &[FragmentId],
    problems: &mut Vec<SelectionProblem>,
) -> Option<Vec<FragmentId>> {
    match object_get(fragment_object, "requires") {
        Some(JsonValue::Array(entries)) => {
            let mut required = Vec::new();
            for (entry_index, entry) in entries.iter().enumerate() {
                match entry {
                    JsonValue::String(name) => match name.parse::<FragmentId>() {
                        Ok(id) => {
                            if seen.contains(&id) {
                                required.push(id);
                            } else {
                                problems.push(SelectionProblem::error(
                                    "E-MANIFEST-02",
                                    format!(
                                        "fragments[{index}].requires[{entry_index}] \
                                         references '{id}', which must appear earlier \
                                         in manifest order"
                                    ),
                                ));
                            }
                        }
                        Err(()) => problems.push(SelectionProblem::error(
                            "E-MANIFEST-02",
                            format!(
                                "fragments[{index}].requires[{entry_index}] \
                                 references unknown id '{name}'"
                            ),
                        )),
                    },
                    _ => problems.push(SelectionProblem::error(
                        "E-MANIFEST-02",
                        format!("fragments[{index}].requires[{entry_index}] must be a string"),
                    )),
                }
            }
            Some(required)
        }
        Some(_) => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}].requires must be an array"),
            ));
            None
        }
        None => {
            problems.push(SelectionProblem::error(
                "E-MANIFEST-02",
                format!("fragments[{index}] is missing 'requires'"),
            ));
            None
        }
    }
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

fn is_hex_64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
