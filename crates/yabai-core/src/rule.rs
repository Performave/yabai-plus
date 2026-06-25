//! The `rule` domain data model, ported from `src/rule.{h,c}` and the
//! `parse_rule` validator in `src/message.c`.
//!
//! This layer is pure: it parses, stores, and serializes rules. Regex
//! compilation/matching against live windows and applying effects (manage, etc.)
//! are the runtime/daemon's job. `display`/`space` selectors are stored verbatim
//! (their resolution needs live managers) and serialized as `0` for now.

use crate::parser::KeyValue;

/// A window sub-layer level (`sub-layer` rule key), mirroring `enum layer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Below,
    Normal,
    Above,
    Auto,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Below => "below",
            Layer::Normal => "normal",
            Layer::Above => "above",
            Layer::Auto => "auto",
        }
    }
}

/// The effects a rule applies to a matching window. Mirrors `struct rule_effects`;
/// `Option`/`None` corresponds to the C `RULE_PROP_UD` (unset) sentinel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleEffects {
    pub manage: Option<bool>,
    pub sticky: Option<bool>,
    pub mff: Option<bool>,
    pub fullscreen: Option<bool>,
    pub opacity: Option<f32>,
    pub layer: Option<Layer>,
    pub grid: Option<[u32; 6]>,
    pub scratchpad: Option<String>,
    /// Raw `display`/`space` selector strings (resolution deferred to the daemon).
    pub display: Option<String>,
    pub space: Option<String>,
    pub follow_space: bool,
}

/// A registered window rule: filters (each an optional regex pattern plus an
/// exclusion flag) and the effects to apply. Patterns are stored verbatim;
/// compilation happens in the runtime.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rule {
    pub label: Option<String>,
    pub app: Option<String>,
    pub app_exclude: bool,
    pub title: Option<String>,
    pub title_exclude: bool,
    pub role: Option<String>,
    pub role_exclude: bool,
    pub subrole: Option<String>,
    pub subrole_exclude: bool,
    pub effects: RuleEffects,
    pub one_shot: bool,
}

impl Rule {
    /// Build a rule from `rule --add` key-values (and the `--one-shot` flag),
    /// mirroring `parse_rule`: at least one of `app`/`title`/`role`/`subrole` is
    /// required, the `on|off` keys and `opacity`/`grid`/`sub-layer` value domains
    /// are validated, and `!` (exclusion) is only allowed on the filter keys.
    /// Reproduces the C `daemon_fail` text. `display`/`space` are accepted and
    /// stored raw (a leading `^` sets `follow_space`); their selector resolution
    /// is deferred to the daemon.
    pub fn from_key_values(pairs: &[KeyValue], one_shot: bool) -> Result<Rule, String> {
        let mut rule = Rule {
            one_shot,
            ..Rule::default()
        };
        let mut has_filter = false;
        let mut unsupported_exclusion: Option<&str> = None;

        for KeyValue {
            key,
            value,
            exclusion,
        } in pairs
        {
            let excl = *exclusion;
            match key.as_str() {
                "label" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.label = Some(value.clone());
                }
                "app" => {
                    has_filter = true;
                    rule.app = Some(value.clone());
                    rule.app_exclude = excl;
                }
                "title" => {
                    has_filter = true;
                    rule.title = Some(value.clone());
                    rule.title_exclude = excl;
                }
                "role" => {
                    has_filter = true;
                    rule.role = Some(value.clone());
                    rule.role_exclude = excl;
                }
                "subrole" => {
                    has_filter = true;
                    rule.subrole = Some(value.clone());
                    rule.subrole_exclude = excl;
                }
                "manage" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.effects.manage = Some(parse_on_off(value, key)?);
                }
                "sticky" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.effects.sticky = Some(parse_on_off(value, key)?);
                }
                "mouse_follows_focus" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.effects.mff = Some(parse_on_off(value, key)?);
                }
                "native-fullscreen" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.effects.fullscreen = Some(parse_on_off(value, key)?);
                }
                "opacity" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    let opacity = value
                        .parse::<f32>()
                        .ok()
                        .filter(|o| (0.0..=1.0).contains(o))
                        .ok_or_else(|| invalid_value(value, key))?;
                    rule.effects.opacity = Some(opacity);
                }
                "sub-layer" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.effects.layer = Some(match value.as_str() {
                        "below" => Layer::Below,
                        "normal" => Layer::Normal,
                        "above" => Layer::Above,
                        "auto" => Layer::Auto,
                        _ => return Err(invalid_value(value, key)),
                    });
                }
                "grid" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.effects.grid = Some(parse_grid(value, key)?);
                }
                "scratchpad" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    // The C rejects reserved identifiers and forces manage=off.
                    rule.effects.scratchpad = Some(value.clone());
                    rule.effects.manage = Some(false);
                }
                "display" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.effects.display = Some(strip_follow_space(value, &mut rule.effects));
                }
                "space" => {
                    if excl {
                        unsupported_exclusion = Some(key);
                    }
                    rule.effects.space = Some(strip_follow_space(value, &mut rule.effects));
                }
                _ => return Err(format!("unknown key '{key}'\n")),
            }
        }

        if !has_filter {
            return Err(
                "missing required key-value pair 'app[!]=..' or 'title[!]=..'\n".to_string(),
            );
        }
        if let Some(key) = unsupported_exclusion {
            return Err(format!(
                "unsupported token '!' (exclusion) given for key '{key}'\n"
            ));
        }

        Ok(rule)
    }
}

fn parse_on_off(value: &str, key: &str) -> Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(invalid_value(value, key)),
    }
}

fn parse_grid(value: &str, key: &str) -> Result<[u32; 6], String> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 6 {
        return Err(invalid_value(value, key));
    }
    let mut grid = [0u32; 6];
    for (slot, part) in grid.iter_mut().zip(parts) {
        *slot = part.parse::<u32>().map_err(|_| invalid_value(value, key))?;
    }
    Ok(grid)
}

/// A leading `^` on a `display`/`space` value sets `follow_space` and is stripped.
fn strip_follow_space(value: &str, effects: &mut RuleEffects) -> String {
    if let Some(rest) = value.strip_prefix('^') {
        effects.follow_space = true;
        rest.to_string()
    } else {
        value.to_string()
    }
}

fn invalid_value(value: &str, key: &str) -> String {
    format!("invalid value '{value}' for key '{key}'\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(key: &str, value: &str) -> KeyValue {
        KeyValue {
            key: key.to_string(),
            value: value.to_string(),
            exclusion: false,
        }
    }

    #[test]
    fn builds_a_rule_with_effects() {
        let rule = Rule::from_key_values(
            &[
                kv("app", "^Finder$"),
                kv("title", "Downloads"),
                kv("manage", "off"),
                kv("grid", "4:4:0:0:2:2"),
                kv("label", "fin"),
            ],
            true,
        )
        .unwrap();
        assert_eq!(rule.app.as_deref(), Some("^Finder$"));
        assert_eq!(rule.title.as_deref(), Some("Downloads"));
        assert_eq!(rule.effects.manage, Some(false));
        assert_eq!(rule.effects.grid, Some([4, 4, 0, 0, 2, 2]));
        assert_eq!(rule.label.as_deref(), Some("fin"));
        assert!(rule.one_shot);
    }

    #[test]
    fn requires_a_filter() {
        let err = Rule::from_key_values(&[kv("manage", "off")], false).unwrap_err();
        assert!(err.contains("missing required key-value pair 'app[!]=..' or 'title[!]=..'"));
    }

    #[test]
    fn validates_values_and_exclusion() {
        let bad_manage =
            Rule::from_key_values(&[kv("app", "x"), kv("manage", "maybe")], false).unwrap_err();
        assert!(bad_manage.contains("invalid value 'maybe' for key 'manage'"));

        let bad_grid =
            Rule::from_key_values(&[kv("app", "x"), kv("grid", "1:2:3")], false).unwrap_err();
        assert!(bad_grid.contains("invalid value '1:2:3' for key 'grid'"));

        let bad_opacity =
            Rule::from_key_values(&[kv("app", "x"), kv("opacity", "2.0")], false).unwrap_err();
        assert!(bad_opacity.contains("invalid value '2.0' for key 'opacity'"));

        let unknown =
            Rule::from_key_values(&[kv("app", "x"), kv("bogus", "y")], false).unwrap_err();
        assert!(unknown.contains("unknown key 'bogus'"));

        let excl = Rule::from_key_values(
            &[
                kv("app", "x"),
                KeyValue {
                    key: "manage".to_string(),
                    value: "off".to_string(),
                    exclusion: true,
                },
            ],
            false,
        )
        .unwrap_err();
        assert!(excl.contains("unsupported token '!' (exclusion) given for key 'manage'"));
    }

    #[test]
    fn space_caret_sets_follow_space() {
        let rule = Rule::from_key_values(&[kv("app", "x"), kv("space", "^2")], false).unwrap();
        assert_eq!(rule.effects.space.as_deref(), Some("2"));
        assert!(rule.effects.follow_space);
    }
}
