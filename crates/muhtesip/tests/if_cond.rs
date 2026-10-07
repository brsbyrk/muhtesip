//! The if-cond rule: constant conditions, and an expression that is not the whole value.

use muhtesip::{Severity, lint};

/// A workflow with a constant job condition, a constant step condition, and a wrapper misuse.
const BAD: &str = include_str!("fixtures/if_cond.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "if-cond")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

/// A workflow with one step carrying the given condition.
fn step_with(condition: &str) -> String {
    format!("jobs:\n  a:\n    steps:\n      - if: {condition}\n        run: echo hi\n")
}

#[test]
fn the_fixture_reports_the_job_condition_the_step_condition_and_the_wrapper_misuse() {
    let found = hits(BAD);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![9, 12, 15], "{found:?}");
    assert!(
        found[0].1.contains("'true'") && found[0].1.contains("always runs"),
        "{:?}",
        found[0]
    );
    assert!(
        found[1].1.contains("'false'") && found[1].1.contains("never runs"),
        "{:?}",
        found[1]
    );
    assert!(
        found[2].1.contains("always true") && found[2].1.contains("text outside"),
        "{:?}",
        found[2]
    );
}

#[test]
fn a_condition_that_decides_something_is_not_flagged() {
    // The fixture's last step, and a bare expression with no wrapper.
    assert!(
        hits(&step_with("github.event_name == 'push'")).is_empty(),
        "{:?}",
        hits(&step_with("github.event_name == 'push'"))
    );
}

#[test]
fn a_constant_inside_a_whole_wrapper_is_still_constant() {
    let found = hits(&step_with("${{ true }}"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("constant"), "{}", found[0].1);
}

#[test]
fn text_after_the_wrapper_makes_the_condition_always_true() {
    // The footgun: the platform reads the value, not the expression.
    for condition in [
        "${{ x }} && true",
        "true && ${{ x }}",
        "${{ false }} || ${{ true }}",
    ] {
        let found = hits(&step_with(condition));
        assert_eq!(found.len(), 1, "{condition}: {found:?}");
        assert!(found[0].1.contains("always true"), "{condition}");
    }
}

#[test]
fn a_job_condition_is_checked_too() {
    let text = "jobs:\n  a:\n    if: false\n    steps: []\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("never runs"), "{}", found[0].1);
}

#[test]
fn a_finding_points_at_the_condition_value() {
    let text = "jobs:\n  a:\n    steps: []\n  b:\n    if: true\n    steps: []\n";
    let found = hits(text);
    assert_eq!(found[0].0, 5, "{found:?}");
}

#[test]
fn a_workflow_without_conditions_is_not_this_rules_business() {
    let text = "jobs:\n  a:\n    steps:\n      - run: echo hi\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
    // Control: a constant condition *is* reported.
    assert_eq!(hits(&step_with("true")).len(), 1);
}

#[test]
fn the_declared_severity_is_warning() {
    // A constant condition is a defect, but not a security or scheduling failure on its own.
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "if-cond")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Warning);
}
