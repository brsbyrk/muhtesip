//! The unpinned-action rule.

use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

/// The number of hex digits in an immutable commit reference.
const COMMIT_HEX_LEN: usize = 40;

/// A prefix marking a reference to an action in the same repository.
const LOCAL_PREFIX: &str = "./";

/// A prefix marking an action in the same repository, resolved at the running commit.
///
/// Not the same as `./`: a `./` reference resolves in the runner's checked-out workspace, while `$/`
/// always resolves against the repository the file is in.
const RUNNING_COMMIT_PREFIX: &str = "$/";

/// A `uses:` reference must be pinned to an immutable commit.
///
/// A tag or branch can be moved after review, so a workflow that trusts one is trusting
/// whatever the target points at on the day it runs.
pub struct UnpinnedAction;

impl UnpinnedAction {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "unpinned-action",
        severity: Severity::Warning,
        description: "a `uses:` reference is not pinned to an immutable commit",
    };
}

impl Rule for UnpinnedAction {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            // A job that calls a reusable workflow names a reference too, and it is the one that does
            // not sit on a step. It is checked here for the same reason: a reusable workflow called at
            // a tag is trusting whatever that tag points at on the day it runs.
            if let Some(uses) = &job.uses {
                self.report(uses, job.uses_line.unwrap_or(job.line), findings);
            }
            for step in &job.steps {
                let Some(uses) = &step.uses else { continue };
                self.report(uses, step.uses_line.unwrap_or(step.line), findings);
            }
        }
    }
}

impl UnpinnedAction {
    /// Report one reference if it is not pinned.
    fn report(&self, uses: &str, line: usize, findings: &mut Vec<Finding>) {
        if is_pinned(uses) {
            return;
        }
        findings.push(Finding {
            rule: self.id(),
            severity: self.meta().severity,
            message: format!("action '{uses}' is not pinned to an immutable commit"),
            line,
        });
    }
}

/// Whether a `uses:` reference is immutable.
///
/// Two forms are immutable without naming a commit, each for its own documented reason: a local
/// reference (`./path`) comes from the same checkout, so no third party can move it, and a `$/`
/// reference resolves to the repository of the file at the running commit — the docs say it "resolves
/// to that repository at the running commit (the same SHA as the running workflow or action)".
///
/// A `docker://` reference is not a commit at all. It is judged by the same rule as a moved tag: a tag
/// can be repointed after review, so it is not immutable, and the platform's workflow-syntax page
/// never mentions digests, so a digest form is not accepted on a guess.
fn is_pinned(uses: &str) -> bool {
    let reference = uses.trim();
    if reference.starts_with(LOCAL_PREFIX) || reference.starts_with(RUNNING_COMMIT_PREFIX) {
        return true;
    }
    let Some((_, commit)) = reference.rsplit_once('@') else {
        return false;
    };
    commit.len() == COMMIT_HEX_LEN && commit.chars().all(|c| c.is_ascii_hexdigit())
}
