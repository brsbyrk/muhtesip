//! A fast, embeddable linter for CI workflow files.
//!
//! The library reads a workflow document and returns findings. It never prints, exits,
//! or touches the filesystem.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::panic))]

mod config;
mod cron;
mod data;
mod glob;
mod inputs;
mod model;
mod registry;
mod rule;
mod rules;
mod settings;

pub mod directives;
pub mod report;

pub use config::Config;
pub use model::{
    ContainerPassword, Document, EnvEntry, EventFilter, InputDecl, Job, ListedValue, Matrix,
    MatrixRow, PermissionEntry, PermissionsBlock, ScheduleEntry, Step, Trigger,
};
pub use registry::Registry;
pub use rule::{Finding, Rule, RuleMeta, Severity};
pub use settings::{RuleOverride, RuleSettings};

/// Errors returned by the public entry points.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The document could not be parsed.
    #[error("could not parse workflow: {0}")]
    Parse(String),
    /// The configuration was malformed or named something unknown.
    #[error("invalid configuration: {0}")]
    Config(String),
}

/// Lint a workflow document, returning findings sorted by line, then rule id.
///
/// Sorting is part of the contract: a linter whose output order depends on a hash map
/// cannot be diffed between runs.
///
/// # Examples
///
/// ```
/// use muhtesip::{lint_str, Registry};
///
/// let findings = lint_str(
///     "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n",
///     &Registry::all(),
/// ).unwrap();
///
/// assert_eq!(findings.len(), 1);
/// assert_eq!(findings[0].rule, "missing-timeout");
/// assert_eq!(findings[0].line, 3, "the job's key line");
/// ```
///
/// A rule can be turned off through the registry's settings:
///
/// ```
/// use muhtesip::{lint_str, Registry, RuleOverride, RuleSettings};
///
/// let settings = RuleSettings::new().with("missing-timeout", RuleOverride {
///     enabled: Some(false),
///     ..Default::default()
/// });
/// let findings = lint_str(
///     "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n",
///     &Registry::all().configure(settings),
/// ).unwrap();
///
/// assert!(findings.is_empty());
/// ```
pub fn lint_str(text: &str, registry: &Registry) -> Result<Vec<Finding>, Error> {
    let doc = Document::parse(text)?;
    let mut findings = registry.find(&doc, text);
    findings.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.rule.cmp(b.rule)));
    Ok(findings)
}

/// Lint with the default rule set.
///
/// # Examples
///
/// ```
/// use muhtesip::{lint, Severity};
///
/// let findings = lint(
///     "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n      - uses: example/toolbox@v4\n",
/// ).unwrap();
///
/// assert_eq!(findings.len(), 1);
/// assert_eq!(findings[0].rule, "unpinned-action");
/// assert_eq!(findings[0].severity, Severity::Warning);
/// ```
///
/// A document the parser cannot read comes back as an [`Error`], never as a panic:
///
/// ```
/// use muhtesip::lint;
///
/// assert!(lint("jobs: [unclosed").is_err());
/// ```
pub fn lint(text: &str) -> Result<Vec<Finding>, Error> {
    lint_str(text, &Registry::default())
}
