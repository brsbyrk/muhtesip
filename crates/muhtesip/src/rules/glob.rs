//! The glob rule.

use crate::data::{PATH_FILTER_KEYS, REF_FILTER_KEYS};
use crate::glob::{path_pattern_errors, ref_pattern_errors};
use crate::model::Document;
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// A filter pattern must be one the platform can match.
///
/// An invalid `branches`, `tags`, or `paths` pattern is rejected when the workflow is read, so the
/// run never starts — and the rejection names the pattern, not the mistake in it.
pub struct FilterPatterns;

impl FilterPatterns {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "glob",
        severity: Severity::Error,
        description: "a ref or path filter pattern is not valid",
    };
}

impl Rule for FilterPatterns {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for trigger in &doc.triggers {
            for filter in &trigger.filters {
                let check: fn(&str) -> Vec<String> =
                    if REF_FILTER_KEYS.contains(&filter.name.as_str()) {
                        ref_pattern_errors
                    } else if PATH_FILTER_KEYS.contains(&filter.name.as_str()) {
                        path_pattern_errors
                    } else {
                        // Not a pattern filter (`types`, `workflows`, …).
                        continue;
                    };

                for value in &filter.values {
                    // A pattern built from an expression has no value to check.
                    if value.value.contains(EXPRESSION_OPEN) {
                        continue;
                    }
                    for message in check(&value.value) {
                        findings.push(Finding {
                            rule: self.id(),
                            severity: self.meta().severity,
                            message: format!("pattern '{}': {message}", value.value),
                            line: value.line,
                        });
                    }
                }
            }
        }
    }
}
