//! Ordered COMP-02 validation of fenced containers over scanner evidence.
//!
//! Every syntactically valid container is validated independently against its
//! own classified body; balanced nested containers count as one block and are
//! themselves validated in opener source order. Each failed unit produces one
//! `W-COMP-02` warning carrying the container name and a null target.

use std::ops::Range;

use crate::scanner::ContainerEvidence;

use super::Diagnostic;
use super::shape::{self, ShapeBlock, ShapeSummary};

const CALLOUTS: &[&str] = &["note", "warning", "critical", "success", "decision"];

pub(super) fn validate(
    containers: &[ContainerEvidence<'_>],
    body: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for container in containers {
        let body_slice = &body[container.body_range.clone()];
        let nested: Vec<Range<usize>> = containers
            .iter()
            .filter(|candidate| {
                candidate.offset >= container.body_range.start
                    && candidate.offset < container.body_range.end
            })
            .map(|candidate| {
                let start = candidate.offset - container.body_range.start;
                let end = candidate.body_range.end - container.body_range.start;
                start..end
            })
            .collect();
        validate_one(container, body_slice, &nested, diagnostics);
    }
}

fn warn(diagnostics: &mut Vec<Diagnostic>, name: &str, message: &str) {
    diagnostics.push(Diagnostic::warning("W-COMP-02", message).with_name(name.to_string()));
}

fn validate_one(
    container: &ContainerEvidence<'_>,
    body: &str,
    nested: &[Range<usize>],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = container.name;
    let argument = container.argument;
    let summary = shape::classify(body, nested);

    if CALLOUTS.contains(&name) {
        if argument.is_some() {
            warn(diagnostics, name, "container argument is not allowed");
        } else if summary.blocks.is_empty() {
            warn(diagnostics, name, "container body is empty");
        }
        return;
    }
    match name {
        "quote" | "details" => {
            if summary.blocks.is_empty() {
                warn(diagnostics, name, "container body is empty");
            }
        }
        "columns" => {
            if argument.is_some() {
                warn(diagnostics, name, "container argument is not allowed");
            } else if summary.blocks.len() < 2 {
                warn(
                    diagnostics,
                    name,
                    "container body needs at least two blocks",
                );
            }
        }
        "stats" | "bars" => validate_table(container, &summary, diagnostics, name == "bars"),
        "kv" => validate_kv(container, &summary, diagnostics),
        "steps" => {
            if argument.is_some() {
                warn(diagnostics, name, "container argument is not allowed");
            } else if !is_single_nonempty_ordered_list(&summary) {
                warn(
                    diagnostics,
                    name,
                    "container body must be a single nonempty ordered list",
                );
            }
        }
        "grid" => {
            if argument.is_some() {
                warn(diagnostics, name, "container argument is not allowed");
            } else if !matches!(summary.blocks.first(), Some(ShapeBlock::Heading(3))) {
                warn(
                    diagnostics,
                    name,
                    "container body must start with a level-3 heading",
                );
            }
        }
        _ => warn(diagnostics, name, "unknown container name"),
    }
}

fn single_table<'a>(summary: &'a ShapeSummary<'a>) -> Option<&'a shape::TableShape<'a>> {
    match summary.blocks.as_slice() {
        [ShapeBlock::Table(table)] => Some(table),
        _ => None,
    }
}

fn validate_table(
    container: &ContainerEvidence<'_>,
    summary: &ShapeSummary<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    bars: bool,
) {
    let name = container.name;
    if container.argument.is_some() {
        warn(diagnostics, name, "container argument is not allowed");
        return;
    }
    let Some(table) = single_table(summary) else {
        warn(
            diagnostics,
            name,
            "container body must be a single two-column table",
        );
        return;
    };
    if table.header.len() != 2 {
        warn(
            diagnostics,
            name,
            "container table header must have exactly two cells",
        );
        return;
    }
    if table.rows.is_empty() {
        warn(
            diagnostics,
            name,
            "container table needs at least one body row",
        );
        return;
    }
    if table.rows.iter().any(|row| row.len() != 2) {
        warn(
            diagnostics,
            name,
            "container table row must have exactly two cells",
        );
        return;
    }
    if bars
        && table
            .rows
            .iter()
            .any(|row| shape::parse_bar_value(row[1]).is_none())
    {
        warn(
            diagnostics,
            name,
            "container bar value must be a finite number not less than zero",
        );
    }
}

fn validate_kv(
    container: &ContainerEvidence<'_>,
    summary: &ShapeSummary<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = container.name;
    if container.argument.is_some() {
        warn(diagnostics, name, "container argument is not allowed");
        return;
    }
    match summary.blocks.as_slice() {
        [ShapeBlock::Table(table)] => {
            if table.header.len() != 2 {
                warn(
                    diagnostics,
                    name,
                    "container table header must have exactly two cells",
                );
            } else if table.rows.is_empty() {
                warn(
                    diagnostics,
                    name,
                    "container table needs at least one body row",
                );
            } else if table.rows.iter().any(|row| row.len() != 2) {
                warn(
                    diagnostics,
                    name,
                    "container table row must have exactly two cells",
                );
            }
        }
        [ShapeBlock::List(list)] => {
            if list.ordered {
                warn(diagnostics, name, "container kv list must be unordered");
            } else if list.items.is_empty() {
                warn(diagnostics, name, "container kv list must be nonempty");
            } else if list.items.iter().any(|item| {
                item.task || !item.nonempty || !item.first_is_paragraph || !item.kv_prefix_valid
            }) {
                warn(
                    diagnostics,
                    name,
                    "container kv item must start with a nonempty strong key",
                );
            }
        }
        _ => warn(
            diagnostics,
            name,
            "container body must be a two-column table or an unordered key list",
        ),
    }
}

fn is_single_nonempty_ordered_list(summary: &ShapeSummary<'_>) -> bool {
    match summary.blocks.as_slice() {
        [ShapeBlock::List(list)] => {
            list.ordered && !list.items.is_empty() && list.items.iter().all(|item| item.nonempty)
        }
        _ => false,
    }
}
