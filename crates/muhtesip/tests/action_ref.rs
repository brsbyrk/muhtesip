//! The `action-ref` rule: a `uses:` reference must be in a form the platform can resolve.
//!
//! The line numbers are read off the fixture, never guessed, and the accepted group is asserted as
//! hard as the reported one — a rule about reference *forms* is only worth having if the forms it
//! leaves alone are the documented ones.

use muhtesip::lint;

const FIXTURE: &str = include_str!("fixtures/action_ref.yaml");

/// The findings of one rule, as (line, message) pairs.
fn findings(rule: &str) -> Vec<(usize, String)> {
    lint(FIXTURE)
        .expect("the fixture parses")
        .into_iter()
        .filter(|finding| finding.rule == rule)
        .map(|finding| (finding.line, finding.message))
        .collect()
}

#[test]
fn a_reference_the_platform_cannot_resolve_is_reported_at_its_own_line() {
    let reported = findings("action-ref");
    let lines: Vec<usize> = reported.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![21, 22, 23, 25, 26], "{reported:#?}");
}

#[test]
fn each_message_names_what_is_wrong_with_that_reference() {
    let reported = findings("action-ref");
    assert!(
        reported[0].1.contains("names no `@ref`"),
        "a remote reference with no ref: {}",
        reported[0].1
    );
    assert!(
        reported[1].1.contains("is not `{owner}/{repo}[/path]@ref`"),
        "one segment is not a repository: {}",
        reported[1].1
    );
    assert!(
        reported[3].1.contains("`$/` form does not take"),
        "a ref on a running-commit reference: {}",
        reported[3].1
    );
    assert!(
        reported[4].1.contains("names no image"),
        "an empty docker reference: {}",
        reported[4].1
    );
}

#[test]
fn every_documented_form_is_left_alone() {
    // Lines 10-17: a commit-pinned remote action, a subdirectory at a branch, `$/path`, `$/` alone,
    // `./`, `./path`, and both Docker forms. The bare `./` and `$/` cases are the action at the
    // repository root — measured, not assumed: a stricter version of this rule rejected 50 of those
    // across the 258-file corpus, every one of them in GitHub's own action repositories.
    let lines: Vec<usize> = findings("action-ref")
        .iter()
        .map(|(line, _)| *line)
        .collect();
    for accepted in 10..=17 {
        assert!(
            !lines.contains(&accepted),
            "line {accepted} is a documented form and must not be reported"
        );
    }
}

#[test]
fn a_workflow_of_documented_forms_only_is_silent() {
    let clean = "on: push\njobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262\n";
    let reported: Vec<String> = lint(clean)
        .expect("the document parses")
        .into_iter()
        .filter(|finding| finding.rule == "action-ref")
        .map(|finding| finding.message)
        .collect();
    assert!(reported.is_empty(), "{reported:#?}");
}
