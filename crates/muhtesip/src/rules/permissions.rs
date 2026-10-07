//! The permissions rule.

use crate::data::{is_known_access, is_known_permissions_value, is_known_scope};
use crate::model::{Document, PermissionsBlock};
use crate::rule::{Finding, Rule, RuleMeta, Severity};

use super::EXPRESSION_OPEN;

/// A workflow token must not be granted something the platform does not offer.
///
/// A misspelt scope or access is silently ignored by the platform, so the job runs with less
/// authority than the author intended — or, worse, the author believes a permission was granted
/// that never was.
pub struct Permissions;

impl Permissions {
    /// The rule's declarative facts.
    pub const META: RuleMeta = RuleMeta {
        id: "permissions",
        severity: Severity::Error,
        description: "a `permissions` entry names an unknown scope or access",
    };
}

impl Rule for Permissions {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        if let Some(block) = &doc.permissions {
            check_permissions(self, block, findings);
        }
        for job in &doc.jobs {
            if let Some(block) = &job.permissions {
                check_permissions(self, block, findings);
            }
        }
    }
}

/// Check one permissions block, whether it belongs to the workflow or to a job.
fn check_permissions(rule: &Permissions, block: &PermissionsBlock, findings: &mut Vec<Finding>) {
    match block {
        PermissionsBlock::Token { value, line } => {
            if value.contains(EXPRESSION_OPEN) || is_known_permissions_value(value) {
                return;
            }
            findings.push(Finding {
                rule: rule.id(),
                severity: rule.meta().severity,
                message: format!(
                    "permissions value '{value}' is unknown; expected 'read-all' or 'write-all'"
                ),
                line: *line,
            });
        }
        PermissionsBlock::Scopes { entries, .. } => {
            for entry in entries {
                if !is_known_scope(&entry.scope) {
                    findings.push(Finding {
                        rule: rule.id(),
                        severity: rule.meta().severity,
                        message: format!("permission '{}' is unknown", entry.scope),
                        line: entry.line,
                    });
                    continue;
                }
                if !entry.access.contains(EXPRESSION_OPEN) && !is_known_access(&entry.access) {
                    findings.push(Finding {
                        rule: rule.id(),
                        severity: rule.meta().severity,
                        message: format!(
                            "access '{}' for permission '{}' is unknown; expected read, write, or none",
                            entry.access, entry.scope
                        ),
                        line: entry.line,
                    });
                }
            }
        }
    }
}
