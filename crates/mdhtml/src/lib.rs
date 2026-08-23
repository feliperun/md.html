pub mod analysis;
pub mod build;
pub mod check;
pub mod cli;
pub mod commands;
pub mod extract;
pub mod frontmatter;
pub mod scanner;
pub mod security;
pub mod selection;

pub use analysis::{
    Analysis, AnalyzedSection, Diagnostic, Fonts, NormalizedConfig, PendingBinding, Severity,
    Theme, Toc, TocPosition, TocSetting, analyze_document, slugify,
};
pub use cli::{CliAction, CliError, Command, ParsedTemplate, parse_args};
pub use scanner::{
    ContainerEvidence, HeadingEvidence, ImageEvidence, ImageKind, LinkEvidence, ScanEvidence,
    scan_document,
};

pub fn run_cli<I, T>(args: I) -> Result<String, build::BuildError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    match parse_args(args) {
        Err(error) => Err(build::BuildError::Cli(error)),
        Ok(CliAction::Help) => Ok(cli::help_text()),
        Ok(CliAction::Version) => Ok(cli::version_text()),
        Ok(CliAction::Command(command)) => commands::dispatch(command),
    }
}
