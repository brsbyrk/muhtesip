//! The matrix rule.

use crate::model::{Document, MatrixRow};
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// A job's matrix must describe distinct work, and its `exclude` must have something to remove.
///
/// A repeated value silently runs the same job twice; an `exclude` with nothing to remove is dead
/// configuration that reads as if it filters something.
pub struct MatrixCombinations;

impl MatrixCombinations {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "matrix",
        severity: Severity::Warning,
        description: "a matrix repeats a value, or its `exclude` has nothing to remove",
    };
}

impl Rule for MatrixCombinations {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for job in &doc.jobs {
            let Some(matrix) = &job.matrix else {
                continue;
            };
            for row in &matrix.rows {
                report_repeats(self, row, findings);
            }
            // An `exclude` with nothing to remove: no rows, and nothing `include` could add.
            if matrix.exclude_count > 0 && matrix.rows.is_empty() && matrix.include_count == 0 {
                if let Some(line) = matrix.exclude_line {
                    findings.push(Finding {
                        rule: self.id(),
                        severity: self.meta().severity,
                        message: "'exclude' is declared but the matrix has no values to remove"
                            .to_owned(),
                        line,
                    });
                }
            }
        }
    }
}

/// Report a value a row lists more than once.
fn report_repeats(rule: &MatrixCombinations, row: &MatrixRow, findings: &mut Vec<Finding>) {
    let mut seen: Vec<&str> = Vec::new();
    for value in &row.values {
        // Two identical expressions are not provably the same value.
        if value.value.contains(EXPRESSION_OPEN) {
            continue;
        }
        if seen.contains(&value.value.as_str()) {
            findings.push(Finding {
                rule: rule.id(),
                severity: rule.meta().severity,
                message: format!(
                    "matrix row '{}' lists '{}' more than once",
                    row.name, value.value
                ),
                line: value.line,
            });
            continue;
        }
        seen.push(&value.value);
    }
}
