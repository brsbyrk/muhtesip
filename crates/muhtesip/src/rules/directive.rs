//! A suppression comment that does not do what it says.
//!
//! This is the one rule that reads the source text instead of the parsed document, because a comment
//! is not part of the YAML structure — the parser drops it. It is registered like any other rule, so
//! it is configurable, appears in `--init-config`, and gets a SARIF descriptor like the rest.

use crate::directives::Directives;
use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

/// A directive that names no rule this build has — or that is not a usable directive at all —
/// suppresses nothing while reading as though it does.
pub struct Directive;

/// The rule's declarative facts, kept out of the `Rule` impl so `meta()` can return a `'static`
/// reference and the registry can read a rule's id without running it.
const META: RuleMeta = RuleMeta {
    id: "directive",
    severity: Severity::Warning,
    description: "a suppression comment names an unknown rule, or is not a usable directive",
};

impl Rule for Directive {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    /// Nothing to check in the parsed document: the model carries no comments. The work is in
    /// [`Rule::check_source`].
    fn check(&self, _doc: &Document, _findings: &mut Vec<Finding>) {}

    fn check_source(&self, source: &str, ids: &[&'static str], findings: &mut Vec<Finding>) {
        let directives = Directives::parse(source);

        for line in directives.malformed() {
            findings.push(finding(
                line,
                "suppression comment is not a usable directive; write `# muhtesip: ignore[rule]`"
                    .to_owned(),
            ));
        }

        // The same name twice in one directive is one mistake, not two.
        let mut reported: Vec<(usize, &str)> = Vec::new();
        for (line, name) in directives.named() {
            if ids.contains(&name) || reported.contains(&(line, name)) {
                continue;
            }
            reported.push((line, name));
            findings.push(finding(
                line,
                format!("suppression comment names an unknown rule '{name}'"),
            ));
        }
    }
}

/// A finding about a suppression comment, at the line the comment sits on.
fn finding(line: usize, message: String) -> Finding {
    Finding {
        rule: META.id,
        severity: META.severity,
        message,
        line,
    }
}
