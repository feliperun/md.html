//! T15 CLI integration (CLI-04): `new`/`themes` commands, `--no-fonts`
//! equivalence with `fonts: system`, and the idempotent signal-clean watch
//! loop. All user-facing lines follow CLI-05: one `mdhtml: <code>:` line.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

fn template_bytes(name: &str) -> Vec<u8> {
    fs::read(repo_root().join("templates").join(format!("{name}.md")))
        .unwrap_or_else(|error| panic!("read templates/{name}.md: {error}"))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mdhtml-t15-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn wait_for(condition: impl FnMut() -> bool, timeout: Duration, what: &str) {
    let deadline = Instant::now() + timeout;
    let mut check = condition;
    while Instant::now() < deadline {
        if check() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

#[test]
fn new_command_materializes_the_selected_and_default_templates() {
    let dir = temp_dir("new");
    for (template, name) in [
        ("resume", "cv.md"),
        ("memo", "status.md"),
        ("spec", "api.md"),
        ("recipe", "dinner.md"),
        ("chapter", "one.md"),
    ] {
        let target = dir.join(name);
        let output = run(&[
            "new",
            target.to_str().expect("utf8 path"),
            "--template",
            template,
        ]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stderr), "");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!(
                "mdhtml: I-CLI-04: created {} from {template}\n",
                target.display()
            )
        );
        assert_eq!(
            fs::read(&target).expect("read materialized template"),
            template_bytes(template),
            "materialized bytes equal the committed template for {template}"
        );
    }

    let default = dir.join("default.md");
    let output = run(&["new", default.to_str().expect("utf8 path")]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(&default).expect("read default template"),
        template_bytes("memo"),
        "no --template selects the memo template"
    );
}

#[test]
fn new_command_never_overwrites_an_existing_file() {
    let dir = temp_dir("new-collision");
    let target = dir.join("existing.md");
    fs::write(&target, "precious content").expect("write existing file");

    let output = run(&["new", target.to_str().expect("utf8 path")]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        format!(
            "mdhtml: E-CLI-04: output {} already exists\n",
            target.display()
        )
    );
    assert_eq!(
        fs::read_to_string(&target).expect("read target"),
        "precious content",
        "an existing file is never overwritten"
    );
}

#[test]
fn new_command_rejects_unknown_templates_as_one_line_cli05() {
    let dir = temp_dir("new-unknown");
    let target = dir.join("draft.md");
    let output = run(&[
        "new",
        target.to_str().expect("utf8 path"),
        "--template",
        "novel",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "mdhtml: E-CLI-05: invalid template novel\n"
    );
    assert!(!target.exists(), "no file is written for an unknown template");
}

#[test]
fn themes_command_lists_the_builtin_presets_in_order() {
    let output = run(&["themes"]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "mdhtml: I-CLI-04: technical\nmdhtml: I-CLI-04: editorial\n"
    );
}

fn build_into(dir: &Path, name: &str, source: &str, args: &[&str]) -> std::process::Output {
    let input = dir.join(name);
    fs::write(&input, source).expect("write input");
    let mut command = vec!["build", input.to_str().expect("utf8 path"), "-o"];
    let output = dir.join(format!("{name}.html"));
    command.push(output.to_str().expect("utf8 path"));
    command.extend_from_slice(args);
    run(&command)
}

/// The embedded canonical source, verbatim, of a built artifact.
fn canonical_source_of(html: &str) -> &str {
    let marker = "<script id=\"mdhtml-source\" type=\"text/markdown\">";
    let start = html.find(marker).expect("source element") + marker.len();
    let end = html[start..].find("</script>").expect("source close") + start;
    &html[start..end]
}

#[test]
fn no_fonts_build_is_equivalent_to_fonts_system() {
    let dir = temp_dir("no-fonts");
    let source = "---\ntitle: Fonts\n---\nBody with *emphasis* and `code`.\n";
    let system_source =
        "---\ntitle: Fonts\nfonts: system\n---\nBody with *emphasis* and `code`.\n";

    let no_fonts = build_into(&dir, "flag.md", source, &["--no-fonts"]);
    assert!(
        no_fonts.status.success(),
        "{}",
        String::from_utf8_lossy(&no_fonts.stderr)
    );
    let system = build_into(&dir, "system.md", system_source, &[]);
    assert!(
        system.status.success(),
        "{}",
        String::from_utf8_lossy(&system.stderr)
    );

    let flagged = fs::read_to_string(dir.join("flag.md.html")).expect("read no-fonts build");
    let system_html = fs::read_to_string(dir.join("system.md.html")).expect("read system build");
    assert!(!flagged.contains("style id=\"mdhtml-fonts\""));
    assert!(!flagged.contains("font/woff2"));
    assert!(!flagged.contains("fonts: system"));
    assert_eq!(
        canonical_source_of(&flagged),
        source,
        "the canonical source is stored verbatim"
    );
    let flagged_rest = flagged.replacen(canonical_source_of(&flagged), "", 1);
    let system_rest = system_html.replacen(canonical_source_of(&system_html), "", 1);
    assert_eq!(
        flagged_rest, system_rest,
        "--no-fonts assembles the document exactly as fonts: system; only the verbatim source differs"
    );

    let flagged_system = build_into(&dir, "flagged-system.md", system_source, &["--no-fonts"]);
    assert!(
        flagged_system.status.success(),
        "{}",
        String::from_utf8_lossy(&flagged_system.stderr)
    );
    assert_eq!(
        fs::read_to_string(dir.join("flagged-system.md.html")).expect("read flagged system build"),
        system_html,
        "--no-fonts is a byte-identical no-op when the source already declares fonts: system"
    );
}

#[test]
fn no_fonts_build_differs_from_normal_build_only_in_the_fonts_block() {
    let dir = temp_dir("no-fonts-rest");
    let source = "---\ntitle: Fonts\n---\nBody with *emphasis* and `code`.\n";

    let normal = build_into(&dir, "normal.md", source, &[]);
    assert!(
        normal.status.success(),
        "{}",
        String::from_utf8_lossy(&normal.stderr)
    );
    let no_fonts = build_into(&dir, "flagged.md", source, &["--no-fonts"]);
    assert!(
        no_fonts.status.success(),
        "{}",
        String::from_utf8_lossy(&no_fonts.stderr)
    );

    let normal_html = fs::read_to_string(dir.join("normal.md.html")).expect("read normal build");
    let flagged_html = fs::read_to_string(dir.join("flagged.md.html")).expect("read flagged build");
    let marker = "<style id=\"mdhtml-fonts\">";
    let start = normal_html.find(marker).expect("normal build embeds fonts");
    let end = normal_html[start..]
        .find("</style>")
        .expect("fonts block close")
        + start
        + "</style>".len();
    let mut stripped = normal_html.clone();
    stripped.replace_range(start - 2..end + 1, "");
    assert_eq!(
        stripped, flagged_html,
        "the flagged build is the normal build with only the fonts block removed"
    );
}

#[test]
fn unsafe_build_requires_the_flag_and_attests_and_warns() {
    let dir = temp_dir("unsafe");
    let input = dir.join("unsafe.md");
    let source = "---\ntitle: Unsafe\n---\n\n[click](javascript:alert(1))\n";
    fs::write(&input, source).expect("write input");

    let safe_output = dir.join("safe.html");
    let without = run(&[
        "build",
        input.to_str().expect("utf8 path"),
        "-o",
        safe_output.to_str().expect("utf8 path"),
    ]);
    assert_eq!(without.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&without.stdout), "");
    assert!(
        String::from_utf8_lossy(&without.stderr)
            .starts_with("mdhtml: E-MDHSEC-012: "),
        "{}",
        String::from_utf8_lossy(&without.stderr)
    );
    assert!(
        !safe_output.exists(),
        "a guard-violating source without --unsafe writes no artifact"
    );

    let unsafe_output = dir.join("unsafe.html");
    let with = run(&[
        "build",
        input.to_str().expect("utf8 path"),
        "-o",
        unsafe_output.to_str().expect("utf8 path"),
        "--unsafe",
    ]);
    assert!(
        with.status.success(),
        "{}",
        String::from_utf8_lossy(&with.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&with.stdout), "");
    assert_eq!(
        String::from_utf8_lossy(&with.stderr),
        "mdhtml: W-MDHSEC-019: --unsafe disables the security guards; \
         this artifact is marked unsafe and will fail mdhtml audit\n"
    );
    let artifact = fs::read_to_string(&unsafe_output).expect("read unsafe artifact");
    assert!(artifact.contains("data-mdhtml-safe=\"false\""));

    let checked = run(&["check", unsafe_output.to_str().expect("utf8 path")]);
    assert!(
        checked.status.success(),
        "mdhtml check stays green on the unsafe artifact: {}",
        String::from_utf8_lossy(&checked.stdout)
    );
}

#[test]
fn every_template_builds_and_checks_clean() {
    let dir = temp_dir("templates");
    for template in ["resume", "memo", "spec", "recipe", "chapter"] {
        let source = repo_root().join("templates").join(format!("{template}.md"));
        let artifact = dir.join(format!("{template}.md.html"));
        let built = run(&[
            "build",
            source.to_str().expect("utf8 path"),
            "-o",
            artifact.to_str().expect("utf8 path"),
        ]);
        assert!(
            built.status.success(),
            "{template} build failed: {}",
            String::from_utf8_lossy(&built.stderr)
        );

        let checked = run(&["check", artifact.to_str().expect("utf8 path")]);
        assert!(
            checked.status.success(),
            "{template} artifact check failed: {}",
            String::from_utf8_lossy(&checked.stdout)
        );

        let checked_source = run(&["check", source.to_str().expect("utf8 path")]);
        assert!(
            checked_source.status.success(),
            "{template} source check failed: {}",
            String::from_utf8_lossy(&checked_source.stdout)
        );
    }
}

#[cfg(unix)]
fn spawn_watch(input: &Path, output: &Path) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_mdhtml"))
        .args([
            "build",
            input.to_str().expect("utf8 path"),
            "-o",
            output.to_str().expect("utf8 path"),
            "--watch",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn watch process")
}

#[cfg(unix)]
fn signal(child: &std::process::Child, signal_name: &str) {
    let status = Command::new("kill")
        .args([format!("-{signal_name}"), child.id().to_string()])
        .status()
        .expect("run kill");
    assert!(status.success(), "kill -{signal_name} should succeed");
}

#[cfg(unix)]
#[test]
fn watch_rebuilds_once_per_change_and_stops_cleanly_on_sigint() {
    let dir = temp_dir("watch");
    let input = dir.join("note.md");
    let output = dir.join("note.md.html");
    fs::write(&input, "---\ntitle: Note\n---\n# Note\n").expect("write input");

    let mut child = spawn_watch(&input, &output);
    wait_for(|| output.exists(), Duration::from_secs(5), "initial build");
    let first = fs::read_to_string(&output).expect("read first build");
    assert!(first.starts_with("<!doctype html>\n"));

    fs::write(&input, "---\ntitle: Note\n---\n# Note\n\nAdded paragraph.\n").expect("change input");
    wait_for(
        || fs::read_to_string(&output).ok().as_deref() != Some(first.as_str()),
        Duration::from_secs(5),
        "first rebuild",
    );
    let second = fs::read_to_string(&output).expect("read second build");
    assert!(second.contains("Added paragraph."));
    assert_eq!(second.matches("<!doctype html>").count(), 1, "no duplication");

    fs::write(&input, "---\ntitle: Note\n---\n# Note\n\nSecond paragraph.\n")
        .expect("change input again");
    wait_for(
        || fs::read_to_string(&output).ok().as_deref() != Some(second.as_str()),
        Duration::from_secs(5),
        "second rebuild",
    );
    let third = fs::read_to_string(&output).expect("read third build");
    assert!(third.contains("Second paragraph."));
    assert!(!third.contains("Added paragraph."));
    assert_eq!(third.matches("<!doctype html>").count(), 1, "no duplication");

    signal(&child, "INT");
    let status = child.wait().expect("watch process exits");
    assert!(status.code().is_none(), "SIGINT terminates the process");
    let final_doc = fs::read_to_string(&output).expect("final document readable");
    assert!(
        final_doc.starts_with("<!doctype html>\n"),
        "the destination is never left partial"
    );
}

#[cfg(unix)]
#[test]
fn watch_rerun_is_idempotent_and_never_duplicates() {
    let dir = temp_dir("watch-rerun");
    let input = dir.join("note.md");
    let output = dir.join("note.md.html");
    fs::write(&input, "---\ntitle: Note\n---\n# Note\n").expect("write input");

    let mut first_run = spawn_watch(&input, &output);
    wait_for(|| output.exists(), Duration::from_secs(5), "first run initial build");
    signal(&first_run, "TERM");
    let _ = first_run.wait();

    let mut second_run = spawn_watch(&input, &output);
    wait_for(
        || fs::read_to_string(&output).is_ok(),
        Duration::from_secs(5),
        "second run initial build",
    );
    signal(&second_run, "TERM");
    let _ = second_run.wait();

    let entries: Vec<String> = fs::read_dir(&dir)
        .expect("list watch dir")
        .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    let stray: Vec<&String> = entries
        .iter()
        .filter(|name| !name.starts_with('.'))
        .filter(|name| name.as_str() != "note.md" && name.as_str() != "note.md.html")
        .collect();
    assert!(
        stray.is_empty(),
        "rerunning watch duplicates or creates stray files: {stray:?} (all: {entries:?})"
    );
    let document = fs::read_to_string(&output).expect("read final document");
    assert!(document.starts_with("<!doctype html>\n"));
    assert_eq!(document.matches("<!doctype html>").count(), 1);
}

#[test]
fn audit_of_a_freshly_built_clean_artifact_exits_zero_with_the_prd13_lines() {
    let dir = temp_dir("audit-clean");
    let input = dir.join("note.md");
    fs::write(&input, "---\ntitle: Note\n---\n# Note\n").expect("write input");
    let artifact = dir.join("note.md.html");
    let built = run(&[
        "build",
        input.to_str().expect("utf8 path"),
        "-o",
        artifact.to_str().expect("utf8 path"),
    ]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let audited = run(&["audit", artifact.to_str().expect("utf8 path")]);
    assert!(
        audited.status.success(),
        "{}",
        String::from_utf8_lossy(&audited.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&audited.stderr), "");
    let stdout = String::from_utf8_lossy(&audited.stdout);
    assert_eq!(
        stdout,
        "✓ valid mdhtml v1.0\n\
         ✓ canonical source present\n\
         ✓ source integrity valid\n\
         ✓ HTML security policy passed\n\
         ✓ CSS security policy passed\n\
         ✓ no unauthorized executable content\n\
         ✓ runtime integrity valid\n\
         ✓ no unexpected external resources\n\
         SAFE\n"
    );
}

#[test]
fn audit_json_matches_the_frozen_schema_in_exact_field_order() {
    let dir = temp_dir("audit-json");
    let input = dir.join("note.md");
    fs::write(&input, "---\ntitle: Note\n---\n# Note\n").expect("write input");
    let artifact = dir.join("note.md.html");
    let built = run(&[
        "build",
        input.to_str().expect("utf8 path"),
        "-o",
        artifact.to_str().expect("utf8 path"),
    ]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let audited = run(&[
        "audit",
        artifact.to_str().expect("utf8 path"),
        "--json",
    ]);
    assert!(
        audited.status.success(),
        "{}",
        String::from_utf8_lossy(&audited.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&audited.stderr), "");
    assert_eq!(
        String::from_utf8_lossy(&audited.stdout),
        "{\"safe\":true,\"specVersion\":\"1.0\",\"sourceIntegrity\":true,\"html\":\"pass\",\"css\":\"pass\",\"runtime\":\"pass\",\"externalResources\":[]}\n"
    );
}

#[test]
fn audit_of_an_unsafe_built_artifact_exits_one_and_prints_the_report_to_stdout() {
    let dir = temp_dir("audit-unsafe");
    let input = dir.join("unsafe.md");
    fs::write(
        &input,
        "---\ntitle: Unsafe\n---\n\n[click](javascript:alert(1))\n",
    )
    .expect("write input");
    let artifact = dir.join("unsafe.md.html");
    let built = run(&[
        "build",
        input.to_str().expect("utf8 path"),
        "-o",
        artifact.to_str().expect("utf8 path"),
        "--unsafe",
    ]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let audited = run(&["audit", artifact.to_str().expect("utf8 path")]);
    assert_eq!(audited.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&audited.stderr),
        "mdhtml: E-CLI-06: artifact failed audit\n"
    );
    let stdout = String::from_utf8_lossy(&audited.stdout);
    assert!(stdout.contains("✗ HTML security policy passed\n"), "{stdout}");
    assert!(
        stdout.contains("mdhtml: E-MDHSEC-018: artifact is marked unsafe\n"),
        "{stdout}"
    );
    assert!(stdout.ends_with("UNSAFE\n"), "{stdout}");
}

#[test]
fn audit_of_a_non_artifact_file_exits_one_with_the_cli05_one_liner() {
    let dir = temp_dir("audit-input");
    let input = dir.join("note.md");
    fs::write(&input, "---\ntitle: Note\n---\n# Note\n").expect("write input");

    let audited = run(&["audit", input.to_str().expect("utf8 path")]);
    assert_eq!(audited.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&audited.stdout), "");
    let stderr = String::from_utf8_lossy(&audited.stderr);
    assert!(
        stderr.starts_with("mdhtml: E-CLI-05: input "),
        "{stderr}"
    );
    assert_eq!(stderr.matches('\n').count(), 1);

    let missing = run(&["audit", dir.join("nope.md.html").to_str().expect("utf8 path")]);
    assert_eq!(missing.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&missing.stdout), "");
    assert_eq!(String::from_utf8_lossy(&missing.stderr).matches('\n').count(), 1);
    assert!(String::from_utf8_lossy(&missing.stderr).starts_with("mdhtml: E-CLI-05: "));
}
