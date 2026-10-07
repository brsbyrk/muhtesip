//! The env-var rule: variable names that the platform will not export.

use muhtesip::{Severity, lint};

/// A workflow with a bad name at workflow, job, and step level.
const BAD: &str = include_str!("fixtures/env_var.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "env-var")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

#[test]
fn the_fixture_reports_the_bad_name_at_every_level() {
    let found = hits(BAD);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![7, 14, 19], "{found:?}");
    assert!(found[0].1.contains("'BAD NAME'"), "{:?}", found[0]);
    assert!(found[1].1.contains("'ALSO=BAD'"), "{:?}", found[1]);
    assert!(found[2].1.contains("'amp&name'"), "{:?}", found[2]);
}

#[test]
fn a_finding_points_at_the_variable_name() {
    let text = "jobs:\n  a:\n    steps:\n      - run: echo\n        env:\n          FINE: 1\n          BAD=ONE: 2\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, 7, "the name's own line, not the step's");
}

#[test]
fn an_ordinary_name_is_accepted() {
    let text = "env:\n  GOOD_NAME: 1\n  also-good: 2\njobs:\n  a:\n    steps: []\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_tab_in_a_name_is_rejected() {
    // A tab is legal in a quoted YAML key and illegal in an environment variable name.
    let text = "jobs:\n  a:\n    steps:\n      - run: echo\n        env:\n          \"TAB\\tNAME\": nope\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_name_built_from_an_expression_is_not_judged() {
    let text = "jobs:\n  a:\n    steps:\n      - run: echo\n        env:\n          ${{ format('NAME') }}: nope\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_workflow_without_env_is_not_this_rules_business() {
    let text = "jobs:\n  a:\n    steps:\n      - run: echo hi\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
    // Control: an invalid variable name is reported.
    assert_eq!(
        hits("jobs:\n  a:\n    steps:\n      - run: echo\n        env:\n          BAD=ONE: 2\n")
            .len(),
        1
    );
}

#[test]
fn the_declared_severity_is_error() {
    // The variable is silently not exported, so the step runs without a value it expects.
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "env-var")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Error);
}
