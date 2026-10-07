//! The job-needs rule: undefined references, repeats, and dependency cycles.

use muhtesip::{Severity, lint};

/// A workflow whose `build` job repeats one dependency and names one that does not exist.
const BAD: &str = include_str!("fixtures/job_needs.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "job-needs")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

#[test]
fn the_fixture_reports_a_repeat_and_an_undefined_reference() {
    let found = hits(BAD);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found.iter().all(|(line, _)| *line == 9),
        "both point at the needs value's line: {found:?}"
    );
    assert!(
        found[0].1.contains("more than once") && found[0].1.contains("setup"),
        "{}",
        found[0].1
    );
    assert!(
        found[1].1.contains("does not define") && found[1].1.contains("deploy"),
        "{}",
        found[1].1
    );
}

#[test]
fn a_two_job_cycle_is_reported_for_both_members() {
    let text = "jobs:\n  a:\n    needs: b\n    steps: []\n  b:\n    needs: a\n    steps: []\n";
    let found = hits(text);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().all(|(_, m)| m.contains("cycle")), "{found:?}");
    assert!(
        found[0].1.contains("'a'") && found[1].1.contains("'b'"),
        "{found:?}"
    );
}

#[test]
fn a_job_that_merely_depends_on_a_cycle_is_not_accused() {
    // `c` needs `a`, and `a` is in a cycle with `b` — but `c` itself is fine.
    let text = "jobs:\n  a:\n    needs: b\n    steps: []\n  b:\n    needs: a\n    steps: []\n  c:\n    needs: a\n    steps: []\n";
    let found = hits(text);
    assert_eq!(found.len(), 2, "only the two members: {found:?}");
    assert!(
        !found.iter().any(|(_, message)| message.contains("'c'")),
        "{found:?}"
    );
}

#[test]
fn a_three_job_cycle_reports_every_member() {
    let text = "jobs:\n  a:\n    needs: c\n    steps: []\n  b:\n    needs: a\n    steps: []\n  c:\n    needs: b\n    steps: []\n";
    let found = hits(text);
    assert_eq!(found.len(), 3, "{found:?}");
}

#[test]
fn a_self_dependency_is_a_cycle() {
    let text = "jobs:\n  a:\n    needs: a\n    steps: []\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("cycle"), "{}", found[0].1);
}

#[test]
fn two_separate_cycles_are_both_reported() {
    let text = "jobs:\n  a:\n    needs: b\n    steps: []\n  b:\n    needs: a\n    steps: []\n  c:\n    needs: d\n    steps: []\n  d:\n    needs: c\n    steps: []\n";
    let found = hits(text);
    assert_eq!(
        found.len(),
        4,
        "actionlint reports only the first cycle: {found:?}"
    );
}

#[test]
fn a_diamond_is_not_a_cycle() {
    let text = "jobs:\n  a:\n    steps: []\n  b:\n    needs: a\n    steps: []\n  c:\n    needs: a\n    steps: []\n  d:\n    needs: [b, c]\n    steps: []\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn job_ids_are_compared_case_insensitively() {
    let text = "jobs:\n  Build:\n    steps: []\n  deploy:\n    needs: BUILD\n    steps: []\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn the_declared_severity_is_error() {
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "job-needs")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Error);
}
