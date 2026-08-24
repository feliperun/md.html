use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

const HELP: &str = "Usage: mdhtml <command>\n\nmdhtml build <in.md> [-o out] [--watch] [--no-fonts] [--unsafe]\nmdhtml check <file>\nmdhtml audit <file.md.html> [--json]\nmdhtml extract <in.md.html> [-o out.md] [--assets dir]\nmdhtml publish <source> [--url <base-url>]\nmdhtml new <name> [--template resume|memo|spec|recipe|chapter]\nmdhtml themes\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliAction {
    Help,
    Version,
    Command(Command),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Build {
        input: PathBuf,
        output: Option<PathBuf>,
        watch: bool,
        no_fonts: bool,
        unsafe_mode: bool,
    },
    Check {
        file: PathBuf,
    },
    Audit {
        file: PathBuf,
        json: bool,
    },
    Extract {
        input: PathBuf,
        output: Option<PathBuf>,
        assets: Option<PathBuf>,
    },
    Publish {
        source: PathBuf,
        url: Option<String>,
    },
    New {
        name: OsString,
        template: Option<ParsedTemplate>,
    },
    Themes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParsedTemplate {
    Resume,
    Memo,
    Spec,
    Recipe,
    Chapter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn format_for_user(&self) -> String {
        format!("mdhtml: E-CLI-05: {}", self.message)
    }

    pub(crate) fn from_not_implemented(command: &str) -> Self {
        Self::new(format!("{command} is not implemented"))
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.format_for_user())
    }
}

impl std::error::Error for CliError {}

pub fn parse_args<I, T>(args: I) -> Result<CliAction, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let Some(first) = args.first() else {
        return Err(CliError::new("a command is required; use --help for usage"));
    };

    if args.len() == 1 && (first == "-h" || first == "--help") {
        return Ok(CliAction::Help);
    }
    if args.len() == 1 && (first == "-V" || first == "--version") {
        return Ok(CliAction::Version);
    }
    if is_dash_prefixed(first) {
        return Err(CliError::new(format!(
            "unsupported top-level option {}; use --help for usage",
            display(first)
        )));
    }

    dispatch_subcommand(first, &args[1..])
}

/// The subcommand table: each known name parses its own remaining args; an
/// unrecognized name is `E-CLI-05`.
fn dispatch_subcommand(name: &OsStr, rest: &[OsString]) -> Result<CliAction, CliError> {
    match name.to_str() {
        Some("build") => parse_build(rest).map(CliAction::Command),
        Some("check") => parse_check(rest).map(CliAction::Command),
        Some("audit") => parse_audit(rest).map(CliAction::Command),
        Some("extract") => parse_extract(rest).map(CliAction::Command),
        Some("publish") => parse_publish(rest).map(CliAction::Command),
        Some("new") => parse_new(rest).map(CliAction::Command),
        Some("themes") => parse_themes(rest).map(CliAction::Command),
        _ => Err(CliError::new(format!(
            "unknown subcommand {}; use --help for usage",
            display(name)
        ))),
    }
}

pub fn help_text() -> String {
    HELP.to_owned()
}

pub fn version_text() -> String {
    format!("mdhtml {}\n", env!("CARGO_PKG_VERSION"))
}

fn parse_build(args: &[OsString]) -> Result<Command, CliError> {
    let mut input = None;
    let mut output = None;
    let mut watch = false;
    let mut no_fonts = false;
    let mut unsafe_mode = false;
    let mut options = true;
    let mut index = 0;

    while index < args.len() {
        let token = &args[index];
        if options && token == "--" {
            options = false;
            index += 1;
            continue;
        }
        if options && is_dash_prefixed(token) {
            parse_build_option(
                token,
                &mut output,
                &mut watch,
                &mut no_fonts,
                &mut unsafe_mode,
                args,
                &mut index,
            )?;
        } else {
            set_input(&mut input, token)?;
        }
        index += 1;
    }

    let input = input.ok_or_else(|| missing_positional("build", "<in.md>"))?;
    Ok(Command::Build {
        input,
        output,
        watch,
        no_fonts,
        unsafe_mode,
    })
}

/// One dash-prefixed `build` option: the boolean flags reject duplicates and
/// `-o` consumes the following token as the output path.
fn parse_build_option(
    token: &OsStr,
    output: &mut Option<PathBuf>,
    watch: &mut bool,
    no_fonts: &mut bool,
    unsafe_mode: &mut bool,
    args: &[OsString],
    index: &mut usize,
) -> Result<(), CliError> {
    match token.to_str() {
        Some("--watch") if !*watch => *watch = true,
        Some("--watch") => return duplicate("--watch"),
        Some("--no-fonts") if !*no_fonts => *no_fonts = true,
        Some("--no-fonts") => return duplicate("--no-fonts"),
        Some("--unsafe") if !*unsafe_mode => *unsafe_mode = true,
        Some("--unsafe") => return duplicate("--unsafe"),
        Some("-o") => {
            if output.is_some() {
                return duplicate("-o");
            }
            *output = Some(PathBuf::from(option_value(args, index, token)?));
        }
        _ => return unknown_option(token),
    }
    Ok(())
}

fn parse_check(args: &[OsString]) -> Result<Command, CliError> {
    let file = one_positional(args, "check", "<file>")?;
    Ok(Command::Check { file })
}

fn parse_audit(args: &[OsString]) -> Result<Command, CliError> {
    let mut file = None;
    let mut json = false;
    let mut options = true;
    for token in args {
        if options && token == "--" {
            options = false;
        } else if options && is_dash_prefixed(token) {
            match token.to_str() {
                Some("--json") if !json => json = true,
                Some("--json") => return duplicate("--json"),
                _ => return unknown_option(token),
            }
        } else if file.replace(PathBuf::from(token)).is_some() {
            return Err(CliError::new("audit accepts one positional argument"));
        }
    }
    let file = file.ok_or_else(|| missing_positional("audit", "<file.md.html>"))?;
    Ok(Command::Audit { file, json })
}

fn parse_extract(args: &[OsString]) -> Result<Command, CliError> {
    let mut input = None;
    let mut output = None;
    let mut assets = None;
    let mut options = true;
    let mut index = 0;

    while index < args.len() {
        let token = &args[index];
        if options && token == "--" {
            options = false;
            index += 1;
            continue;
        }
        if options && is_dash_prefixed(token) {
            match token.to_str() {
                Some("-o") => {
                    if output.is_some() {
                        return duplicate("-o");
                    }
                    output = Some(PathBuf::from(option_value(args, &mut index, token)?));
                }
                Some("--assets") => {
                    if assets.is_some() {
                        return duplicate("--assets");
                    }
                    assets = Some(PathBuf::from(option_value(args, &mut index, token)?));
                }
                _ => return unknown_option(token),
            }
        } else {
            set_input(&mut input, token)?;
        }
        index += 1;
    }

    let input = input.ok_or_else(|| missing_positional("extract", "<in.md.html>"))?;
    Ok(Command::Extract {
        input,
        output,
        assets,
    })
}

/// One positional `<source>` and the optional `--url` base; duplicates and
/// unknown options are rejected like every other parser (E-CLI-05).
fn parse_publish(args: &[OsString]) -> Result<Command, CliError> {
    let mut source = None;
    let mut url = None;
    let mut options = true;
    let mut index = 0;

    while index < args.len() {
        let token = &args[index];
        if options && token == "--" {
            options = false;
            index += 1;
            continue;
        }
        if options && is_dash_prefixed(token) {
            parse_publish_option(token, &mut url, args, &mut index)?;
        } else {
            set_input(&mut source, token)?;
        }
        index += 1;
    }

    let source = source.ok_or_else(|| missing_positional("publish", "<source>"))?;
    Ok(Command::Publish { source, url })
}

/// The one dash-prefixed `publish` option: `--url` consumes the following
/// token as the publish endpoint's base URL.
fn parse_publish_option(
    token: &OsStr,
    url: &mut Option<String>,
    args: &[OsString],
    index: &mut usize,
) -> Result<(), CliError> {
    match token.to_str() {
        Some("--url") => {
            if url.is_some() {
                return duplicate("--url");
            }
            *url = Some(option_value(args, index, token)?.to_string_lossy().into_owned());
            Ok(())
        }
        _ => unknown_option(token),
    }
}

fn parse_new(args: &[OsString]) -> Result<Command, CliError> {
    let mut name = None;
    let mut template = None;
    let mut options = true;
    let mut index = 0;

    while index < args.len() {
        let token = &args[index];
        if options && token == "--" {
            options = false;
            index += 1;
            continue;
        }
        if options && is_dash_prefixed(token) {
            match token.to_str() {
                Some("--template") => {
                    if template.is_some() {
                        return duplicate("--template");
                    }
                    let value = option_value(args, &mut index, token)?;
                    template = Some(parse_template(&value)?);
                }
                _ => return unknown_option(token),
            }
        } else if name.replace(token.clone()).is_some() {
            return Err(CliError::new("new accepts one positional argument"));
        }
        index += 1;
    }

    let name = name.ok_or_else(|| missing_positional("new", "<name>"))?;
    Ok(Command::New { name, template })
}

fn parse_themes(args: &[OsString]) -> Result<Command, CliError> {
    if args.iter().any(|arg| *arg != "--") || args.iter().filter(|arg| *arg == "--").count() > 1 {
        return Err(CliError::new("themes does not accept arguments"));
    }
    Ok(Command::Themes)
}

fn one_positional(args: &[OsString], command: &str, usage: &str) -> Result<PathBuf, CliError> {
    let mut value = None;
    let mut options = true;
    for token in args {
        if options && token == "--" {
            options = false;
        } else if options && is_dash_prefixed(token) {
            return unknown_option(token);
        } else if value.replace(PathBuf::from(token)).is_some() {
            return Err(CliError::new(format!(
                "{command} accepts one positional argument"
            )));
        }
    }
    value.ok_or_else(|| missing_positional(command, usage))
}

fn set_input(input: &mut Option<PathBuf>, token: &OsStr) -> Result<(), CliError> {
    if input.replace(PathBuf::from(token)).is_some() {
        return Err(CliError::new("command accepts one positional argument"));
    }
    Ok(())
}

fn option_value(
    args: &[OsString],
    index: &mut usize,
    option: &OsStr,
) -> Result<OsString, CliError> {
    let Some(value) = args.get(*index + 1) else {
        return Err(CliError::new(format!(
            "option {} requires a value",
            display(option)
        )));
    };
    if value == "--" || is_dash_prefixed(value) {
        return Err(CliError::new(format!(
            "option {} requires a value",
            display(option)
        )));
    }
    *index += 1;
    Ok(value.clone())
}

fn parse_template(value: &OsStr) -> Result<ParsedTemplate, CliError> {
    match value.to_str() {
        Some("resume") => Ok(ParsedTemplate::Resume),
        Some("memo") => Ok(ParsedTemplate::Memo),
        Some("spec") => Ok(ParsedTemplate::Spec),
        Some("recipe") => Ok(ParsedTemplate::Recipe),
        Some("chapter") => Ok(ParsedTemplate::Chapter),
        _ => Err(CliError::new(format!(
            "invalid template {}",
            display(value)
        ))),
    }
}

fn is_dash_prefixed(value: &OsStr) -> bool {
    value.to_str().is_some_and(|text| text.starts_with('-'))
}

fn display(value: &OsStr) -> String {
    value
        .to_string_lossy()
        .chars()
        .flat_map(char::escape_default)
        .collect()
}

fn duplicate<T>(option: &str) -> Result<T, CliError> {
    Err(CliError::new(format!("duplicate option {option}")))
}

fn unknown_option<T>(option: &OsStr) -> Result<T, CliError> {
    Err(CliError::new(format!("unknown option {}", display(option))))
}

fn missing_positional(command: &str, usage: &str) -> CliError {
    CliError::new(format!("{command} requires {usage}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_build_options_in_any_order() {
        assert_eq!(
            parse_args(args(&[
                "build",
                "--no-fonts",
                "-o",
                "out",
                "input.md",
                "--watch",
                "--unsafe"
            ])),
            Ok(CliAction::Command(Command::Build {
                input: PathBuf::from("input.md"),
                output: Some(PathBuf::from("out")),
                watch: true,
                no_fonts: true,
                unsafe_mode: true,
            }))
        );
    }

    #[test]
    fn parses_end_marker_for_dash_prefixed_paths_and_names() {
        assert_eq!(
            parse_args(args(&["build", "--", "-input.md"])),
            Ok(CliAction::Command(Command::Build {
                input: PathBuf::from("-input.md"),
                output: None,
                watch: false,
                no_fonts: false,
                unsafe_mode: false,
            }))
        );
        assert_eq!(
            parse_args(args(&["new", "--", "-draft"])),
            Ok(CliAction::Command(Command::New {
                name: OsString::from("-draft"),
                template: None,
            }))
        );
    }

    #[test]
    fn parses_all_commands_and_templates() {
        assert!(matches!(
            parse_args(args(&["check", "file"])),
            Ok(CliAction::Command(Command::Check { .. }))
        ));
        assert!(matches!(
            parse_args(args(&[
                "extract", "file", "--assets", "assets", "-o", "out"
            ])),
            Ok(CliAction::Command(Command::Extract { .. }))
        ));
        for template in ["resume", "memo", "spec", "recipe", "chapter"] {
            assert!(matches!(
                parse_args(args(&["new", "name", "--template", template])),
                Ok(CliAction::Command(Command::New {
                    template: Some(_),
                    ..
                }))
            ));
        }
        assert_eq!(
            parse_args(args(&["themes"])),
            Ok(CliAction::Command(Command::Themes))
        );
    }

    #[test]
    fn parses_only_top_level_help_and_version() {
        assert_eq!(parse_args(args(&["-h"])), Ok(CliAction::Help));
        assert_eq!(parse_args(args(&["--help"])), Ok(CliAction::Help));
        assert_eq!(parse_args(args(&["-V"])), Ok(CliAction::Version));
        assert_eq!(parse_args(args(&["--version"])), Ok(CliAction::Version));
        assert!(parse_args(args(&["build", "--help", "file"])).is_err());
        assert!(parse_args(args(&["--help", "build"])).is_err());
    }

    #[test]
    fn rejects_every_invalid_argument_shape() {
        let invalid = [
            vec![],
            vec!["unknown"],
            vec!["--watch"],
            vec!["build"],
            vec!["build", "a", "b"],
            vec!["build", "--bad", "a"],
            vec!["build", "-o"],
            vec!["build", "-o", "--", "a"],
            vec!["build", "-o", "a", "--output", "b", "input"],
            vec!["build", "--watch", "--watch", "input"],
            vec!["build", "--no-fonts", "--no-fonts", "input"],
            vec!["build", "--unsafe", "--unsafe", "input"],
            vec!["check"],
            vec!["check", "--bad"],
            vec!["check", "a", "b"],
            vec!["audit"],
            vec!["audit", "--bad", "a"],
            vec!["audit", "a", "b"],
            vec!["audit", "a", "--json", "--json"],
            vec!["extract"],
            vec!["extract", "--assets"],
            vec!["extract", "--assets", "a", "--assets", "b", "input"],
            vec!["extract", "a", "b"],
            vec!["new"],
            vec!["new", "a", "b"],
            vec!["new", "--template"],
            vec!["new", "--template", "bad", "a"],
            vec!["new", "--template", "memo", "--template", "spec", "a"],
            vec!["themes", "extra"],
        ];
        for case in invalid {
            assert!(
                parse_args(args(&case)).is_err(),
                "accepted invalid args: {case:?}"
            );
        }
    }

    #[test]
    fn help_lists_the_unsafe_flag_on_the_build_usage_line() {
        assert!(help_text().contains(
            "mdhtml build <in.md> [-o out] [--watch] [--no-fonts] [--unsafe]\n"
        ));
    }

    #[test]
    fn parses_audit_file_and_json_flag() {
        assert_eq!(
            parse_args(args(&["audit", "note.md.html"])),
            Ok(CliAction::Command(Command::Audit {
                file: PathBuf::from("note.md.html"),
                json: false,
            }))
        );
        assert_eq!(
            parse_args(args(&["audit", "note.md.html", "--json"])),
            Ok(CliAction::Command(Command::Audit {
                file: PathBuf::from("note.md.html"),
                json: true,
            }))
        );
        assert_eq!(
            parse_args(args(&["audit", "--json", "note.md.html"])),
            Ok(CliAction::Command(Command::Audit {
                file: PathBuf::from("note.md.html"),
                json: true,
            }))
        );
    }

    #[test]
    fn audit_rejects_duplicate_json_and_missing_positional() {
        assert_eq!(
            parse_args(args(&["audit", "a.md.html", "--json", "--json"]))
                .expect_err("duplicate --json")
                .to_string(),
            "mdhtml: E-CLI-05: duplicate option --json"
        );
        assert_eq!(
            parse_args(args(&["audit"])).expect_err("missing positional").to_string(),
            "mdhtml: E-CLI-05: audit requires <file.md.html>"
        );
    }

    #[test]
    fn help_lists_the_audit_usage_line() {
        assert!(help_text().contains(
            "mdhtml audit <file.md.html> [--json]\n"
        ));
    }

    #[test]
    fn parses_publish_source_and_url() {
        assert_eq!(
            parse_args(args(&["publish", "doc.md"])),
            Ok(CliAction::Command(Command::Publish {
                source: PathBuf::from("doc.md"),
                url: None,
            }))
        );
        assert_eq!(
            parse_args(args(&["publish", "doc.md", "--url", "http://127.0.0.1:8080"])),
            Ok(CliAction::Command(Command::Publish {
                source: PathBuf::from("doc.md"),
                url: Some("http://127.0.0.1:8080".to_string()),
            }))
        );
    }

    #[test]
    fn publish_rejects_duplicate_url_missing_source_and_extra_positional() {
        assert_eq!(
            parse_args(args(&[
                "publish", "a.md", "--url", "http://a", "--url", "http://b"
            ]))
            .expect_err("duplicate --url")
            .to_string(),
            "mdhtml: E-CLI-05: duplicate option --url"
        );
        assert_eq!(
            parse_args(args(&["publish"]))
                .expect_err("missing source")
                .to_string(),
            "mdhtml: E-CLI-05: publish requires <source>"
        );
        assert!(parse_args(args(&["publish", "a.md", "b.md"])).is_err());
    }

    #[test]
    fn help_lists_the_publish_usage_line() {
        assert!(help_text().contains("mdhtml publish <source> [--url <base-url>]\n"));
    }

    #[test]
    fn formats_all_errors_with_one_public_prefix() {
        let error = parse_args(args(&["build"])).expect_err("build needs an input");
        assert_eq!(
            error.to_string(),
            "mdhtml: E-CLI-05: build requires <in.md>"
        );
        assert_eq!(error.format_for_user(), error.to_string());
    }
}
