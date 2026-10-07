//! The if-cond rule.

use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::{EXPRESSION_CLOSE, EXPRESSION_OPEN};

/// The literal a condition must not be: always true.
const TRUE_LITERAL: &str = "true";
/// The literal a condition must not be: always false.
const FALSE_LITERAL: &str = "false";

/// A job's or a step's `if:` condition must be able to decide something.
///
/// A condition the platform evaluates to a constant either runs something the author meant to
/// guard or silently skips something the author meant to run.
pub struct Condition;

impl Condition {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "if-cond",
        severity: Severity::Warning,
        description: "an `if` condition is constant, or its expression is not the whole value",
    };
}

impl Rule for Condition {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            if let Some(value) = &job.condition {
                check_condition(
                    self,
                    value,
                    job.condition_line.unwrap_or(job.line),
                    findings,
                );
            }
            for step in &job.steps {
                if let Some(value) = &step.condition {
                    check_condition(
                        self,
                        value,
                        step.condition_line.unwrap_or(step.line),
                        findings,
                    );
                }
            }
        }
    }
}

/// Check one `if` value.
fn check_condition(rule: &Condition, value: &str, line: usize, findings: &mut Vec<Finding>) {
    let trimmed = value.trim();
    let opens = trimmed.matches(EXPRESSION_OPEN).count();
    let closes = trimmed.matches(EXPRESSION_CLOSE).count();

    if opens + closes > 0 {
        let wraps_whole = opens == 1
            && closes == 1
            && trimmed.starts_with(EXPRESSION_OPEN)
            && trimmed.ends_with(EXPRESSION_CLOSE);
        if !wraps_whole {
            // The platform reads the value, not the expression: anything left outside the braces
            // keeps the whole condition non-empty, so it never decides anything.
            findings.push(Finding {
                rule: rule.id(),
                severity: rule.meta().severity,
                message: format!(
                    "if condition '{trimmed}' is always true: text outside {EXPRESSION_OPEN} {EXPRESSION_CLOSE} leaves the value non-empty"
                ),
                line,
            });
            return;
        }
    }

    let inner = if opens == 1 {
        trimmed
            .trim_start_matches(EXPRESSION_OPEN)
            .trim_end_matches(EXPRESSION_CLOSE)
            .trim()
    } else {
        trimmed
    };

    let consequence = match inner {
        TRUE_LITERAL => "the guarded block always runs",
        FALSE_LITERAL => "the guarded block never runs",
        _ => return,
    };
    findings.push(Finding {
        rule: rule.id(),
        severity: rule.meta().severity,
        message: format!("if condition is the constant '{inner}'; {consequence}"),
        line,
    });
}
