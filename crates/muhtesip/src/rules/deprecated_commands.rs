//! The deprecated-commands rule.

use crate::data::{deprecated_commands_in, replacement_for};
use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

/// A `run:` script must not use a workflow command the platform retired.
///
/// The retired commands either do nothing or are refused, so a workflow that still uses them
/// silently loses the value it thought it was setting.
pub struct DeprecatedCommands;

impl DeprecatedCommands {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "deprecated-commands",
        severity: Severity::Warning,
        description: "a `run` script uses a workflow command the platform retired",
    };
}

impl Rule for DeprecatedCommands {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            for step in &job.steps {
                let Some(script) = &step.run else {
                    continue;
                };
                let first_line = step.run_line.unwrap_or(step.line);
                for (command, offset) in deprecated_commands_in(script) {
                    // Count the line breaks before the command, so a finding inside a multi-line
                    // script points at the command rather than at the script's first line.
                    let line = first_line + script[..offset].matches('\n').count();
                    let replacement = replacement_for(command).unwrap_or_default();
                    findings.push(Finding {
                        rule: self.id(),
                        severity: self.meta().severity,
                        message: format!(
                            "workflow command '::{command}' is retired; use `{replacement}` instead"
                        ),
                        line,
                    });
                }
            }
        }
    }
}
