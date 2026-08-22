//! Body-shape validation for bound section components over the accepted
//! shallow classifier. Each component has one shape predicate with exact
//! parity to `renderSectionComponent` and the shared table/kv predicates in
//! runtime/src/render.js; a failed binding emits exactly one `W-COMP-02`
//! carrying the component name and the bound slug.

use std::ops::Range;

use crate::scanner::ContainerEvidence;

use super::shape::{self, ShapeBlock, ShapeSummary};
use super::{AnalyzedSection, Diagnostic, PendingBinding};

/// Classifies the bound section body and reports whether the component shape
/// holds; on mismatch pushes one `W-COMP-02` with component name and slug.
pub(super) fn validate(
    binding: &PendingBinding,
    body: &str,
    sections: &[AnalyzedSection],
    containers: &[ContainerEvidence<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let section = &sections[binding.section_index];
    let body_slice = &body[section.body_range.clone()];
    let nested: Vec<Range<usize>> = containers
        .iter()
        .filter(|candidate| {
            candidate.offset >= section.body_range.start
                && candidate.offset < section.body_range.end
        })
        .map(|candidate| {
            let start = candidate.offset - section.body_range.start;
            let end = candidate.body_range.end - section.body_range.start;
            start..end
        })
        .collect();
    let summary = shape::classify(body_slice, &nested);
    let valid = match binding.component.as_str() {
        "timeline" => timeline_valid(&summary),
        "cards" => cards_valid(&summary),
        "meters" => meters_valid(&summary),
        "gallery" => gallery_valid(&summary),
        "kv" => kv_valid(&summary),
        "columns" => columns_valid(&summary),
        "hero" => hero_valid(&summary),
        _ => false,
    };
    if valid {
        return true;
    }
    diagnostics.push(
        Diagnostic::warning("W-COMP-02", shape_message(binding.component.as_str()))
            .with_name(binding.component.clone())
            .with_target(binding.slug.clone()),
    );
    false
}

fn shape_message(component: &str) -> &'static str {
    match component {
        "timeline" => "section body must be a single nonempty list",
        "cards" => "section body must contain only child headings",
        "meters" => "section body must be a two-column table with values from 0 through 100",
        "gallery" => "section body must contain only standalone image paragraphs",
        "kv" => "section body must be a two-column table or a strong-key list",
        "columns" => "section body needs at least two blocks",
        "hero" => "section body must be nonempty with at most one standalone image paragraph",
        _ => "section body does not match the component shape",
    }
}

fn timeline_valid(summary: &ShapeSummary<'_>) -> bool {
    matches!(
        summary.blocks.as_slice(),
        [ShapeBlock::List(list)]
            if !list.items.is_empty() && list.items.iter().all(|item| item.nonempty)
    )
}

/// Cards: nonempty body whose first top-level block is an ATX heading; a
/// section body range never contains headings at or above the section level,
/// so this equals no leading top-level content before the child sections.
fn cards_valid(summary: &ShapeSummary<'_>) -> bool {
    matches!(summary.blocks.first(), Some(ShapeBlock::Heading(_)))
}

fn meters_valid(summary: &ShapeSummary<'_>) -> bool {
    match summary.blocks.as_slice() {
        [ShapeBlock::Table(table)] => {
            two_column_table(table)
                && table
                    .rows
                    .iter()
                    .all(|row| shape::parse_bar_value(row[1]).is_some_and(|value| value <= 100.0))
        }
        _ => false,
    }
}

fn gallery_valid(summary: &ShapeSummary<'_>) -> bool {
    !summary.blocks.is_empty()
        && summary
            .blocks
            .iter()
            .all(|block| matches!(block, ShapeBlock::Paragraph { image_only: true }))
}

fn kv_valid(summary: &ShapeSummary<'_>) -> bool {
    match summary.blocks.as_slice() {
        [ShapeBlock::Table(table)] => two_column_table(table),
        [ShapeBlock::List(list)] => {
            !list.ordered
                && !list.items.is_empty()
                && list.items.iter().all(|item| {
                    !item.task && item.nonempty && item.first_is_paragraph && item.kv_prefix_valid
                })
        }
        _ => false,
    }
}

fn columns_valid(summary: &ShapeSummary<'_>) -> bool {
    summary.blocks.len() >= 2
}

fn hero_valid(summary: &ShapeSummary<'_>) -> bool {
    !summary.blocks.is_empty()
        && summary
            .blocks
            .iter()
            .filter(|block| matches!(block, ShapeBlock::Paragraph { image_only: true }))
            .count()
            <= 1
}

/// The accepted structured-table predicate: header with two source cells,
/// at least one body row, every row with two source cells.
fn two_column_table(table: &shape::TableShape<'_>) -> bool {
    table.header.len() == 2 && !table.rows.is_empty() && table.rows.iter().all(|row| row.len() == 2)
}
