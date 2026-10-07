//! The runner-label rule.

use crate::data::{SELF_HOSTED_LABEL, is_known, os_family};
use crate::model::{Document, Job};
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// A job must name a runner the platform can actually provide.
///
/// A misspelt or invented label never schedules, so the workflow fails for a reason that is
/// invisible until a run is attempted.
pub struct RunnerLabel;

impl RunnerLabel {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "runner-label",
        severity: Severity::Error,
        description: "a `runs-on` label is not one GitHub hosts",
    };
}

impl Rule for RunnerLabel {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            if job.runs_on.is_empty() {
                continue;
            }
            // A value this rule cannot decide is not a value it may guess about: an expression
            // has no value here, and a self-hosted runner's own labels are unknowable without
            // configuration. Skipping is the honest choice; flagging would be a false positive.
            if job.runs_on.iter().any(|label| is_undecidable(label)) {
                continue;
            }
            let line = job.runs_on_line.unwrap_or(job.line);
            for label in job.runs_on.iter().filter(|label| !is_known(label)) {
                findings.push(Finding {
                    rule: self.id(),
                    severity: self.meta().severity,
                    message: format!("runner label '{label}' is not one GitHub hosts"),
                    line,
                });
            }
            report_conflicts(self, job, line, findings);
        }
    }
}

/// Report labels in one `runs-on` list that name different operating systems.
///
/// A list is a conjunction — the runner must carry every label — and two different operating
/// systems cannot both be true, so such a job never schedules.
///
/// Deliberately narrow: two *versions* of one system are not treated as a conflict. Version-level
/// claims are the kind that false-positive on real files, and a false report costs more than a
/// missing one.
fn report_conflicts(rule: &RunnerLabel, job: &Job, line: usize, findings: &mut Vec<Finding>) {
    let mut families = job
        .runs_on
        .iter()
        .filter_map(|label| os_family(label).map(|family| (family, label)));
    let Some((first_family, first_label)) = families.next() else {
        return;
    };
    for (family, label) in families {
        if family != first_family {
            findings.push(Finding {
                rule: rule.id(),
                severity: rule.meta().severity,
                message: format!(
                    "runner labels '{first_label}' ({}) and '{label}' ({}) name different operating systems; one job cannot run on both",
                    first_family.name(),
                    family.name()
                ),
                line,
            });
        }
    }
}

/// Whether a label makes the whole `runs-on` value undecidable by this rule.
fn is_undecidable(label: &str) -> bool {
    label.contains(EXPRESSION_OPEN) || label.eq_ignore_ascii_case(SELF_HOSTED_LABEL)
}
