//! The action-ref rule.

use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

/// The prefix marking a Docker image reference.
const DOCKER_PREFIX: &str = "docker://";
/// The prefix marking an action in the same repository, at the running commit.
const RUNNING_COMMIT_PREFIX: &str = "$/";
/// The prefix marking an action in the runner's checked-out workspace.
const WORKSPACE_PREFIX: &str = "./";
/// The fewest non-empty path segments a remote reference can have: `owner/repo`.
const REMOTE_SEGMENTS: usize = 2;

/// A `uses:` reference must be written in a form the platform can resolve.
///
/// The documented forms are `{owner}/{repo}@{ref}`, `{owner}/{repo}/{path}@{ref}`, `$/path`,
/// `./path` and `docker://{image}:{tag}`. A reference in no such form resolves to nothing: the run
/// fails with a message about the reference rather than about the mistake that produced it.
pub struct ActionRef;

impl ActionRef {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "action-ref",
        severity: Severity::Error,
        description: "a `uses:` reference is not in a form the platform can resolve",
    };

    /// Report one reference if the platform cannot resolve it.
    fn report(&self, uses: &str, line: usize, findings: &mut Vec<Finding>) {
        let Some(problem) = unresolvable(uses) else {
            return;
        };
        findings.push(Finding {
            rule: self.id(),
            severity: self.meta().severity,
            message: format!("action reference '{uses}' {problem}"),
            line,
        });
    }
}

impl Rule for ActionRef {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            // A job that calls a reusable workflow references one too, and its `uses` is the only one
            // that does not live on a step.
            if let Some(uses) = &job.uses {
                self.report(uses, job.uses_line.unwrap_or(job.line), findings);
            }
            for step in &job.steps {
                let Some(uses) = &step.uses else {
                    continue;
                };
                self.report(uses, step.uses_line.unwrap_or(step.line), findings);
            }
        }
    }
}

/// Why the platform cannot resolve this reference, or `None` when it can.
///
/// Deliberately permissive where the documented forms are silent. Measured over 1,527 `uses:` values
/// in the 258-file corpus: the only shape a stricter version of this rejected was `./` — the action at
/// the repository root, which GitHub's own action repositories use 50 times. A rule that flags those
/// is worse than a rule that misses a malformed reference.
fn unresolvable(uses: &str) -> Option<&'static str> {
    let reference = uses.trim();
    // `docker://{image}:{tag}`, or `docker://{host}/{image}:{tag}`: the image is the rest of the value.
    if let Some(image) = reference.strip_prefix(DOCKER_PREFIX) {
        return image.is_empty().then_some("names no image");
    }
    // `$/path` resolves to the commit the run is already using, so a ref here is meaningless — and the
    // documented rule says a `$/` reference "must not include an `@{ref}` suffix". `$/` alone is the
    // action at the repository root, which is valid for the same reason `./` is.
    if let Some(path) = reference.strip_prefix(RUNNING_COMMIT_PREFIX) {
        return path
            .contains('@')
            .then_some("carries a `@ref`, which the `$/` form does not take");
    }
    // `./` and `./path` resolve in the checked-out workspace, and the path may be empty: `./` is the
    // repository root. Nothing here is decidable from the document alone, so nothing is reported.
    if reference.starts_with(WORKSPACE_PREFIX) {
        return None;
    }
    let Some((path, named_ref)) = reference.rsplit_once('@') else {
        return Some("names no `@ref`");
    };
    if named_ref.is_empty() {
        return Some("names no `@ref`");
    }
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .count();
    if segments < REMOTE_SEGMENTS {
        return Some("is not `{owner}/{repo}[/path]@ref`");
    }
    None
}
