use mdhtml::analysis::{Analysis, NormalizedConfig, Toc, TocPosition, TocSetting};
use mdhtml::selection::{FragmentId, SelectionError, load, select_fragments};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

fn runtime_dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../runtime/dist")
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Staging root under the cargo target directory, never inside the committed
/// fixtures tree: the invalid-manifest tests write their shared manifest.json
/// here, and the whole tree is removed on drop even when an assertion unwinds.
fn staging_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.runs/cargo-t11c/staging")
}

fn analysis_with_toc(toc: TocSetting) -> Analysis {
    Analysis {
        config: NormalizedConfig {
            toc,
            ..NormalizedConfig::default()
        },
        sections: Vec::new(),
        bindings: Vec::new(),
        degraded: Vec::new(),
        diagnostics: Vec::new(),
    }
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

/// Owns the staged manifest tree plus the staging lock. Dropping removes the
/// whole staging directory even when an assertion unwinds, and the lock is
/// held until then so the two invalid-manifest tests never race on the shared
/// staging path (cargo runs tests in parallel).
struct StagedManifest {
    root: PathBuf,
    error: SelectionError,
    _lock: MutexGuard<'static, ()>,
}

impl Drop for StagedManifest {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Serializes staging of the shared `manifest.json`: both invalid-manifest
/// tests use the same staging path, and cargo runs tests in parallel.
static STAGING_LOCK: Mutex<()> = Mutex::new(());

fn stage_invalid_fixture(name: &str) -> StagedManifest {
    let lock = STAGING_LOCK.lock().expect("staging lock");
    let root = staging_root();
    let dir = root.join("fixtures");
    fs::create_dir_all(&dir).expect("staging dir created");

    // The invalid-size fixture references `../runtime/dist/*.min.js` relative
    // to the manifest dir; mirror the real fragment files so the byte checks
    // run against real bytes.
    let mirror = root.join("runtime/dist");
    fs::create_dir_all(&mirror).expect("staging mirror created");
    for entry in fs::read_dir(runtime_dist()).expect("runtime/dist readable") {
        let entry = entry.expect("runtime/dist entry readable");
        let file_name = entry.file_name();
        if file_name.to_string_lossy().ends_with(".min.js") {
            fs::copy(entry.path(), mirror.join(&file_name)).expect("fragment mirrored");
        }
    }

    let source = fs::read_to_string(fixtures_dir().join(name)).expect("fixture is readable");
    fs::write(dir.join("manifest.json"), source).expect("staged manifest.json");
    let error = load(&dir).expect_err("invalid manifest must fail closed");
    StagedManifest {
        root,
        error,
        _lock: lock,
    }
}

#[test]
fn load_committed_manifest_verifies_ids_sizes_hashes_and_concatenation() {
    let manifest = load(&runtime_dist()).expect("committed manifest loads clean");

    let ids: Vec<FragmentId> = manifest
        .fragments
        .iter()
        .map(|fragment| fragment.id)
        .collect();
    assert_eq!(
        ids,
        [
            FragmentId::Core,
            FragmentId::Copy,
            FragmentId::Toc,
            FragmentId::Lightbox
        ]
    );

    let mut concat = Vec::new();
    for fragment in &manifest.fragments {
        let path = runtime_dist().join(&fragment.file);
        let bytes = fs::read(&path).expect("fragment file is readable");
        assert_eq!(bytes.len() as u64, fragment.size, "{}", fragment.file);
        assert_eq!(fragment.sha256.len(), 64, "{}", fragment.file);
        concat.extend_from_slice(&bytes);
    }

    assert_eq!(manifest.fragments[0].requires, Vec::<FragmentId>::new());
    for fragment in &manifest.fragments[1..] {
        assert_eq!(fragment.requires, [FragmentId::Core]);
    }

    let runtime = fs::read(runtime_dist().join("runtime.min.js")).expect("runtime.min.js readable");
    assert_eq!(
        runtime, concat,
        "runtime.min.js is the fragment concatenation"
    );
}

#[test]
fn select_fragments_always_include_core_and_copy() {
    let manifest = load(&runtime_dist()).expect("committed manifest loads");
    let analysis = analysis_with_toc(TocSetting::Enabled(Toc {
        depth: 3,
        position: TocPosition::Side,
    }));

    assert_eq!(
        select_fragments(
            "plain prose with no headings or images",
            &analysis,
            &manifest
        ),
        [FragmentId::Core, FragmentId::Copy]
    );
}

#[test]
fn select_fragments_adds_toc_when_heading_is_within_normalized_depth() {
    let manifest = load(&runtime_dist()).expect("committed manifest loads");
    let analysis = analysis_with_toc(TocSetting::Enabled(Toc {
        depth: 3,
        position: TocPosition::Side,
    }));

    assert_eq!(
        select_fragments("# Title", &analysis, &manifest),
        [FragmentId::Core, FragmentId::Copy, FragmentId::Toc]
    );
    assert_eq!(
        select_fragments(
            "### Deep heading at the depth boundary",
            &analysis,
            &manifest
        ),
        [FragmentId::Core, FragmentId::Copy, FragmentId::Toc]
    );
}

#[test]
fn select_fragments_omits_toc_when_heading_is_beyond_normalized_depth() {
    let manifest = load(&runtime_dist()).expect("committed manifest loads");
    let analysis = analysis_with_toc(TocSetting::Enabled(Toc {
        depth: 2,
        position: TocPosition::Side,
    }));

    assert_eq!(
        select_fragments("### Too deep", &analysis, &manifest),
        [FragmentId::Core, FragmentId::Copy]
    );
}

#[test]
fn select_fragments_omits_toc_when_disabled() {
    let manifest = load(&runtime_dist()).expect("committed manifest loads");
    let analysis = analysis_with_toc(TocSetting::Disabled);

    assert_eq!(
        select_fragments("# Title", &analysis, &manifest),
        [FragmentId::Core, FragmentId::Copy]
    );
}

#[test]
fn select_fragments_adds_lightbox_when_document_has_images() {
    let manifest = load(&runtime_dist()).expect("committed manifest loads");
    let analysis = analysis_with_toc(TocSetting::Enabled(Toc {
        depth: 3,
        position: TocPosition::Side,
    }));

    assert_eq!(
        select_fragments("![alt](images/photo.png)", &analysis, &manifest),
        [FragmentId::Core, FragmentId::Copy, FragmentId::Lightbox]
    );
}

#[test]
fn select_fragments_follows_manifest_order() {
    let manifest = load(&runtime_dist()).expect("committed manifest loads");
    let analysis = analysis_with_toc(TocSetting::Enabled(Toc {
        depth: 3,
        position: TocPosition::Side,
    }));

    assert_eq!(
        select_fragments("# Title\n\n![alt](images/photo.png)", &analysis, &manifest),
        [
            FragmentId::Core,
            FragmentId::Copy,
            FragmentId::Toc,
            FragmentId::Lightbox
        ]
    );
}

#[test]
fn invalid_schema_manifest_reports_every_schema_problem() {
    let staged = stage_invalid_fixture("manifest-invalid-schema.json");
    let error = &staged.error;
    let codes = problem_codes(&error);
    assert!(
        codes.iter().all(|code| *code == "E-MANIFEST-02"),
        "schema problems only: {codes:?}"
    );

    let messages = problem_messages(&error);
    assert!(messages.iter().any(|m| m.contains("format must be")));
    assert!(messages.iter().any(|m| m.contains("sha256 must be 64 hex")));
    assert!(messages.iter().any(|m| m.contains("must appear earlier")));
    assert!(
        messages
            .iter()
            .any(|m| m.contains("unknown key 'minified'"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("unknown id 'highlight'"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("missing fragment 'toc'"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("missing fragment 'lightbox'"))
    );
    assert_eq!(
        error.problems.len(),
        7,
        "every schema problem is an entry: {messages:?}"
    );
}

#[test]
fn invalid_size_manifest_reports_size_and_hash_mismatches() {
    let staged = stage_invalid_fixture("manifest-invalid-size.json");
    let error = &staged.error;

    let codes = problem_codes(&error);
    assert!(
        codes.contains(&"E-MANIFEST-04"),
        "size mismatch reported: {codes:?}"
    );
    assert!(
        codes.contains(&"E-MANIFEST-05"),
        "hash mismatch reported: {codes:?}"
    );
    assert!(
        !codes.contains(&"E-MANIFEST-06"),
        "concatenation still matches with real bytes: {codes:?}"
    );

    let messages = problem_messages(&error);
    assert!(messages.iter().any(|m| m.contains("'core' size mismatch")));
    assert!(
        messages
            .iter()
            .any(|m| m.contains("'copy' sha256 mismatch"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("runtime.min.js is unreadable")),
        "the committed runtime.min.js is not reachable from the fixtures dir: {messages:?}"
    );
    assert_eq!(
        error.problems.len(),
        3,
        "every byte problem is an entry: {messages:?}"
    );
}
