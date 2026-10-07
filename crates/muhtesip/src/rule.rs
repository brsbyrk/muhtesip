//! The rule contract, the declarative facts a rule publishes, and the finding type.

use crate::model::Document;

/// How serious a finding is.
///
/// Declared **least serious first**, so the derived ordering *is* the severity ordering:
/// `Note < Warning < Error`, and `severity >= threshold` reads as "at least this serious". The
/// reverse order would be a trap — the variants would sort the opposite way to how they read, and a
/// threshold comparison would quietly pick the wrong end of the scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Something worth knowing.
    Note,
    /// A likely problem.
    Warning,
    /// A defect that should fail a check.
    Error,
}

impl Severity {
    /// The canonical lower-case name, also what [`Display`](std::fmt::Display) prints.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::Severity;
    ///
    /// assert_eq!(Severity::Error.name(), "error");
    /// assert_eq!(Severity::Error.to_string(), "error");
    /// ```
    pub fn name(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        }
    }

    /// Parse the canonical name, or `None` for anything else.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::Severity;
    ///
    /// assert_eq!(Severity::from_name("warning"), Some(Severity::Warning));
    /// assert_eq!(Severity::from_name("loud"), None);
    /// ```
    pub fn from_name(text: &str) -> Option<Self> {
        [Severity::Error, Severity::Warning, Severity::Note]
            .into_iter()
            .find(|severity| severity.name() == text)
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The declarative facts about a rule: what it is called, how serious it is by default, and what
/// it catches.
///
/// Rules are data. Every consumer that must *describe* a rule — the CLI formatter, a config file,
/// later a SARIF/JSON encoder — reads these facts instead of hardcoding per-rule knowledge. One
/// rule, one place its identity lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleMeta {
    /// The stable identifier. This is also the value carried in [`Finding::rule`].
    pub id: &'static str,
    /// The severity a finding gets when the rule is not configured otherwise.
    pub severity: Severity,
    /// One line describing what the rule catches. Lower case, no trailing period.
    pub description: &'static str,
}

/// Where, inside the repository, every rule is documented.
const RULES_DOC_PATH: &str = "docs/rules.md";

impl RuleMeta {
    /// The URL of the page that documents this rule.
    ///
    /// Derived from the crate's own repository URL and this rule's id, so the link cannot drift
    /// from the id the rule reports.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::Registry;
    ///
    /// let url = Registry::all().meta("id").unwrap().docs_url();
    /// assert!(url.ends_with("#id"));
    /// ```
    pub fn docs_url(&self) -> String {
        format!(
            "{repo}/blob/main/{path}#{id}",
            repo = env!("CARGO_PKG_REPOSITORY"),
            path = RULES_DOC_PATH,
            id = self.id,
        )
    }
}

/// One result of running one rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The id of the rule that produced this finding.
    pub rule: &'static str,
    /// How serious the finding is.
    pub severity: Severity,
    /// A human-readable explanation.
    pub message: String,
    /// The 1-based line in the source document.
    pub line: usize,
}

/// A single check over a parsed document.
///
/// A rule must not know about any other rule; the registry composes them.
pub trait Rule {
    /// The rule's declarative facts. The single source of truth for its id.
    fn meta(&self) -> &'static RuleMeta;

    /// Append findings for this rule to `findings`.
    fn check(&self, doc: &Document, findings: &mut Vec<Finding>);

    /// Append findings for a rule that must read what the parsed document drops.
    ///
    /// The YAML model carries structure, and a comment is not structure — so a rule that judges
    /// comments (the `directive` rule is the only one) needs the text itself. It is given the ids this
    /// build has as well, because the only question worth asking about a suppression directive is
    /// whether it names a rule that exists, and the registry is the only thing that knows. Defaulted,
    /// so a rule that needs neither the text nor the ids never mentions it.
    fn check_source(&self, _source: &str, _ids: &[&'static str], _findings: &mut Vec<Finding>) {}

    /// The rule's id, derived from [`Rule::meta`] so the two cannot disagree.
    fn id(&self) -> &'static str {
        self.meta().id
    }
}
