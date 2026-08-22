//! Semantic analysis over scanner evidence: closed config normalization,
//! heading identity (SECT-01 slugs, overrides, collisions), ordered
//! section-binding validation (COMP-02 schema/name/class/body shape) and
//! fenced container validation (COMP-02 name/argument/body shape).

use std::collections::HashMap;
use std::ops::Range;

use crate::frontmatter::{Value, parse_front_matter};
use crate::scanner::scan_document;

mod bindings;
mod config;
mod containers;
mod section_components;
mod shape;
mod slug;
mod slug_ascii;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Stable machine-readable record: code, severity, one-line message, plus the
/// optional component `name` and section-slug `target` evidence of SPEC §16.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub name: Option<String>,
    pub target: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            name: None,
            target: None,
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            name: None,
            target: None,
        }
    }

    pub fn info(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Info,
            message: message.into(),
            name: None,
            target: None,
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_target(mut self, target: String) -> Self {
        self.target = Some(target);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Theme {
    Technical,
    Editorial,
    Local(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fonts {
    Auto,
    System,
    Map {
        body: Option<String>,
        mono: Option<String>,
        url: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TocPosition {
    Side,
    Inline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Toc {
    pub depth: u8,
    pub position: TocPosition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TocSetting {
    Disabled,
    Enabled(Toc),
}

/// Reserved front matter keys, closed to the SPEC §8 subset. Local theme/font
/// paths stay as strings; filesystem resolution is a later stage.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedConfig {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub lang: Option<String>,
    pub theme: Theme,
    pub tokens: Value,
    pub fonts: Fonts,
    pub url: Option<String>,
    pub cover: Option<String>,
    pub toc: TocSetting,
    pub sections: Value,
    pub figures: Value,
}

impl Default for NormalizedConfig {
    fn default() -> Self {
        Self {
            title: None,
            summary: None,
            lang: Some("en".to_string()),
            theme: Theme::Technical,
            tokens: Value::Mapping(Vec::new()),
            fonts: Fonts::Auto,
            url: None,
            cover: None,
            toc: TocSetting::Enabled(Toc {
                depth: 3,
                position: TocPosition::Side,
            }),
            sections: Value::Mapping(Vec::new()),
            figures: Value::Mapping(Vec::new()),
        }
    }
}

/// A registered heading: final unique id, projected visible text, and the
/// byte range (body-relative) of its section content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzedSection {
    pub id: String,
    pub level: u8,
    pub text: String,
    pub explicit: bool,
    pub body_range: Range<usize>,
    pub offset: usize,
    pub line: usize,
}

/// A sections entry whose schema, component name, class and section-body
/// shape all validated; these are the applicable bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingBinding {
    pub slug: String,
    pub component: String,
    pub class: Option<String>,
    pub section_index: usize,
}

/// A sections entry whose schema and component name validated but whose
/// section body did not match the component shape: kept for downstream
/// selection while `bindings` stays applicable-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DegradedBinding {
    pub slug: String,
    pub component: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    pub config: NormalizedConfig,
    pub sections: Vec<AnalyzedSection>,
    pub bindings: Vec<PendingBinding>,
    pub degraded: Vec<DegradedBinding>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Analyze the canonical source (front matter + body). Diagnostics keep an
/// explicit order: config (reserved-key order), then heading duplicates in
/// document order, then binding records in `sections:` mapping order, then
/// container records in opener source order.
pub fn analyze_document(source: &str) -> Analysis {
    let mut diagnostics = Vec::new();
    let parsed = match parse_front_matter(source) {
        Ok(parsed) => parsed,
        Err(error) => {
            diagnostics.push(Diagnostic::error("E-PARSE-01", error.message().to_string()));
            return Analysis {
                config: NormalizedConfig::default(),
                sections: Vec::new(),
                bindings: Vec::new(),
                degraded: Vec::new(),
                diagnostics,
            };
        }
    };

    let config = config::normalize(&parsed.front_matter, &mut diagnostics);
    let evidence = scan_document(parsed.body);
    let references = slug::collect_reference_labels(parsed.body);
    let sections = slug::compute_sections(
        &evidence.headings,
        parsed.body,
        &references,
        &mut diagnostics,
    );
    let id_to_index: HashMap<String, usize> = sections
        .iter()
        .enumerate()
        .map(|(index, section)| (section.id.clone(), index))
        .collect();
    let (bindings, degraded) = bindings::validate(
        &config.sections,
        &id_to_index,
        &sections,
        parsed.body,
        &evidence.containers,
        &mut diagnostics,
    );
    containers::validate(&evidence.containers, parsed.body, &mut diagnostics);

    Analysis {
        config,
        sections,
        bindings,
        degraded,
        diagnostics,
    }
}

pub use slug::slugify;
