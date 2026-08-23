//! CLI-06 audit: deterministic pass/fail over a BUILT `.md.html` artifact —
//! never the source. The artifact is inspected structurally (reusing the
//! accepted `check` element scan and stored-source analysis), the stored
//! canonical source re-runs the node-1 located guards, the embedded
//! `style#mdhtml-user` re-passes `guard_author_css`, the runtime hash is
//! recomputed and compared against the CSP, and the external origins are
//! checked against what the CSP would sanction. The report renders the PRD
//! §13 check lines or the frozen `--json` schema.
//!
//! Documented deviation from the Tech Spec: the spec names
//! `crates/mdhtml/src/commands/audit.rs`, but `commands` is a flat
//! `commands.rs` file in this repo; the audit engine lives in this module
//! (`pub mod audit` in lib.rs — same layering as `check/`) with a thin
//! `commands::audit` reader that reads the file and prints.

use crate::analysis::{Diagnostic, Severity};
use crate::build::{self, BuildError};
use crate::check;
use crate::frontmatter::parse_front_matter;
use crate::security::Violation;
use crate::security::css::guard_author_css;

/// One audit diagnostic: a frozen `E-MDHSEC-*` (or stored-source analysis)
/// code and its full located message, including the PRD §14 excerpt block
/// when the guard attached a position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditDiagnostic {
    pub code: &'static str,
    pub message: String,
}

impl AuditDiagnostic {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn from_analysis(diagnostic: &Diagnostic) -> Self {
        Self::new(diagnostic.code, diagnostic.message.clone())
    }
}

/// One PRD §13 check line: the frozen label, its pass/fail verdict and the
/// diagnostics that failed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditCheck {
    pub label: &'static str,
    pub pass: bool,
    pub diagnostics: Vec<AuditDiagnostic>,
}

/// The full deterministic audit result: the eight check lines in frozen
/// order, the optional `E-MDHSEC-018` attestation diagnostic, the distinct
/// external origins the artifact may contact (document order) and the `safe`
/// conjunction (every check passing plus a true attestation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditReport {
    pub checks: Vec<AuditCheck>,
    pub attestation: Option<AuditDiagnostic>,
    pub origins: Vec<String>,
    pub safe: bool,
}

impl AuditReport {
    /// `sourceIntegrity` JSON field: the stored source re-analyzes clean.
    pub fn source_integrity(&self) -> bool {
        self.checks[2].pass
    }

    /// `html` JSON field: the stored-source guard re-run (line 4) AND the
    /// markup structural scan (line 6) both pass.
    pub fn html_pass(&self) -> bool {
        self.checks[3].pass && self.checks[5].pass
    }

    /// `css` JSON field.
    pub fn css_pass(&self) -> bool {
        self.checks[4].pass
    }

    /// `runtime` JSON field.
    pub fn runtime_pass(&self) -> bool {
        self.checks[6].pass
    }

    /// The external-resources category: every origin is CSP-sanctioned.
    pub fn external_resources_pass(&self) -> bool {
        self.checks[7].pass
    }

    /// Whether the report carries a diagnostic with the exact code, anywhere
    /// (a check line or the attestation).
    pub fn has_code(&self, code: &str) -> bool {
        self.checks
            .iter()
            .any(|check| check.diagnostics.iter().any(|d| d.code == code))
            || self.attestation.as_ref().is_some_and(|d| d.code == code)
    }

    /// The PRD §13 human report: one ✓/✗ line per check, each failed check
    /// followed by its located diagnostic block(s), then the attestation
    /// diagnostic when the artifact is marked unsafe, closing with `SAFE` or
    /// `UNSAFE`.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for check in &self.checks {
            out.push_str(if check.pass { "✓ " } else { "✗ " });
            out.push_str(check.label);
            out.push('\n');
            for diagnostic in &check.diagnostics {
                out.push_str(&format!(
                    "mdhtml: {}: {}\n",
                    diagnostic.code, diagnostic.message
                ));
            }
        }
        if let Some(diagnostic) = &self.attestation {
            out.push_str(&format!(
                "mdhtml: {}: {}\n",
                diagnostic.code, diagnostic.message
            ));
        }
        out.push_str(if self.safe { "SAFE\n" } else { "UNSAFE\n" });
        out
    }

    /// The frozen `--json` schema, hand-rolled (no serde): `{ "safe": bool,
    /// "specVersion": "1.0", "sourceIntegrity": bool, "html": "pass"|"fail",
    /// "css": "pass"|"fail", "runtime": "pass"|"fail",
    /// "externalResources": [...] }` in that exact field order.
    pub fn render_json(&self) -> String {
        format!(
            "{{\"safe\":{},\"specVersion\":\"1.0\",\"sourceIntegrity\":{},\"html\":\"{}\",\
             \"css\":\"{}\",\"runtime\":\"{}\",\"externalResources\":[{}]}}\n",
            self.safe,
            self.source_integrity(),
            verdict(self.html_pass()),
            verdict(self.css_pass()),
            verdict(self.runtime_pass()),
            self.origins
                .iter()
                .map(|origin| escape_json_string(origin))
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

fn verdict(pass: bool) -> &'static str {
    if pass { "pass" } else { "fail" }
}

/// One JSON string with quotes, backslashes and control characters escaped
/// defensively (origins are toolchain-derived, but the schema must stay
/// well-formed for any artifact bytes).
fn escape_json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            ch if ch.is_ascii_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

/// Audit one built artifact: run every category check and assemble the
/// frozen report. All checks run regardless of earlier failures, so the
/// report surfaces every violation at once.
pub fn audit_artifact(html: &str) -> AuditReport {
    let elements = check::scan_elements(html);
    let scripts: Vec<&check::Element<'_>> = elements
        .iter()
        .filter(|element| element.is("script"))
        .collect();
    let root = elements.iter().find(|element| element.is("html"));
    let source_scripts: Vec<&check::Element<'_>> = scripts
        .iter()
        .filter(|element| check::attr(element, "id") == Some("mdhtml-source"))
        .copied()
        .collect();
    let markdown_scripts: Vec<&check::Element<'_>> = scripts
        .iter()
        .filter(|element| check::attr(element, "type") == Some("text/markdown"))
        .copied()
        .collect();
    let runtime_scripts: Vec<&check::Element<'_>> = scripts
        .iter()
        .filter(|element| check::attr(element, "id") == Some("mdhtml-runtime"))
        .copied()
        .collect();

    let identity = root.is_some_and(|element| check::attr(element, "data-mdhtml") == Some("1.0"));
    let source_ok = source_scripts.len() == 1
        && markdown_scripts.len() == 1
        && source_scripts[0].attrs.iter().any(|(name, value)| {
            name.eq_ignore_ascii_case("type") && *value == "text/markdown"
        });
    let runtime_present = runtime_scripts.len() == 1;

    let mut identity_diagnostics = Vec::new();
    let mut source_diagnostics = Vec::new();
    let mut runtime_diagnostics = Vec::new();
    if !identity {
        identity_diagnostics.push(structure_diagnostic(
            "document root must declare data-mdhtml=\"1.0\"",
        ));
    }
    if !source_ok {
        source_diagnostics.push(structure_diagnostic(
            "document must contain exactly one script#mdhtml-source[type=\"text/markdown\"]",
        ));
    }
    if !runtime_present {
        runtime_diagnostics.push(structure_diagnostic(
            "document must contain exactly one script#mdhtml-runtime",
        ));
    }

    let stored_source = source_scripts.first().and_then(|element| element.text);
    let stored = stored_source.map(check::analyze_stored_source);

    let integrity = stored
        .as_ref()
        .map(|stored| {
            stored
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity != Severity::Error)
        })
        .unwrap_or(false);
    let integrity_diagnostics = stored
        .as_ref()
        .map(|stored| {
            stored
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == Severity::Error)
                .map(AuditDiagnostic::from_analysis)
                .collect()
        })
        .unwrap_or_default();

    let mut origins = stored
        .as_ref()
        .map(|stored| stored.origins.clone())
        .unwrap_or_default();
    check::collect_html_origins(&elements, &mut origins);

    let (guards_pass, guards_diagnostics, sanctioned) = match stored_source {
        Some(source) => re_run_guards(source),
        None => (true, Vec::new(), Vec::new()),
    };
    let (markup_pass, markup_diagnostics) = scan_markup(&elements);
    let (css_pass, css_diagnostics) = css_check(&elements, &sanctioned);

    let fonts_url = stored.as_ref().and_then(|stored| stored.fonts_url.as_deref());
    let (hash_pass, hash_diagnostics) = runtime_check(&elements, fonts_url);
    let runtime_pass = runtime_present && hash_pass;
    runtime_diagnostics.extend(hash_diagnostics);

    let external_pass = origins.iter().all(|origin| sanctioned.contains(origin));
    let external_diagnostics = if external_pass {
        Vec::new()
    } else {
        vec![structure_diagnostic(
            "artifact references external origin(s) the CSP does not sanction",
        )]
    };

    let attestation_ok = root
        .and_then(|element| check::attr(element, "data-mdhtml-safe"))
        == Some("true");
    let attestation = if attestation_ok {
        None
    } else {
        Some(AuditDiagnostic::new(
            "E-MDHSEC-018",
            "artifact is marked unsafe",
        ))
    };

    let checks = vec![
        AuditCheck {
            label: "valid mdhtml v1.0",
            pass: identity,
            diagnostics: identity_diagnostics,
        },
        AuditCheck {
            label: "canonical source present",
            pass: source_ok,
            diagnostics: source_diagnostics,
        },
        AuditCheck {
            label: "source integrity valid",
            pass: integrity,
            diagnostics: integrity_diagnostics,
        },
        AuditCheck {
            label: "HTML security policy passed",
            pass: guards_pass,
            diagnostics: guards_diagnostics,
        },
        AuditCheck {
            label: "CSS security policy passed",
            pass: css_pass,
            diagnostics: css_diagnostics,
        },
        AuditCheck {
            label: "no unauthorized executable content",
            pass: markup_pass,
            diagnostics: markup_diagnostics,
        },
        AuditCheck {
            label: "runtime integrity valid",
            pass: runtime_pass,
            diagnostics: runtime_diagnostics,
        },
        AuditCheck {
            label: "no unexpected external resources",
            pass: external_pass,
            diagnostics: external_diagnostics,
        },
    ];
    let safe = checks.iter().all(|check| check.pass) && attestation.is_none();
    AuditReport {
        checks,
        attestation,
        origins,
        safe,
    }
}

/// The audit's own artifact-structure code (E-MDHSEC-017); it does NOT reuse
/// check's E-FMT-01.
fn structure_diagnostic(message: impl Into<String>) -> AuditDiagnostic {
    AuditDiagnostic::new("E-MDHSEC-017", message)
}

/// Re-run the node-1 located guards over the stored canonical source: links,
/// images, heading `{#id}` overrides, section/class tokens, the metadata
/// `url` and `fonts.url` (E-MDHSEC-004/-005/-006/-012). The guards reuse the
/// accepted `build::guard_document` pipeline — scanners are never duplicated
/// — and surface the first located violation. The fonts.url-sanctioned
/// origins are returned for the css and external-resources categories.
fn re_run_guards(source: &str) -> (bool, Vec<AuditDiagnostic>, Vec<String>) {
    let analysis = crate::analysis::analyze_document(source);
    let (sanctioned, _) = check::fonts_origins(&analysis.config.fonts);
    let parsed = match parse_front_matter(source) {
        Ok(parsed) => parsed,
        Err(_) => return (true, Vec::new(), sanctioned),
    };
    let body = parsed.body.to_owned();
    let line_offset = source[..parsed.body_offset]
        .bytes()
        .filter(|&byte| byte == b'\n')
        .count();
    match build::guard_document(&analysis, source, &body, line_offset) {
        Ok(()) => (true, Vec::new(), sanctioned),
        Err(error) => (false, vec![build_error_diagnostic(&error)], sanctioned),
    }
}

/// The renderer's fixed emitted element set; anything else in the artifact
/// markup is a mutation the toolchain could not have produced.
const EMITTED_ELEMENTS: [&str; 10] = [
    "html", "head", "body", "meta", "title", "link", "style", "div", "noscript", "script",
];

/// The fixed attribute set per emitted element. Any attribute outside the
/// set is denied (E-MDHSEC-003); `on*` attributes are executable handlers
/// (E-MDHSEC-001) and take precedence.
fn allowed_attributes(name: &str) -> &'static [&'static str] {
    match name {
        "html" => &["lang", "data-mdhtml", "data-mdhtml-portable", "data-mdhtml-safe"],
        "meta" => &["charset", "name", "content", "property", "http-equiv"],
        "link" => &["rel", "href"],
        "style" | "div" => &["id"],
        "script" => &["id", "type", "data-path", "data-type"],
        _ => &[],
    }
}

/// The structural markup scan: any `on*` attribute (E-MDHSEC-001), any
/// element outside the fixed emitted set (E-MDHSEC-002), any denied
/// attribute (E-MDHSEC-003) and any script beyond
/// `#mdhtml-source`/`#mdhtml-runtime`/`application/octet-stream` asset
/// blocks fails the "no unauthorized executable content" check.
fn scan_markup(elements: &[check::Element<'_>]) -> (bool, Vec<AuditDiagnostic>) {
    let mut pass = true;
    let mut diagnostics = Vec::new();
    for element in elements {
        let name = element.name.to_ascii_lowercase();
        if !EMITTED_ELEMENTS.contains(&name.as_str()) {
            pass = false;
            diagnostics.push(AuditDiagnostic::new(
                "E-MDHSEC-002",
                format!("element <{name}> is outside the renderer's fixed emitted set"),
            ));
            continue;
        }
        for (attr_name, _) in &element.attrs {
            let attribute = attr_name.to_ascii_lowercase();
            if attribute.starts_with("on") {
                pass = false;
                diagnostics.push(AuditDiagnostic::new(
                    "E-MDHSEC-001",
                    format!("event handler attribute {attr_name:?} is forbidden"),
                ));
            } else if !allowed_attributes(&name).contains(&attribute.as_str()) {
                pass = false;
                diagnostics.push(AuditDiagnostic::new(
                    "E-MDHSEC-003",
                    format!("attribute {attr_name:?} is forbidden on <{name}>"),
                ));
            }
        }
        if name == "script" && !is_allowed_script(element) {
            pass = false;
            diagnostics.push(AuditDiagnostic::new(
                "E-MDHSEC-002",
                "script elements are limited to #mdhtml-source, #mdhtml-runtime \
                 and application/octet-stream asset blocks",
            ));
        }
    }
    (pass, diagnostics)
}

/// Whether a script element is one of the three kinds the toolchain emits.
fn is_allowed_script(element: &check::Element<'_>) -> bool {
    let id = check::attr(element, "id");
    let ty = check::attr(element, "type");
    (id == Some("mdhtml-source") && ty == Some("text/markdown"))
        || id == Some("mdhtml-runtime")
        || (ty == Some("application/octet-stream") && check::attr(element, "data-path").is_some())
}

/// The css category: the embedded `style#mdhtml-user` block must re-pass
/// `guard_author_css` (E-MDHSEC-007..010), and no style block may carry a
/// network `url()` outside the sanctioned fonts origins (E-MDHSEC-009). The
/// `mdhtml-tokens` block is a custom-property declaration list, never a
/// stylesheet — it is scanned for `url()` origins, not parsed.
fn css_check(
    elements: &[check::Element<'_>],
    sanctioned: &[String],
) -> (bool, Vec<AuditDiagnostic>) {
    let mut pass = true;
    let mut diagnostics = Vec::new();
    let mut user_block_flagged = false;
    for element in elements.iter().filter(|element| element.is("style")) {
        if check::attr(element, "id") == Some("mdhtml-user") {
            if let Some(text) = element.text {
                match guard_author_css(text) {
                    Ok(_) => {}
                    Err(violation) => {
                        pass = false;
                        user_block_flagged = true;
                        diagnostics.push(css_violation_diagnostic(&violation, text));
                    }
                }
            }
        }
    }
    for element in elements.iter().filter(|element| element.is("style")) {
        if user_block_flagged && check::attr(element, "id") == Some("mdhtml-user") {
            continue;
        }
        let Some(text) = element.text else {
            continue;
        };
        let mut origins = Vec::new();
        check::collect_css_url_origins(text, &mut origins);
        for origin in origins {
            if !sanctioned.iter().any(|candidate| candidate == &origin) {
                pass = false;
                diagnostics.push(network_url_diagnostic(text, &origin));
            }
        }
    }
    (pass, diagnostics)
}

/// The located diagnostic for a CSS guard violation, citing the embedded
/// style block text.
fn css_violation_diagnostic(violation: &Violation, css: &str) -> AuditDiagnostic {
    build_error_diagnostic(&build::security_error(violation.clone(), css, None))
}

/// The located diagnostic for a network `url()` in a non-user style block,
/// citing the origin within the block text.
fn network_url_diagnostic(css: &str, origin: &str) -> AuditDiagnostic {
    let violation = match build::locate_in_source(css, origin) {
        Some((line, column)) => Violation::new(
            "E-MDHSEC-009",
            "style blocks must not reference network URLs outside the sanctioned fonts origins",
        )
        .at(line, column),
        None => Violation::new(
            "E-MDHSEC-009",
            "style blocks must not reference network URLs outside the sanctioned fonts origins",
        ),
    };
    build_error_diagnostic(&build::security_error(violation, css, Some(origin)))
}

/// The runtime category: recompute the SHA-256 of the embedded
/// `script#mdhtml-runtime` bytes and compare the artifact CSP against the
/// assembly the stored source requires. A missing or structurally
/// contradictory CSP is E-MDHSEC-016; a well-formed CSP that pins a
/// different hash than the artifact's own runtime is E-MDHSEC-015. A missing
/// runtime script was already reported by the structure check.
fn runtime_check(
    elements: &[check::Element<'_>],
    fonts_url: Option<&str>,
) -> (bool, Vec<AuditDiagnostic>) {
    let runtime = elements
        .iter()
        .find(|element| element.is("script") && check::attr(element, "id") == Some("mdhtml-runtime"));
    let csp = elements
        .iter()
        .filter(|element| element.is("meta"))
        .find(|element| {
            check::attr(element, "http-equiv")
                .is_some_and(|value| value.eq_ignore_ascii_case("Content-Security-Policy"))
        })
        .and_then(|element| check::attr(element, "content"));
    let Some(runtime_text) = runtime.and_then(|element| element.text) else {
        return (false, Vec::new());
    };
    let hash = crate::selection::sha256::digest_base64(runtime_text.as_bytes());
    let expected = match fonts_url {
        Some(url) => build::assets::relaxed_csp(url, &hash),
        None => build::canonical_csp(&hash),
    };
    match csp {
        None => (
            false,
            vec![AuditDiagnostic::new(
                "E-MDHSEC-016",
                "the artifact must carry a Content-Security-Policy meta",
            )],
        ),
        Some(actual) if actual == expected => (true, Vec::new()),
        Some(actual) if csp_matches_family(actual, fonts_url) => (
            false,
            vec![AuditDiagnostic::new(
                "E-MDHSEC-015",
                "the CSP pins a different runtime hash than the embedded runtime",
            )],
        ),
        Some(_) => (
            false,
            vec![AuditDiagnostic::new(
                "E-MDHSEC-016",
                "the CSP contradicts the expected assembly",
            )],
        ),
    }
}

/// Whether the artifact CSP is structurally the expected family (canonical,
/// or relaxed for the stored source's `fonts.url`) with the runtime hash
/// replaced by a placeholder — i.e. well formed but pinning a different
/// hash than the embedded runtime.
fn csp_matches_family(actual: &str, fonts_url: Option<&str>) -> bool {
    const PLACEHOLDER: &str = "HASH";
    let expected = match fonts_url {
        Some(url) => build::assets::relaxed_csp(url, PLACEHOLDER),
        None => build::canonical_csp(PLACEHOLDER),
    };
    let mut normalized = actual.to_string();
    if let Some(start) = actual.find("sha256-") {
        let after = &actual[start + "sha256-".len()..];
        let end = after.find('\'').unwrap_or(after.len());
        if end > 0 {
            normalized.replace_range(
                start + "sha256-".len()..start + "sha256-".len() + end,
                PLACEHOLDER,
            );
        }
    }
    normalized == expected
}

/// One guard BuildError as an audit diagnostic: the frozen code and the
/// located message without the CLI prefix.
fn build_error_diagnostic(error: &BuildError) -> AuditDiagnostic {
    match error {
        BuildError::Build { code, message } => AuditDiagnostic {
            code,
            message: message.clone(),
        },
        BuildError::Cli(_) => unreachable!("guard errors are never CLI errors"),
    }
}
