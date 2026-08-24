use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::build::{self, BuildError};
use crate::cli::{Command, ParsedTemplate};

/// CLI-04: the fixed interval at which `--watch` polls the input.
const WATCH_POLL_INTERVAL: Duration = Duration::from_millis(200);

pub fn dispatch(command: Command) -> Result<String, BuildError> {
    match command {
        Command::Build {
            input,
            output,
            watch,
            no_fonts,
            unsafe_mode,
        } => build(input, output, watch, no_fonts, unsafe_mode),
        Command::Check { file } => check(file),
        Command::Audit { file, json } => audit(file, json),
        Command::Extract {
            input,
            output,
            assets,
        } => extract(input, output, assets),
        Command::Publish { source, url } => publish(&source, url.as_deref()),
        Command::New { name, template } => new(name, template),
        Command::Themes => themes(),
    }
}

/// Phase 5: build + audit locally, then upload the source plus referenced
/// assets to the Publish API and return the server-issued public URL.
fn publish(source: &Path, base_url: Option<&str>) -> Result<String, BuildError> {
    crate::publish::publish(source, base_url)
}

fn build(
    input: PathBuf,
    output: Option<PathBuf>,
    watch_flag: bool,
    no_fonts: bool,
    unsafe_mode: bool,
) -> Result<String, BuildError> {
    if watch_flag {
        return watch(input, output, no_fonts, unsafe_mode);
    }
    let source = read_source(&input)?;
    let source_dir = parent_dir(&input);
    let (runtime_dir, themes_dir, fonts_dir) = crate::repo::repository_layout();
    let document = build_document(
        &source,
        &source_dir,
        no_fonts,
        unsafe_mode,
        &runtime_dir,
        &themes_dir,
        &fonts_dir,
    )?;
    if unsafe_mode {
        warn_unsafe_build();
    }
    let destination = output.unwrap_or_else(|| append_html(&input));
    write_atomic(&destination, document.as_bytes())?;
    Ok(String::new())
}

/// W-MDHSEC-019: the one CLI-05 warning an unsafe build prints to stderr,
/// before the build reports success (ADR 0009).
fn warn_unsafe_build() {
    eprintln!(
        "mdhtml: W-MDHSEC-019: --unsafe disables the security guards; \
         this artifact is marked unsafe and will fail mdhtml audit"
    );
}

fn read_source(input: &Path) -> Result<String, BuildError> {
    fs::read_to_string(input).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is unreadable: {error}", input.display()),
        )
    })
}

fn read_bytes(input: &Path) -> Result<Vec<u8>, BuildError> {
    fs::read(input).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is unreadable: {error}", input.display()),
        )
    })
}

fn parent_dir(input: &Path) -> PathBuf {
    match input.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn build_document(
    source: &str,
    source_dir: &Path,
    no_fonts: bool,
    unsafe_mode: bool,
    runtime_dir: &Path,
    themes_dir: &Path,
    fonts_dir: &Path,
) -> Result<String, BuildError> {
    match (no_fonts, unsafe_mode) {
        (false, false) => build::build(source, source_dir, runtime_dir, themes_dir, fonts_dir),
        (true, false) => {
            build::build_no_fonts(source, source_dir, runtime_dir, themes_dir, fonts_dir)
        }
        (false, true) => {
            build::build_unsafe(source, source_dir, runtime_dir, themes_dir, fonts_dir)
        }
        (true, true) => {
            build::build_unsafe_no_fonts(source, source_dir, runtime_dir, themes_dir, fonts_dir)
        }
    }
}

/// CLI-04: `--watch`. Poll the input at a fixed interval and rebuild once per
/// change into the destination through the accepted atomic write. A rerun
/// never duplicates or destroys files, and the destination is never left
/// partial: every write is temp + rename, so termination by SIGINT/SIGTERM
/// cannot corrupt it. A failed rebuild reports one CLI-05 line and the last
/// good destination stays in place.
fn watch(
    input: PathBuf,
    output: Option<PathBuf>,
    no_fonts: bool,
    unsafe_mode: bool,
) -> Result<String, BuildError> {
    let destination = output.unwrap_or_else(|| append_html(&input));
    let source_dir = parent_dir(&input);
    let (runtime_dir, themes_dir, fonts_dir) = crate::repo::repository_layout();
    let mut last = read_bytes(&input)?;
    let initial = String::from_utf8(last.clone()).map_err(|_| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is not valid UTF-8", input.display()),
        )
    })?;
    let document = build_document(
        &initial,
        &source_dir,
        no_fonts,
        unsafe_mode,
        &runtime_dir,
        &themes_dir,
        &fonts_dir,
    )?;
    if unsafe_mode {
        warn_unsafe_build();
    }
    write_atomic(&destination, document.as_bytes())?;
    loop {
        thread::sleep(WATCH_POLL_INTERVAL);
        let current = match read_bytes(&input) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{error}");
                continue;
            }
        };
        if current == last {
            continue;
        }
        last = current;
        let source = match String::from_utf8(last.clone()) {
            Ok(source) => source,
            Err(_) => {
                eprintln!(
                    "mdhtml: E-CLI-05: input {} is not valid UTF-8",
                    input.display()
                );
                continue;
            }
        };
        let rebuilt = build_document(
            &source,
            &source_dir,
            no_fonts,
            unsafe_mode,
            &runtime_dir,
            &themes_dir,
            &fonts_dir,
        )
        .and_then(|document| {
            if unsafe_mode {
                warn_unsafe_build();
            }
            write_atomic(&destination, document.as_bytes()).map(|_| ())
        });
        if let Err(error) = rebuilt {
            eprintln!("{error}");
        }
    }
}

/// CLI-04: materialize one of the five canonical templates into the given
/// file name. `memo` is the default template; an unknown template is already
/// rejected by the CLI grammar as E-CLI-05. An existing target fails with
/// E-CLI-04 and is never overwritten.
fn new(name: OsString, template: Option<ParsedTemplate>) -> Result<String, BuildError> {
    let template = template.unwrap_or(ParsedTemplate::Memo);
    let destination = PathBuf::from(&name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(BuildError::new(
            "E-CLI-04",
            format!("output {} already exists", destination.display()),
        ));
    }
    if let Some(parent) = destination.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| {
                BuildError::new(
                    "E-CLI-05",
                    format!("output {} is not writable: {error}", destination.display()),
                )
            })?;
        }
    }
    let source = template_source(template);
    fs::write(&destination, source).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("output {} is not writable: {error}", destination.display()),
        )
    })?;
    Ok(format!(
        "mdhtml: I-CLI-04: created {} from {}\n",
        destination.display(),
        template_name(template)
    ))
}

/// CLI-04: list the built-in presets in the accepted deterministic order.
fn themes() -> Result<String, BuildError> {
    Ok("mdhtml: I-CLI-04: technical\nmdhtml: I-CLI-04: editorial\n".to_string())
}

fn template_source(template: ParsedTemplate) -> &'static str {
    match template {
        ParsedTemplate::Resume => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../templates/resume.md"
        )),
        ParsedTemplate::Memo => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../templates/memo.md"
        )),
        ParsedTemplate::Spec => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../templates/spec.md"
        )),
        ParsedTemplate::Recipe => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../templates/recipe.md"
        )),
        ParsedTemplate::Chapter => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../templates/chapter.md"
        )),
    }
}

fn template_name(template: ParsedTemplate) -> &'static str {
    match template {
        ParsedTemplate::Resume => "resume",
        ParsedTemplate::Memo => "memo",
        ParsedTemplate::Spec => "spec",
        ParsedTemplate::Recipe => "recipe",
        ParsedTemplate::Chapter => "chapter",
    }
}

fn append_html(input: &Path) -> PathBuf {
    let mut value = input.as_os_str().to_os_string();
    value.push(".html");
    PathBuf::from(value)
}

fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<(), BuildError> {
    let directory = match destination.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let file_name = destination.file_name().ok_or_else(|| {
        BuildError::new(
            "E-CLI-05",
            format!("output {} is not a file path", destination.display()),
        )
    })?;
    let temporary = directory.join(format!(".{}.tmp", file_name.to_string_lossy()));
    fs::write(&temporary, bytes).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("output {} is not writable: {error}", destination.display()),
        )
    })?;
    fs::rename(&temporary, destination).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        BuildError::new(
            "E-CLI-05",
            format!("output {} is not writable: {error}", destination.display()),
        )
    })?;
    Ok(())
}

/// CLI-02: validate a `.md` source or a built `.md.html` artifact and print
/// the deterministic report (every diagnostic plus the I-CLI-02 verdict and
/// byte budgets). Errors exit nonzero; warnings alone exit zero.
fn check(input: std::path::PathBuf) -> Result<String, BuildError> {
    let bytes = fs::read(&input).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is unreadable: {error}", input.display()),
        )
    })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is not valid UTF-8", input.display()),
        )
    })?;
    let report = if is_artifact(&input) {
        crate::check::check_artifact(&text)
    } else {
        let (runtime_dir, _themes_dir, fonts_dir) = crate::repo::repository_layout();
        crate::check::check_source(&text, &runtime_dir, &fonts_dir)
    };
    print!("{}", report.render());
    if report.has_errors() {
        Err(BuildError::new("E-CLI-02", "document failed check"))
    } else {
        Ok(String::new())
    }
}

/// CLI-06: audit a BUILT `.md.html` artifact — never the source — and
/// print the PRD §13 check lines or the frozen `--json` schema. A failed
/// audit still prints the full report to stdout, then returns E-CLI-06 so
/// the process exits 1 with that single stderr line (the exact pattern
/// `check` uses with E-CLI-02); unreadable, non-UTF-8 or non-`.md.html`
/// input is E-CLI-05.
fn audit(input: PathBuf, json: bool) -> Result<String, BuildError> {
    let bytes = fs::read(&input).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is unreadable: {error}", input.display()),
        )
    })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is not valid UTF-8", input.display()),
        )
    })?;
    if !is_mdhtml_artifact(&input) {
        return Err(BuildError::new(
            "E-CLI-05",
            format!("input {} is not a built .md.html artifact", input.display()),
        ));
    }
    let report = crate::audit::audit_artifact(&text);
    if json {
        print!("{}", report.render_json());
    } else {
        print!("{}", report.render());
    }
    if report.safe {
        Ok(String::new())
    } else {
        Err(BuildError::new("E-CLI-06", "artifact failed audit"))
    }
}

/// Audit accepts only `.md.html` artifacts (CLI-06), unlike `check` which
/// also validates `.md` sources.
fn is_mdhtml_artifact(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".md.html"))
}

fn is_artifact(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("html"))
}

/// CLI-03: restore the canonical source byte-for-byte and, with `--assets`,
/// write every embedded asset under the given directory preserving
/// `data-path`. All validation and collision checks complete before any
/// write; asset outputs are staged and committed only after every block
/// validated, so a failure never leaves partial extraction behind.
fn extract(
    input: PathBuf,
    output: Option<PathBuf>,
    assets: Option<PathBuf>,
) -> Result<String, BuildError> {
    let bytes = fs::read(&input).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is unreadable: {error}", input.display()),
        )
    })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        BuildError::new(
            "E-CLI-05",
            format!("input {} is not valid UTF-8", input.display()),
        )
    })?;
    let source = crate::extract::extract_source(text.as_bytes())?;
    let extracted = assets
        .as_ref()
        .map(|_| crate::extract::extract_assets(text.as_bytes()))
        .transpose()?;

    if let Some(destination) = &output {
        if fs::symlink_metadata(destination).is_ok() {
            return Err(BuildError::new(
                "E-CLI-03",
                format!("output {} already exists", destination.display()),
            ));
        }
    }

    let mut report = String::new();
    if let (Some(assets_dir), Some(extracted)) = (&assets, &extracted) {
        let mut targets = Vec::new();
        for asset in extracted {
            let target = assets_dir.join(&asset.path);
            if fs::symlink_metadata(&target).is_ok() {
                return Err(BuildError::new(
                    "E-CLI-03",
                    format!("asset target {} already exists", target.display()),
                ));
            }
            targets.push((asset, target));
        }
        let mut staged = Vec::new();
        for (asset, target) in &targets {
            staged.push(stage_asset(target, &asset.bytes)?);
        }
        for (index, (_, target)) in targets.iter().enumerate() {
            fs::rename(&staged[index], target).map_err(|error| {
                BuildError::new(
                    "E-CLI-05",
                    format!("asset target {} is not writable: {error}", target.display()),
                )
            })?;
        }
        for asset in extracted {
            report.push_str(&format!(
                "mdhtml: I-CLI-03: extracted {} ({})\n",
                asset.path, asset.data_type
            ));
        }
    }

    match (&output, &assets) {
        (Some(destination), _) => write_atomic(destination, &source)?,
        (None, None) => {
            let text = String::from_utf8(source).map_err(|_| {
                BuildError::new(
                    "E-CLI-05",
                    format!("input {} is not valid UTF-8", input.display()),
                )
            })?;
            print!("{text}");
        }
        (None, Some(_)) => {}
    }
    Ok(report)
}

/// Stage one asset as a temporary file in its destination directory; the
/// caller renames it into place only after every block validated.
fn stage_asset(target: &Path, bytes: &[u8]) -> Result<PathBuf, BuildError> {
    let directory = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    fs::create_dir_all(directory).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!(
                "asset directory {} is not writable: {error}",
                directory.display()
            ),
        )
    })?;
    let file_name = target.file_name().ok_or_else(|| {
        BuildError::new(
            "E-CLI-05",
            format!("asset target {} is not a file path", target.display()),
        )
    })?;
    let temporary = directory.join(format!(".{}.tmp", file_name.to_string_lossy()));
    fs::write(&temporary, bytes).map_err(|error| {
        BuildError::new(
            "E-CLI-05",
            format!("asset target {} is not writable: {error}", target.display()),
        )
    })?;
    Ok(temporary)
}
