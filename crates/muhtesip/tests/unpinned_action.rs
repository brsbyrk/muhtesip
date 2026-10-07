//! The unpinned-action rule: a `uses:` reference that is not immutable.
//!
//! The second of the two rules that had no fixture or test file of their own (see
//! `tests/missing_timeout.rs` for the note). The fixture carries every form the rule must leave
//! alone — a commit SHA, `./`, `$/path`, `./path` — beside the three it must report, so the
//! negative case is proved against the same document as the positive one.

use muhtesip::{Severity, lint};

const FIXTURE: &str = include_str!("fixtures/unpinned_action.yaml");

/// This rule's findings for a document, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "unpinned-action")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

#[test]
fn the_fixture_reports_every_unpinned_reference_at_its_own_line() {
    let found = hits(FIXTURE);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![19, 20, 24], "{found:?}");
    assert!(found[0].1.contains("@v4"), "{}", found[0].1);
    assert!(found[1].1.contains("@main"), "{}", found[1].1);
    assert!(
        found[2].1.contains("ci.yml@v1"),
        "the reusable workflow's own `uses`: {}",
        found[2].1
    );
}

#[test]
fn the_immutable_forms_in_the_fixture_are_left_alone() {
    // Lines 10-13: a commit SHA, `./`, `$/path`, and `./path`.
    let lines: Vec<usize> = hits(FIXTURE).iter().map(|(line, _)| *line).collect();
    for accepted in 10..=13 {
        assert!(
            !lines.contains(&accepted),
            "line {accepted} is immutable and must not be reported"
        );
    }
}

#[test]
fn a_commit_pinned_step_is_not_reported() {
    let text = "jobs:\n  a:\n    timeout-minutes: 5\n    steps:\n      - uses: example/toolbox@0123456789abcdef0123456789abcdef01234567\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_tag_pinned_step_is_reported() {
    // The control for the test above.
    let text =
        "jobs:\n  a:\n    timeout-minutes: 5\n    steps:\n      - uses: example/toolbox@v4\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, 5, "the step's own line");
}

#[test]
fn a_reusable_workflow_called_at_a_tag_is_reported() {
    // The one `uses` that does not sit on a step.
    let text = "on: push\njobs:\n  call:\n    uses: owner/repo/.github/workflows/ci.yml@v1\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, 4, "the job's own `uses` line");
}

#[test]
fn the_declared_severity_is_warning() {
    // A moved tag is a supply-chain risk, not a run that cannot start.
    let finding = lint(FIXTURE)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "unpinned-action")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Warning);
}
