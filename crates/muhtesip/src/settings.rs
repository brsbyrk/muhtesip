//! A project's adjustments to the shipped rules, expressed as data.
//!
//! A config file (roadmap A3) will parse straight into [`RuleSettings`]; the registry applies it
//! as a lookup, so no rule's id is ever named in code.

use std::collections::BTreeMap;

use crate::rule::Severity;

/// What a project wants changed about one rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuleOverride {
    /// A severity to use in place of the rule's declared default.
    pub severity: Option<Severity>,
    /// `Some(false)` turns the rule off; `None` or `Some(true)` leaves it on.
    pub enabled: Option<bool>,
}

impl RuleOverride {
    /// Whether the rule runs, honouring the default of "on".
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    /// The severity to emit, given the rule's own declared default.
    pub fn severity_or(&self, declared: Severity) -> Severity {
        self.severity.unwrap_or(declared)
    }
}

/// The per-rule adjustments for a project, keyed by rule id, in a deterministic order.
#[derive(Debug, Clone, Default)]
pub struct RuleSettings {
    /// The adjustments, keyed by rule id; a `BTreeMap` so the order is deterministic.
    overrides: BTreeMap<String, RuleOverride>,
}

impl RuleSettings {
    /// No adjustments: every rule runs with its declared severity.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::RuleSettings;
    ///
    /// assert!(RuleSettings::new().is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an adjustment, builder-style.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{RuleOverride, RuleSettings, Severity};
    ///
    /// let settings = RuleSettings::new().with("id", RuleOverride {
    ///     severity: Some(Severity::Note),
    ///     ..Default::default()
    /// });
    ///
    /// assert_eq!(
    ///     settings.get("id").map(|over| over.severity_or(Severity::Error)),
    ///     Some(Severity::Note),
    /// );
    /// ```
    #[must_use]
    pub fn with(mut self, id: impl Into<String>, adjustment: RuleOverride) -> Self {
        self.overrides.insert(id.into(), adjustment);
        self
    }

    /// The adjustment for a rule, if the project set one.
    pub fn get(&self, id: &str) -> Option<&RuleOverride> {
        self.overrides.get(id)
    }

    /// Whether no adjustments were set.
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }

    /// The ids an adjustment was set for, in ascending order.
    pub fn ids(&self) -> Vec<&str> {
        self.overrides.keys().map(String::as_str).collect()
    }
}
