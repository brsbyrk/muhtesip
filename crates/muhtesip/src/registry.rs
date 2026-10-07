//! The rule registry: which rules run, declared as data.

use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta};
use crate::rules::{
    ActionRef, Condition, Credentials, DeprecatedCommands, Directive, EnvVarNames, FilterPatterns,
    Identifiers, JobNeeds, MatrixCombinations, MissingTimeout, Permissions, RunnerLabel, ShellName,
    TriggerEvents, UnpinnedAction,
};
use crate::settings::RuleSettings;

/// An ordered set of rules, plus a project's per-rule adjustments.
pub struct Registry {
    /// The rules, in registration order — the order findings are emitted in.
    rules: Vec<Box<dyn Rule>>,
    /// The project's per-rule adjustments, looked up by rule id.
    settings: RuleSettings,
}

impl Registry {
    /// An empty registry — no rules run.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{lint_str, Registry};
    ///
    /// let clean = "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n";
    ///
    /// // The shipped set reports the missing timeout...
    /// assert_eq!(lint_str(clean, &Registry::all()).unwrap().len(), 1);
    /// // ...and an empty registry reports nothing at all.
    /// assert!(lint_str(clean, &Registry::new()).unwrap().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            settings: RuleSettings::new(),
        }
    }

    /// Add a rule, builder-style.
    ///
    /// # Examples
    ///
    /// The embedding story: a consumer adds its own check without touching the crate.
    ///
    /// ```
    /// use muhtesip::{lint_str, Document, Finding, Registry, Rule, RuleMeta, Severity};
    ///
    /// /// Reports a document that declares no jobs at all.
    /// struct NoJobs;
    ///
    /// static NO_JOBS: RuleMeta = RuleMeta {
    ///     id: "no-jobs",
    ///     severity: Severity::Warning,
    ///     description: "the document declares no jobs",
    /// };
    ///
    /// impl Rule for NoJobs {
    ///     fn meta(&self) -> &'static RuleMeta {
    ///         &NO_JOBS
    ///     }
    ///
    ///     fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
    ///         if doc.jobs.is_empty() {
    ///             findings.push(Finding {
    ///                 rule: self.id(),
    ///                 severity: self.meta().severity,
    ///                 message: "the document declares no jobs".to_owned(),
    ///                 line: 1,
    ///             });
    ///         }
    ///     }
    /// }
    ///
    /// let registry = Registry::new().with(NoJobs);
    /// let findings = lint_str("on: [push]\n", &registry).unwrap();
    ///
    /// assert_eq!(findings.len(), 1);
    /// assert_eq!(findings[0].rule, "no-jobs");
    /// ```
    #[must_use]
    pub fn with(mut self, rule: impl Rule + 'static) -> Self {
        self.rules.push(Box::new(rule));
        self
    }

    /// Apply a project's per-rule adjustments, builder-style.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{lint_str, Registry, RuleOverride, RuleSettings, Severity};
    ///
    /// let settings = RuleSettings::new().with("missing-timeout", RuleOverride {
    ///     severity: Some(Severity::Error),
    ///     ..Default::default()
    /// });
    /// let findings = lint_str(
    ///     "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n",
    ///     &Registry::all().configure(settings),
    /// ).unwrap();
    ///
    /// // The rule's declared default is `Note`; the project raised it.
    /// assert_eq!(findings[0].severity, Severity::Error);
    /// ```
    #[must_use]
    pub fn configure(mut self, settings: RuleSettings) -> Self {
        self.settings = settings;
        self
    }

    /// The adjustments currently applied.
    pub fn settings(&self) -> &RuleSettings {
        &self.settings
    }

    /// Run every enabled rule and collect the findings, in registration order.
    ///
    /// An adjustment is a lookup on the rule's id: a disabled rule is skipped, and an adjusted
    /// severity is written over whatever the rule emitted. No rule is named here.
    ///
    /// `source` is handed to the rules with it: the parsed document is structure, and a rule that must
    /// judge a comment needs the text that the structure dropped. The ids this registry has go with it,
    /// because only the registry knows which rule names exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{Document, Registry};
    ///
    /// let text = "on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n";
    /// let doc = Document::parse(text).unwrap();
    ///
    /// let findings = Registry::all().find(&doc, text);
    /// assert_eq!(findings.len(), 1);
    /// assert_eq!(findings[0].rule, "missing-timeout");
    /// ```
    pub fn find(&self, doc: &Document, source: &str) -> Vec<Finding> {
        let ids = self.ids();
        let mut findings = Vec::new();
        for rule in &self.rules {
            let adjustment = self.settings.get(rule.id());
            if adjustment.is_some_and(|over| !over.is_enabled()) {
                continue;
            }

            let before = findings.len();
            rule.check(doc, &mut findings);
            rule.check_source(source, &ids, &mut findings);

            let declared = rule.meta().severity;
            let severity = adjustment.map_or(declared, |over| over.severity_or(declared));
            for finding in &mut findings[before..] {
                finding.severity = severity;
            }
        }
        findings
    }

    /// The ids of the registered rules, in registration order.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::Registry;
    ///
    /// let ids = Registry::all().ids();
    ///
    /// assert_eq!(ids.first(), Some(&"unpinned-action"), "registration order, not alphabetical");
    /// assert!(ids.contains(&"directive"));
    /// ```
    pub fn ids(&self) -> Vec<&'static str> {
        self.rules.iter().map(|rule| rule.id()).collect()
    }

    /// The declarative facts of the registered rules, in registration order.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{Registry, Severity};
    ///
    /// let metas = Registry::all().metas();
    ///
    /// assert!(metas
    ///     .iter()
    ///     .any(|meta| meta.id == "unpinned-action" && meta.severity == Severity::Warning));
    /// ```
    pub fn metas(&self) -> Vec<&'static RuleMeta> {
        self.rules.iter().map(|rule| rule.meta()).collect()
    }

    /// Look up one rule's facts by id — a lookup, not a branch.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{Registry, Severity};
    ///
    /// let registry = Registry::all();
    ///
    /// assert_eq!(registry.meta("id").map(|meta| meta.severity), Some(Severity::Error));
    /// assert!(registry.meta("no-such-rule").is_none());
    /// ```
    pub fn meta(&self, id: &str) -> Option<&'static RuleMeta> {
        self.rules
            .iter()
            .map(|rule| rule.meta())
            .find(|meta| meta.id == id)
    }

    /// Check that every rule id a project configured names a rule this registry has.
    ///
    /// The in-memory API is permissive (an unknown id is inert); a configuration **file** is user
    /// input, so a typo is reported instead of silently doing nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{Config, Registry};
    ///
    /// let config = Config::parse("rules:\n  no-such-rule:\n    enabled: false\n").unwrap();
    ///
    /// let error = Registry::all().validate(config.settings()).unwrap_err();
    /// assert!(error.to_string().contains("no-such-rule"));
    /// ```
    pub fn validate(&self, settings: &RuleSettings) -> Result<(), crate::Error> {
        let unknown: Vec<&str> = settings
            .ids()
            .into_iter()
            .filter(|id| self.meta(id).is_none())
            .collect();
        if unknown.is_empty() {
            return Ok(());
        }
        Err(crate::Error::Config(format!(
            "unknown rule id(s): {}",
            unknown.join(", ")
        )))
    }

    /// The shipped rule set: every rule this crate has, in a fixed order.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::Registry;
    ///
    /// // `all()` is also the default, so both report the same set in the same order.
    /// assert_eq!(Registry::all().ids(), Registry::default().ids());
    /// ```
    pub fn all() -> Self {
        Self::new()
            .with(UnpinnedAction)
            // Beside the pinning rule on purpose: both judge a `uses:` reference, one for what it
            // points at and one for whether the platform can resolve it at all.
            .with(ActionRef)
            .with(MissingTimeout)
            .with(RunnerLabel)
            .with(Permissions)
            .with(JobNeeds)
            .with(Identifiers)
            .with(DeprecatedCommands)
            .with(Condition)
            .with(ShellName)
            .with(EnvVarNames)
            .with(Credentials)
            .with(MatrixCombinations)
            .with(FilterPatterns)
            .with(TriggerEvents)
            // Last, because it is the one rule that judges the file's own comments rather than its
            // workflow semantics.
            .with(Directive)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::all()
    }
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("rules", &self.ids())
            .field("settings", &self.settings.ids())
            .finish()
    }
}
