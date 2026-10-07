//! The missing-timeout rule: a job with no `timeout-minutes` bound.
//!
//! This rule and `unpinned-action` were the two without a fixture or test file of their own — their
//! compliant-input (negative) case existed only as a side effect of `tests/lint.rs::good_document_is_clean`.
//! A mutation audit of the suite named them, so each now has its own fixture with both a
//! bounded and an unbounded job, making the negative case local and explicit.

use muhtesip::{Severity, lint};

const FIXTURE: &str = include_str!("fixtures/missing_timeout.yaml");

/// This rule's findings for a document, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "missing-timeout")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

#[test]
fn the_fixture_reports_only_the_unbounded_job_at_its_key_line() {
    let found = hits(FIXTURE);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, 12, "the finding points at the job's key line");
    assert!(found[0].1.contains("'unbounded'"), "{:?}", found[0]);
}

#[test]
fn a_job_with_a_bound_is_not_reported() {
    let text = "jobs:\n  a:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n      - run: echo hi\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_job_without_a_bound_is_reported() {
    // The control for the test above: without it, a rule that never reported would pass.
    let text = "jobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo hi\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, 2, "the job's key line");
}

#[test]
fn a_document_with_no_jobs_is_not_this_rules_business() {
    assert!(hits("on: [push]\n").is_empty());
}

#[test]
fn the_declared_severity_is_note() {
    // A missing bound is advice, not a security or scheduling failure on its own.
    let finding = lint(FIXTURE)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "missing-timeout")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Note);
}
