//! Ordered validation of `sections:` entries over the registered heading ids:
//! schema (mapping, scalar component, optional scalar class), CSS identifier
//! class list, known component name, and section-body shape. Orphans win as
//! E-SECT-01; schema, name and shape problems degrade with one W-COMP-02
//! record per entry, all in strict map source order.

use std::collections::HashMap;

use crate::frontmatter::Value;
use crate::scanner::ContainerEvidence;

use super::section_components;
use super::{AnalyzedSection, DegradedBinding, Diagnostic, PendingBinding};

const KNOWN_COMPONENTS: &[&str] = &[
    "timeline", "cards", "meters", "gallery", "kv", "columns", "hero",
];

fn mapping(value: &Value) -> Option<&[(String, Value)]> {
    match value {
        Value::Mapping(entries) => Some(entries),
        _ => None,
    }
}

fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Mapping(entries) => entries
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value),
        _ => None,
    }
}

/// SPEC class rule: one or more CSS identifiers separated only by whitespace.
fn valid_class(value: &str) -> bool {
    let mut chars = value.chars().peekable();
    if !consume_identifier(&mut chars) {
        return false;
    }
    loop {
        match chars.peek() {
            None => return true,
            Some(ch) if ch.is_whitespace() => {
                while chars.peek().is_some_and(|ch| ch.is_whitespace()) {
                    chars.next();
                }
                if !consume_identifier(&mut chars) {
                    return false;
                }
            }
            Some(_) => return false,
        }
    }
}

fn consume_identifier(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    match chars.peek() {
        Some(ch) if ch.is_ascii_alphabetic() || *ch == '_' => {
            chars.next();
        }
        _ => return false,
    }
    while let Some(ch) = chars.peek() {
        if ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_' {
            chars.next();
        } else {
            break;
        }
    }
    true
}

pub(super) fn validate(
    sections: &Value,
    id_to_index: &HashMap<String, usize>,
    analyzed: &[AnalyzedSection],
    body: &str,
    containers: &[ContainerEvidence<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<PendingBinding>, Vec<DegradedBinding>) {
    let mut bindings = Vec::new();
    let mut degraded = Vec::new();
    let Some(entries) = mapping(sections) else {
        return (bindings, degraded);
    };
    for (slug, raw) in entries {
        let Some(section_index) = id_to_index.get(slug) else {
            diagnostics.push(
                Diagnostic::error("E-SECT-01", "sections key has no matching heading slug")
                    .with_target(slug.clone()),
            );
            continue;
        };
        let is_mapping = mapping(raw).is_some();
        let component = match (is_mapping, get(raw, "component")) {
            (true, Some(Value::String(component))) => component.clone(),
            _ => String::new(),
        };
        let has_class = is_mapping && get(raw, "class").is_some();
        let class_is_string = matches!(get(raw, "class"), Some(Value::String(_)));
        let class_value = match (has_class, get(raw, "class")) {
            (true, Some(Value::String(class))) => Some(class.clone()),
            _ => None,
        };
        let shape_valid = is_mapping
            && (matches!(get(raw, "component"), Some(Value::String(_))))
            && (!has_class || class_is_string)
            && (!has_class || valid_class(class_value.as_deref().unwrap_or("")));
        let known = KNOWN_COMPONENTS.contains(&component.as_str());

        if shape_valid && known {
            let binding = PendingBinding {
                slug: slug.clone(),
                component: component.clone(),
                class: class_value,
                section_index: *section_index,
            };
            if section_components::validate(&binding, body, analyzed, containers, diagnostics) {
                bindings.push(binding);
            } else {
                degraded.push(DegradedBinding {
                    slug: binding.slug,
                    component: binding.component,
                });
            }
            continue;
        }

        let name = component.clone();
        let message = if !is_mapping {
            "section binding is not a mapping"
        } else if !matches!(get(raw, "component"), Some(Value::String(_))) || component.is_empty() {
            "section binding component must be a nonempty string"
        } else if !known {
            "unknown section component"
        } else {
            "section binding class must be a valid CSS identifier list"
        };
        diagnostics.push(
            Diagnostic::warning("W-COMP-02", message)
                .with_name(name)
                .with_target(slug.clone()),
        );
    }
    (bindings, degraded)
}
