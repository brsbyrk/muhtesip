//! Reading a project's configuration document into the data the registry and the caller apply.

use globset::{Glob, GlobMatcher};
use regex::Regex;
use yaml_rust2::{Yaml, YamlLoader};

use crate::Error;
use crate::rule::{Finding, Severity};
use crate::settings::{RuleOverride, RuleSettings};

/// The top-level key naming per-rule adjustments.
const RULES_KEY: &str = "rules";
/// The top-level key naming per-path ignore blocks.
const PATHS_KEY: &str = "paths";
/// The setting names inside a `rules.<id>` mapping.
const SEVERITY_KEY: &str = "severity";
/// The setting name that turns a rule off or on.
const ENABLED_KEY: &str = "enabled";
/// The setting name inside a `paths.<glob>` mapping.
const IGNORE_KEY: &str = "ignore";

/// A parsed project configuration: per-rule adjustments, and ignores scoped to workflow paths.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Per-rule adjustments, applied to whichever registry runs.
    rules: RuleSettings,
    /// Ignores scoped to the workflow paths a glob matches.
    paths: Vec<PathIgnores>,
}

/// Ignores that apply to the workflows a glob matches.
#[derive(Debug, Clone)]
struct PathIgnores {
    /// The glob selecting the workflow paths these ignores apply to.
    glob: GlobMatcher,
    /// The regular expressions a finding's rendered line must match to be ignored.
    patterns: Vec<Regex>,
}

impl Config {
    /// A starter configuration document: every rule this build ships, at the state it already has.
    ///
    /// Generated from the registry rather than written by hand, so it cannot describe a rule the
    /// build does not have or miss one it does. A test parses the result back, because a template
    /// the strict reader rejects would be worse than no template.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{Config, Registry};
    ///
    /// let document = Config::template(&Registry::all());
    /// assert!(document.contains("unpinned-action:"));
    ///
    /// // The point of the file is that the tool can read it back.
    /// Config::parse(&document).unwrap();
    /// ```
    pub fn template(registry: &crate::Registry) -> String {
        let mut ids: Vec<&str> = registry.metas().iter().map(|meta| meta.id).collect();
        ids.sort_unstable();

        let mut out = String::from(
            "# muhtesip configuration.\n\
             #\n\
             # Every rule this build ships, with the state it already has. Escalate or relax a rule by\n\
             # setting its severity (error, warning, note), or turn it off with `enabled: false`.\n\n\
             rules:\n",
        );
        for id in ids {
            let severity = registry
                .meta(id)
                .map(|meta| meta.severity.name())
                .unwrap_or_default();
            out.push_str(&format!("  {id}:\n    severity: {severity}\n"));
        }
        out.push_str(
            "\n# Ignores scoped to a workflow path, matched by glob against the source path:\n\
             # paths:\n\
             #   \"**/legacy.yaml\":\n\
             #     ignore:\n\
             #       - \"not pinned to an immutable commit\"\n",
        );
        out
    }

    /// Parse a configuration document.
    ///
    /// Strict on purpose: this is user input, so a misspelt key, an unknown rule id, a bad glob, or
    /// a bad regular expression is reported rather than ignored. (The in-memory
    /// [`Registry::configure`](crate::Registry::configure) stays permissive — an unknown id there
    /// is simply inert.)
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::Config;
    ///
    /// let config = Config::parse("rules:\n  missing-timeout:\n    enabled: false\n").unwrap();
    /// assert!(!config.settings().is_empty());
    ///
    /// // A misspelt key is refused rather than ignored.
    /// assert!(Config::parse("rules:\n  missing-timeout:\n    colour: red\n").is_err());
    /// ```
    pub fn parse(text: &str) -> Result<Self, Error> {
        let documents =
            YamlLoader::load_from_str(text).map_err(|error| Error::Config(error.to_string()))?;
        let Some(root) = documents.first() else {
            return Ok(Self::default());
        };

        let Some(top) = root.as_hash() else {
            return Err(Error::Config("the document must be a mapping".to_owned()));
        };

        let mut config = Self::default();
        for (key, value) in top {
            let Some(name) = key.as_str() else {
                return Err(Error::Config("top-level keys must be strings".to_owned()));
            };
            if name == RULES_KEY {
                config.rules = parse_rules(value)?;
            } else if name == PATHS_KEY {
                config.paths = parse_paths(value)?;
            } else {
                return Err(Error::Config(format!("unknown top-level key '{name}'")));
            }
        }

        Ok(config)
    }

    /// The per-rule adjustments the document declares.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::Config;
    ///
    /// let config = Config::parse("rules:\n  id:\n    severity: note\n").unwrap();
    /// assert_eq!(config.settings().ids(), vec!["id"]);
    /// ```
    pub fn settings(&self) -> &RuleSettings {
        &self.rules
    }

    /// Whether a finding about `path` is suppressed by the document.
    ///
    /// A path is a string the caller supplies (its own spelling of the file); a glob matches it as
    /// given, so a caller that passes an absolute path must write its globs accordingly.
    pub fn is_ignored(&self, path: &str, finding: &Finding) -> bool {
        self.paths.iter().any(|entry| {
            entry.glob.is_match(path)
                && entry
                    .patterns
                    .iter()
                    .any(|pattern| pattern.is_match(&finding.message))
        })
    }
}

/// Parse the `rules` mapping.
fn parse_rules(value: &Yaml) -> Result<RuleSettings, Error> {
    let Some(table) = value.as_hash() else {
        return Err(Error::Config(format!("'{RULES_KEY}' must be a mapping")));
    };

    let mut rules = RuleSettings::new();
    for (rule_key, rule_value) in table {
        let Some(id) = rule_key.as_str() else {
            return Err(Error::Config("rule ids must be strings".to_owned()));
        };
        rules = rules.with(id, parse_adjustment(id, rule_value)?);
    }
    Ok(rules)
}

/// Parse one `rules.<id>` mapping.
fn parse_adjustment(id: &str, value: &Yaml) -> Result<RuleOverride, Error> {
    let Some(table) = value.as_hash() else {
        return Err(Error::Config(format!("rules.{id} must be a mapping")));
    };

    let mut adjustment = RuleOverride::default();
    for (key, value) in table {
        let Some(name) = key.as_str() else {
            return Err(Error::Config(format!("rules.{id}: keys must be strings")));
        };

        if name == SEVERITY_KEY {
            let text = value.as_str().ok_or_else(|| {
                Error::Config(format!("rules.{id}.{SEVERITY_KEY} must be a string"))
            })?;
            let severity = Severity::from_name(text).ok_or_else(|| {
                Error::Config(format!(
                    "rules.{id}.{SEVERITY_KEY}: unknown severity '{text}'"
                ))
            })?;
            adjustment.severity = Some(severity);
        } else if name == ENABLED_KEY {
            let on = value.as_bool().ok_or_else(|| {
                Error::Config(format!("rules.{id}.{ENABLED_KEY} must be a boolean"))
            })?;
            adjustment.enabled = Some(on);
        } else {
            return Err(Error::Config(format!(
                "rules.{id}: unknown setting '{name}'"
            )));
        }
    }

    Ok(adjustment)
}

/// Parse the `paths` mapping: a glob to the ignores that apply to what it matches.
fn parse_paths(value: &Yaml) -> Result<Vec<PathIgnores>, Error> {
    let Some(table) = value.as_hash() else {
        return Err(Error::Config(format!("'{PATHS_KEY}' must be a mapping")));
    };

    let mut entries = Vec::new();
    for (glob_key, value) in table {
        let Some(pattern) = glob_key.as_str() else {
            return Err(Error::Config(format!("{PATHS_KEY}: keys must be strings")));
        };
        let glob = Glob::new(pattern)
            .map_err(|error| Error::Config(format!("paths.{pattern}: invalid glob: {error}")))?
            .compile_matcher();

        let Some(settings) = value.as_hash() else {
            return Err(Error::Config(format!("paths.{pattern} must be a mapping")));
        };

        let mut patterns = Vec::new();
        for (key, value) in settings {
            let Some(name) = key.as_str() else {
                return Err(Error::Config(format!(
                    "paths.{pattern}: keys must be strings"
                )));
            };
            if name != IGNORE_KEY {
                return Err(Error::Config(format!(
                    "paths.{pattern}: unknown setting '{name}'"
                )));
            }
            let Some(list) = value.as_vec() else {
                return Err(Error::Config(format!(
                    "paths.{pattern}.{IGNORE_KEY} must be a list"
                )));
            };
            for item in list {
                let Some(text) = item.as_str() else {
                    return Err(Error::Config(format!(
                        "paths.{pattern}.{IGNORE_KEY}: entries must be strings"
                    )));
                };
                patterns.push(Regex::new(text).map_err(|error| {
                    Error::Config(format!(
                        "paths.{pattern}.{IGNORE_KEY}: invalid regex '{text}': {error}"
                    ))
                })?);
            }
        }

        entries.push(PathIgnores { glob, patterns });
    }

    Ok(entries)
}
