//! The credentials rule.

use crate::model::{ContainerPassword, Document};
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// A container's password must come from a secret, never from the workflow file.
///
/// A literal password is committed to the repository in plain text, and anyone who can read the
/// workflow can read it.
pub struct Credentials;

impl Credentials {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "credentials",
        severity: Severity::Error,
        description: "a container password is written literally instead of from a secret",
    };
}

impl Rule for Credentials {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            for password in &job.container_passwords {
                // A value built from an expression comes from somewhere else; only a literal is
                // written into the file.
                if password.value.contains(EXPRESSION_OPEN) {
                    continue;
                }
                findings.push(Finding {
                    rule: self.id(),
                    severity: self.meta().severity,
                    message: message_for(password),
                    line: password.line,
                });
            }
        }
    }
}

/// The message for one literal password, naming where it sits.
fn message_for(password: &ContainerPassword) -> String {
    match &password.service {
        Some(service) => format!(
            "the '{service}' service's password is written literally; pass it from a secret instead"
        ),
        None => "the container's password is written literally; pass it from a secret instead"
            .to_owned(),
    }
}
