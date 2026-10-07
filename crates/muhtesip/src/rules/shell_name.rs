//! The shell-name rule.

use crate::data::{ShellPlatform, is_known_shell, shell_platform, shells_for};
use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// The placeholder a custom shell uses for the script's path.
const CUSTOM_SHELL_PLACEHOLDER: &str = "{0}";

/// A step's `shell:` must name a shell the job's runner provides.
///
/// A shell the runner does not have makes the step fail before its script runs — and the failure
/// arrives at run time rather than at review.
pub struct ShellName;

impl ShellName {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "shell-name",
        severity: Severity::Error,
        description: "a `shell` names something the job's runner does not provide",
    };
}

impl Rule for ShellName {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            let platform = shell_platform(&job.runs_on);
            for step in &job.steps {
                let Some(shell) = &step.shell else {
                    continue;
                };
                // `shell` only means anything on a step that runs a script.
                if step.run.is_none() {
                    continue;
                }
                check_shell(
                    self,
                    shell,
                    platform,
                    step.shell_line.unwrap_or(step.line),
                    findings,
                );
            }
        }
    }
}

/// Check one `shell` value against the runner's platform.
fn check_shell(
    rule: &ShellName,
    shell: &str,
    platform: ShellPlatform,
    line: usize,
    findings: &mut Vec<Finding>,
) {
    let name = shell.trim().to_lowercase();
    // A custom shell names the program and where the script goes; `{0}` is the placeholder. An
    // expression has no value to check. Neither is decidable, so neither is reported.
    if name.contains(CUSTOM_SHELL_PLACEHOLDER) || name.contains(EXPRESSION_OPEN) {
        return;
    }

    let available = shells_for(platform);
    if available.iter().any(|known| *known == name) {
        return;
    }

    // Say *why* it is unavailable when the shell exists, just not here: "sh" on Windows is a
    // different mistake from a misspelt name.
    let qualifier = if is_known_shell(&name) {
        match platform {
            ShellPlatform::Windows => " on Windows",
            ShellPlatform::Unix => " on macOS or Linux",
            ShellPlatform::Any => "",
        }
    } else {
        ""
    };

    findings.push(Finding {
        rule: rule.id(),
        severity: rule.meta().severity,
        message: format!(
            "shell name '{shell}' is invalid{qualifier}; available names are {}",
            available.join(", ")
        ),
        line,
    });
}
