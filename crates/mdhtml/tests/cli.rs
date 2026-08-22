use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mdhtml"))
        .args(args)
        .output()
        .expect("mdhtml binary should run")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

#[test]
fn help_and_version_are_stdout_successes() {
    let help = run(&["--help"]);
    assert!(help.status.success());
    assert_eq!(String::from_utf8_lossy(&help.stderr), "");
    assert_eq!(
        String::from_utf8_lossy(&help.stdout),
        "Usage: mdhtml <command>\n\nmdhtml build <in.md> [-o out] [--watch] [--no-fonts]\nmdhtml check <file>\nmdhtml extract <in.md.html> [-o out.md] [--assets dir]\nmdhtml new <name> [--template resume|memo|spec|recipe|chapter]\nmdhtml themes\n"
    );

    let version = run(&["-V"]);
    assert!(version.status.success());
    assert_eq!(String::from_utf8_lossy(&version.stderr), "");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        format!("mdhtml {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn new_and_themes_commands_are_dispatched() {
    let themes = run(&["themes"]);
    assert!(
        themes.status.success(),
        "{}",
        String::from_utf8_lossy(&themes.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&themes.stderr), "");
    assert_eq!(
        String::from_utf8_lossy(&themes.stdout),
        "mdhtml: I-CLI-04: technical\nmdhtml: I-CLI-04: editorial\n"
    );

    let dir = std::env::temp_dir().join(format!("mdhtml-t15-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    let name = dir.join("draft.md");
    let created = run(&["new", name.to_str().expect("utf8 path")]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&created.stderr), "");
    assert!(name.exists(), "new materializes the default template");
}

#[test]
fn invalid_arguments_are_one_line_stderr_errors() {
    let cases = [
        &["build"] as &[&str],
        &["build", "--unknown", "input.md"] as &[&str],
        &["--watch"] as &[&str],
    ];
    for args in cases {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("mdhtml: E-CLI-05: "));
        assert_eq!(stderr.matches('\n').count(), 1);
    }
}

#[test]
fn invalid_arguments_escape_line_breaks() {
    let output = run(&["--bad\nvalue"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "mdhtml: E-CLI-05: unsupported top-level option --bad\\nvalue; use --help for usage\n"
    );
}

#[test]
fn check_command_prints_the_source_report_and_sets_the_exit_code() {
    let portable = run(&[
        "check",
        repo_root()
            .join("fixtures/check-portable.md")
            .to_str()
            .expect("utf8 path"),
    ]);
    assert!(
        portable.status.success(),
        "{}",
        String::from_utf8_lossy(&portable.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&portable.stderr), "");
    let stdout = String::from_utf8_lossy(&portable.stdout);
    assert!(
        stdout.starts_with("mdhtml: I-CLI-02: portable: true; requests: 0; "),
        "{stdout}"
    );
    assert_eq!(stdout.matches('\n').count(), 1);

    let report = run(&[
        "check",
        repo_root()
            .join("fixtures/check-report.md")
            .to_str()
            .expect("utf8 path"),
    ]);
    assert_eq!(report.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("mdhtml: E-SECT-01: sections key has no matching heading slug\n"));
    assert!(stdout.contains("mdhtml: W-COMP-02: unknown section component\n"));
    assert!(stdout.contains("mdhtml: W-COMP-02: unknown container name\n"));
    let last_line = stdout.lines().last().expect("report has a final line");
    assert!(
        last_line.starts_with("mdhtml: I-CLI-02: portable: true; requests: 0; "),
        "{stdout}"
    );
    assert_eq!(
        String::from_utf8_lossy(&report.stderr),
        "mdhtml: E-CLI-02: document failed check\n"
    );
}

#[test]
fn check_command_validates_a_built_artifact() {
    let dir = std::env::temp_dir().join(format!("mdhtml-t13-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    let input = dir.join("note.md");
    fs::write(&input, "---\ntitle: Note\n---\n# Note\n").expect("write input");

    let built = run(&["build", input.to_str().expect("utf8 path")]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let artifact = dir.join("note.md.html");
    let checked = run(&["check", artifact.to_str().expect("utf8 path")]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let stdout = String::from_utf8_lossy(&checked.stdout);
    assert!(
        stdout.starts_with("mdhtml: I-CLI-02: portable: true; requests: 0; "),
        "{stdout}"
    );
    assert_eq!(String::from_utf8_lossy(&checked.stderr), "");
}

#[test]
fn check_command_reports_unreadable_inputs_as_one_line_cli05_errors() {
    let output = run(&["check", "no-such-file.md"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("mdhtml: E-CLI-05: input no-such-file.md is unreadable:"),
        "{stderr}"
    );
    assert_eq!(stderr.matches('\n').count(), 1);
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    fs::read(repo_root().join("fixtures").join(name)).expect("read fixture")
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mdhtml-t14-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

#[test]
fn extract_command_roundtrips_source_and_assets_byte_exactly() {
    let dir = temp_dir("roundtrip");
    let input = dir.join("roundtrip.md");
    fs::write(&input, fixture_bytes("extract-roundtrip.md")).expect("write input");
    fs::write(&dir.join("asset-tiny.svg"), fixture_bytes("asset-tiny.svg")).expect("write svg");
    fs::write(&dir.join("asset-tiny.css"), fixture_bytes("asset-tiny.css")).expect("write css");

    let built = run(&["build", input.to_str().expect("utf8 path")]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&built.stdout), "");

    let artifact = dir.join("roundtrip.md.html");
    let restored = dir.join("restored.md");
    let extracted = run(&[
        "extract",
        artifact.to_str().expect("utf8 path"),
        "-o",
        restored.to_str().expect("utf8 path"),
    ]);
    assert!(
        extracted.status.success(),
        "{}",
        String::from_utf8_lossy(&extracted.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&extracted.stdout), "");
    assert_eq!(String::from_utf8_lossy(&extracted.stderr), "");
    assert_eq!(
        fs::read(&restored).expect("read restored"),
        fixture_bytes("extract-roundtrip.md"),
        "build → extract round-trip is byte-identical"
    );

    let assets_dir = dir.join("assets");
    let extracted_assets = run(&[
        "extract",
        artifact.to_str().expect("utf8 path"),
        "--assets",
        assets_dir.to_str().expect("utf8 path"),
    ]);
    assert!(
        extracted_assets.status.success(),
        "{}",
        String::from_utf8_lossy(&extracted_assets.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&extracted_assets.stdout),
        "mdhtml: I-CLI-03: extracted asset-tiny.svg (image/svg+xml)\n\
         mdhtml: I-CLI-03: extracted asset-tiny.css (text/css)\n"
    );
    assert_eq!(
        fs::read(assets_dir.join("asset-tiny.svg")).expect("read extracted svg"),
        fixture_bytes("asset-tiny.svg"),
        "svg bytes round-trip exactly"
    );
    assert_eq!(
        fs::read(assets_dir.join("asset-tiny.css")).expect("read extracted css"),
        fixture_bytes("asset-tiny.css"),
        "css bytes round-trip exactly"
    );
}

#[test]
fn extract_command_rejects_invalid_artifacts_before_any_write() {
    let dir = temp_dir("invalid");
    let artifact = repo_root().join("fixtures/extract-invalid.md.html");
    let restored = dir.join("out.md");
    let assets_dir = dir.join("assets");

    let output = run(&[
        "extract",
        artifact.to_str().expect("utf8 path"),
        "--assets",
        assets_dir.to_str().expect("utf8 path"),
        "-o",
        restored.to_str().expect("utf8 path"),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "mdhtml: E-CLI-03: asset 'images/broken.png' has an invalid base64 payload\n"
    );
    assert!(!restored.exists(), "no source output on failure");
    assert!(!assets_dir.exists(), "no asset directory on failure");
}

#[test]
fn extract_command_refuses_to_overwrite_existing_targets() {
    let dir = temp_dir("collision");
    let input = dir.join("note.md");
    fs::write(&input, "---\ntitle: Note\n---\n# Note\n").expect("write input");
    let built = run(&["build", input.to_str().expect("utf8 path")]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let artifact = dir.join("note.md.html");

    let restored = dir.join("out.md");
    fs::write(&restored, "precious").expect("write existing output");
    let output = run(&[
        "extract",
        artifact.to_str().expect("utf8 path"),
        "-o",
        restored.to_str().expect("utf8 path"),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with("mdhtml: E-CLI-03: output "),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(&restored).expect("read output"),
        "precious",
        "existing output is never overwritten"
    );

    let asset_input = dir.join("with-asset.md");
    fs::create_dir_all(dir.join("images")).expect("create images dir");
    fs::write(dir.join("images/photo.png"), b"photo-bytes").expect("write source asset");
    fs::write(
        &asset_input,
        "---\ntitle: Asset doc\n---\n\n![photo](images/photo.png)\n",
    )
    .expect("write input");
    let built = run(&["build", asset_input.to_str().expect("utf8 path")]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let artifact = dir.join("with-asset.md.html");

    let assets_dir = dir.join("assets");
    fs::create_dir_all(assets_dir.join("images")).expect("create target dir");
    fs::write(assets_dir.join("images/photo.png"), b"precious").expect("pre-existing target");
    let output = run(&[
        "extract",
        artifact.to_str().expect("utf8 path"),
        "--assets",
        assets_dir.to_str().expect("utf8 path"),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with("mdhtml: E-CLI-03: asset target "),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(assets_dir.join("images/photo.png")).expect("read target"),
        b"precious",
        "existing asset target is never overwritten"
    );
}

#[test]
fn extract_command_writes_source_to_stdout_without_options() {
    let dir = temp_dir("stdout");
    let input = dir.join("note.md");
    let source = "---\ntitle: Note\n---\n# Note\n";
    fs::write(&input, source).expect("write input");
    let built = run(&["build", input.to_str().expect("utf8 path")]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let artifact = dir.join("note.md.html");

    let output = run(&["extract", artifact.to_str().expect("utf8 path")]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    assert_eq!(String::from_utf8_lossy(&output.stdout), source);
}

#[test]
fn extract_command_roundtrips_a_crlf_source_byte_exactly() {
    let dir = temp_dir("crlf");
    let input = dir.join("crlf.md");
    let source = "---\r\ntitle: CRLF round-trip\r\n---\r\n\r\n# CRLF body\r\n\r\nUnicode survives: Olá — 日本語 — 🎉\r\n";
    fs::write(&input, source.as_bytes()).expect("write input");

    let built = run(&["build", input.to_str().expect("utf8 path")]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let artifact = dir.join("crlf.md.html");
    let restored = dir.join("restored.md");
    let extracted = run(&[
        "extract",
        artifact.to_str().expect("utf8 path"),
        "-o",
        restored.to_str().expect("utf8 path"),
    ]);
    assert!(
        extracted.status.success(),
        "{}",
        String::from_utf8_lossy(&extracted.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&extracted.stdout), "");
    assert_eq!(String::from_utf8_lossy(&extracted.stderr), "");
    assert_eq!(
        fs::read(&restored).expect("read restored"),
        source.as_bytes(),
        "build → extract round-trip of a CRLF source is byte-identical"
    );
}
