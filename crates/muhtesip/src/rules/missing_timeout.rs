//! The missing-timeout rule.

use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

/// A job should bound its own runtime.
///
/// Without a bound, a hung step runs until the platform's default limit, which is long
/// enough to strand a queue.
pub struct MissingTimeout;

impl MissingTimeout {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "missing-timeout",
        severity: Severity::Note,
        description: "a job has no `timeout-minutes` bound",
    };
}

impl Rule for MissingTimeout {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            if job.timeout_minutes.is_some() {
                continue;
            }
            findings.push(Finding {
                rule: self.id(),
                severity: self.meta().severity,
                message: format!("job '{}' has no timeout-minutes bound", job.id),
                line: job.line,
            });
        }
    }
}
