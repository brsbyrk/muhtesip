//! The env-var rule.

use crate::model::{Document, EnvEntry};
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// The characters an environment variable name must not contain.
const INVALID_ENV_NAME_CHARS: &str = "&= \t";

/// An environment variable's name must not contain a character that breaks it.
///
/// The platform silently will not export a variable whose name contains `&`, `=`, a space, or a
/// tab — so the step runs without the value the author thought it had.
pub struct EnvVarNames;

impl EnvVarNames {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "env-var",
        severity: Severity::Error,
        description: "an `env` variable name contains a character that breaks it",
    };
}

impl Rule for EnvVarNames {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        check_env_names(self, &doc.env, findings);
        for job in &doc.jobs {
            check_env_names(self, &job.env, findings);
            for step in &job.steps {
                check_env_names(self, &step.env, findings);
            }
        }
    }
}

/// Check the names in one `env` mapping.
fn check_env_names(rule: &EnvVarNames, vars: &[EnvEntry], findings: &mut Vec<Finding>) {
    for var in vars {
        // A name built from an expression has no value to judge.
        if var.name.contains(EXPRESSION_OPEN) {
            continue;
        }
        if var.name.chars().any(|c| INVALID_ENV_NAME_CHARS.contains(c)) {
            findings.push(Finding {
                rule: rule.id(),
                severity: rule.meta().severity,
                message: format!(
                    "environment variable name '{}' is invalid; it must not contain '&', '=', spaces, or tabs",
                    var.name
                ),
                line: var.line,
            });
        }
    }
}
