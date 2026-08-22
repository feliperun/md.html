//! Closed normalization of the reserved front matter keys (SPEC §8) from the
//! parsed `frontmatter::Value`, without coercion. Invalid values fall back to
//! the SPEC defaults and emit one stable diagnostic per condition.

use crate::frontmatter::Value;

use super::{Diagnostic, Fonts, NormalizedConfig, Theme, Toc, TocPosition, TocSetting};

fn mapping(value: &Value) -> Option<&[(String, Value)]> {
    match value {
        Value::Mapping(entries) => Some(entries),
        _ => None,
    }
}

fn get<'a>(entries: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    entries
        .iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value)
}

fn as_string(value: &Value) -> Option<&str> {
    match value {
        Value::String(text) => Some(text),
        _ => None,
    }
}

fn is_integer(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) if number.fract() == 0.0 && number.is_finite() => {
            Some(*number as i64)
        }
        _ => None,
    }
}

fn warn(diagnostics: &mut Vec<Diagnostic>, message: impl Into<String>) {
    diagnostics.push(Diagnostic::warning("W-CONFIG-01", message));
}

pub(super) fn normalize(value: &Value, diagnostics: &mut Vec<Diagnostic>) -> NormalizedConfig {
    let mut config = NormalizedConfig::default();
    let Some(entries) = mapping(value) else {
        return config;
    };

    normalize_title(entries, &mut config, diagnostics);
    normalize_summary(entries, &mut config, diagnostics);
    normalize_lang(entries, &mut config, diagnostics);
    normalize_theme(entries, &mut config, diagnostics);
    normalize_tokens(entries, &mut config, diagnostics);
    config.fonts = normalize_fonts(get(entries, "fonts"), diagnostics);
    normalize_url(entries, &mut config, diagnostics);
    normalize_cover(entries, &mut config, diagnostics);
    config.toc = normalize_toc(get(entries, "toc"), diagnostics);
    normalize_sections(entries, &mut config, diagnostics);
    normalize_figures(entries, &mut config, diagnostics);

    config
}

fn normalize_title(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "title") {
        Some(Value::String(title)) if !title.is_empty() => {
            config.title = Some(title.clone());
        }
        _ => diagnostics.push(Diagnostic::error(
            "E-FMT-05",
            "front matter title is required and must be a nonempty string",
        )),
    }
}

fn normalize_summary(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "summary") {
        Some(Value::String(summary)) => config.summary = Some(summary.clone()),
        Some(_) => warn(diagnostics, "config key summary must be a string; ignored"),
        None => {}
    }
}

fn normalize_lang(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "lang") {
        Some(Value::String(lang)) => config.lang = Some(lang.clone()),
        Some(_) => warn(diagnostics, "config key lang must be a string; ignored"),
        None => {}
    }
}

fn normalize_theme(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "theme") {
        None => {}
        Some(Value::String(theme)) => match theme.as_str() {
            "technical" => config.theme = Theme::Technical,
            "editorial" => config.theme = Theme::Editorial,
            name if name.ends_with(".theme.css") => {
                config.theme = Theme::Local(name.to_string());
            }
            _ => {
                warn(
                    diagnostics,
                    "config key theme names an unknown preset; using technical",
                );
                config.theme = Theme::Technical;
            }
        },
        Some(_) => {
            warn(
                diagnostics,
                "config key theme must be a string; using technical",
            );
            config.theme = Theme::Technical;
        }
    }
}

fn normalize_tokens(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "tokens") {
        None => {}
        Some(value) if mapping(value).is_some() => config.tokens = value.clone(),
        Some(_) => warn(diagnostics, "config key tokens must be a mapping; ignored"),
    }
}

fn normalize_url(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "url") {
        Some(Value::String(url)) => config.url = Some(url.clone()),
        Some(_) => warn(diagnostics, "config key url must be a string; ignored"),
        None => {}
    }
}

fn normalize_cover(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "cover") {
        Some(Value::String(cover)) => config.cover = Some(cover.clone()),
        Some(_) => warn(diagnostics, "config key cover must be a string; ignored"),
        None => {}
    }
}

fn normalize_sections(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "sections") {
        None => {}
        Some(value) if mapping(value).is_some() => config.sections = value.clone(),
        Some(_) => warn(
            diagnostics,
            "config key sections must be a mapping; ignored",
        ),
    }
}

fn normalize_figures(
    entries: &[(String, Value)],
    config: &mut NormalizedConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match get(entries, "figures") {
        None => {}
        Some(value) if mapping(value).is_some() => config.figures = value.clone(),
        Some(_) => warn(diagnostics, "config key figures must be a mapping; ignored"),
    }
}

fn normalize_fonts(value: Option<&Value>, diagnostics: &mut Vec<Diagnostic>) -> Fonts {
    let Some(value) = value else {
        return Fonts::Auto;
    };
    match value {
        Value::String(policy) if policy == "auto" => Fonts::Auto,
        Value::String(policy) if policy == "system" => Fonts::System,
        Value::Mapping(entries) => {
            let mut fonts = Fonts::Map {
                body: None,
                mono: None,
                url: None,
            };
            for (key, value) in entries {
                match key.as_str() {
                    "body" => match as_string(value) {
                        Some(path) => {
                            if let Fonts::Map { body, .. } = &mut fonts {
                                *body = Some(path.to_string());
                            }
                        }
                        None => warn(
                            diagnostics,
                            "config key fonts.body must be a string; ignored",
                        ),
                    },
                    "mono" => match as_string(value) {
                        Some(path) => {
                            if let Fonts::Map { mono, .. } = &mut fonts {
                                *mono = Some(path.to_string());
                            }
                        }
                        None => warn(
                            diagnostics,
                            "config key fonts.mono must be a string; ignored",
                        ),
                    },
                    "url" => match as_string(value) {
                        Some(url) => {
                            if let Fonts::Map { url: slot, .. } = &mut fonts {
                                *slot = Some(url.to_string());
                            }
                        }
                        None => warn(
                            diagnostics,
                            "config key fonts.url must be a string; ignored",
                        ),
                    },
                    other => warn(
                        diagnostics,
                        format!("config key fonts contains unknown key {other}; ignored"),
                    ),
                }
            }
            fonts
        }
        _ => {
            warn(
                diagnostics,
                "config key fonts must be auto, system, or a mapping; using auto",
            );
            Fonts::Auto
        }
    }
}

fn normalize_toc(value: Option<&Value>, diagnostics: &mut Vec<Diagnostic>) -> TocSetting {
    let Some(value) = value else {
        return TocSetting::Enabled(Toc {
            depth: 3,
            position: TocPosition::Side,
        });
    };
    match value {
        Value::Bool(false) => TocSetting::Disabled,
        Value::Mapping(entries) => {
            let mut depth = 3;
            let mut position = TocPosition::Side;
            let mut valid = true;
            for (key, value) in entries {
                match key.as_str() {
                    "depth" => match is_integer(value) {
                        Some(integer) if (1..=6).contains(&integer) => depth = integer as u8,
                        _ => {
                            valid = false;
                            warn(
                                diagnostics,
                                "config key toc.depth must be an integer from 1 to 6; using default",
                            );
                        }
                    },
                    "position" => match as_string(value) {
                        Some("side") => position = TocPosition::Side,
                        Some("inline") => position = TocPosition::Inline,
                        _ => {
                            valid = false;
                            warn(
                                diagnostics,
                                "config key toc.position must be side or inline; using default",
                            );
                        }
                    },
                    other => {
                        valid = false;
                        warn(
                            diagnostics,
                            format!("config key toc contains unknown key {other}; ignored"),
                        );
                    }
                }
            }
            if valid {
                TocSetting::Enabled(Toc { depth, position })
            } else {
                TocSetting::Enabled(Toc {
                    depth: 3,
                    position: TocPosition::Side,
                })
            }
        }
        _ => {
            warn(
                diagnostics,
                "config key toc must be false or a mapping; using default",
            );
            TocSetting::Enabled(Toc {
                depth: 3,
                position: TocPosition::Side,
            })
        }
    }
}
