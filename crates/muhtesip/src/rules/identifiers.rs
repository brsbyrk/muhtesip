//! The id rule.

use std::collections::HashMap;

use crate::data::is_valid_id;
use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// Job and step ids must be unique and must match the platform's grammar.
///
/// An id is how a workflow refers to a job or a step, so a duplicate makes the reference ambiguous
/// and a malformed id makes the reference unrunnable.
pub struct Identifiers;

impl Identifiers {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "id",
        severity: Severity::Error,
        description: "a job or step id is duplicated or does not match the id grammar",
    };
}

impl Rule for Identifiers {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        let mut job_ids: HashMap<String, usize> = HashMap::new();
        for job in &doc.jobs {
            check_grammar(self, &job.id, "job", job.line, findings);
            let key = job.id.to_lowercase();
            if let Some(first) = job_ids.get(&key) {
                findings.push(Finding {
                    rule: self.id(),
                    severity: self.meta().severity,
                    message: format!("job id '{}' is already used at line {first}", job.id),
                    line: job.line,
                });
            } else {
                job_ids.insert(key, job.line);
            }

            // A `needs` entry is a reference to a job id, so it answers to the same grammar.
            let needs_line = job.needs_line.unwrap_or(job.line);
            for need in &job.needs {
                check_grammar(self, need, "job", needs_line, findings);
            }

            // Step ids are unique within their job, not across the workflow.
            let mut step_ids: HashMap<String, usize> = HashMap::new();
            for step in &job.steps {
                let Some(id) = &step.id else {
                    continue;
                };
                let line = step.id_line.unwrap_or(step.line);
                check_grammar(self, id, "step", line, findings);
                let key = id.to_lowercase();
                if let Some(first) = step_ids.get(&key) {
                    findings.push(Finding {
                        rule: self.id(),
                        severity: self.meta().severity,
                        message: format!(
                            "step id '{id}' is already used in job '{}' at line {first}",
                            job.id
                        ),
                        line,
                    });
                } else {
                    step_ids.insert(key, line);
                }
            }
        }
    }
}

/// Report an id that does not match the platform's grammar.
fn check_grammar(
    rule: &Identifiers,
    id: &str,
    what: &str,
    line: usize,
    findings: &mut Vec<Finding>,
) {
    if id.is_empty() || id.contains(EXPRESSION_OPEN) || is_valid_id(id) {
        return;
    }
    findings.push(Finding {
        rule: rule.id(),
        severity: rule.meta().severity,
        message: format!(
            "{what} id '{id}' must start with a letter or underscore and contain only letters, digits, '-', or '_'"
        ),
        line,
    });
}
