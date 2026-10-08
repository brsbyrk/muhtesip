//! The permissions rule: scopes and access the platform offers pass, everything else is reported.

use muhtesip::{Severity, lint};

/// A workflow with one unknown scope (workflow level) and one unknown access (job level).
const BAD: &str = include_str!("fixtures/permissions.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn entries(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "permissions")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

#[test]
fn the_fixture_reports_the_unknown_scope_and_the_unknown_access_in_line_order() {
    let hits = entries(BAD);
    let lines: Vec<usize> = hits.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![7, 15], "{hits:?}");
    assert!(hits[0].1.contains("contenst"), "{}", hits[0].1);
    assert!(hits[1].1.contains("reed"), "{}", hits[1].1);
}

#[test]
fn scopes_from_either_source_are_accepted() {
    // `code-quality` and `vulnerability-alerts` are docs-only; `models` and
    // `repository-projects` are actionlint-only. The union must accept all of them.
    let text = "permissions:\n  code-quality: read\n  models: read\n  vulnerability-alerts: none\n  repository-projects: write\n  id-token: write\njobs:\n  build:\n    steps: []\n";
    assert!(entries(text).is_empty(), "{:?}", entries(text));
}

#[test]
fn block_level_values_are_accepted() {
    for value in ["read-all", "write-all", "{}"] {
        let text = format!("permissions: {value}\njobs:\n  build:\n    steps: []\n");
        assert!(entries(&text).is_empty(), "{value}: {:?}", entries(&text));
    }
}

#[test]
fn an_unknown_block_level_value_is_reported() {
    let text = "permissions: read\njobs:\n  build:\n    steps: []\n";
    let hits = entries(text);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].1.contains("unknown"), "{}", hits[0].1);
    // The block-level form points at the value it wrote, not at the mapping below it. Asserting
    // only that "something was reported" let a `+1` line shift pass the whole suite: a mutation
    // audit found this was the one rule-location claim no test pinned.
    assert_eq!(
        hits[0].0, 1,
        "the block-level finding points at its own line"
    );
}

#[test]
fn an_expression_is_not_a_scope_or_an_access() {
    let text = "permissions:\n  contents: ${{ inputs.access }}\njobs:\n  build:\n    steps: []\n";
    assert!(entries(text).is_empty());
}

#[test]
fn both_the_workflow_and_the_job_block_are_checked() {
    let text = "permissions:\n  zzz: read\njobs:\n  build:\n    permissions:\n      contents: reed\n    steps: []\n";
    let hits = entries(text);
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert!(hits[0].1.contains("zzz"), "{}", hits[0].1);
    assert!(hits[1].1.contains("reed"), "{}", hits[1].1);
}

#[test]
fn an_unknown_scope_does_not_also_produce_an_access_finding() {
    let text = "permissions:\n  zzz: whatever\njobs:\n  build:\n    steps: []\n";
    let hits = entries(text);
    assert_eq!(hits.len(), 1, "one finding per entry, not two: {hits:?}");
}

#[test]
fn the_declared_severity_is_error() {
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "permissions")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Error);
}

#[test]
fn a_document_without_permissions_is_not_this_rules_business() {
    let text = "jobs:\n  build:\n    steps: []\n";
    assert!(entries(text).is_empty());
    // Control: a permissions block naming something unknown *is* reported.
    assert_eq!(
        entries("permissions: read\njobs:\n  build:\n    steps: []\n").len(),
        1
    );
}
