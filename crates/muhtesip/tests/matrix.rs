//! The matrix rule: repeated values in a row, and an `exclude` with nothing to remove.

use muhtesip::{Severity, lint};

/// A job with a repeated matrix value, and a job whose `exclude` removes nothing.
const BAD: &str = include_str!("fixtures/matrix.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "matrix")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

/// A job whose matrix body is the given lines, indented under `matrix:`.
fn job_with_matrix(body: &str) -> String {
    format!("jobs:\n  a:\n    strategy:\n      matrix:\n{body}    steps: []\n")
}

#[test]
fn the_fixture_reports_the_repeat_and_the_empty_exclude() {
    let found = hits(BAD);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![11, 20], "{found:?}");
    assert!(
        found[0].1.contains("'os'") && found[0].1.contains("ubuntu-latest"),
        "{:?}",
        found[0]
    );
    assert!(found[1].1.contains("'exclude'"), "{:?}", found[1]);
}

#[test]
fn a_repeat_is_reported_at_the_second_value() {
    let text = job_with_matrix("        os: [a, b, a]\n");
    let found = hits(&text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("'a'"), "{:?}", found[0]);
}

#[test]
fn distinct_values_are_not_reported() {
    let text =
        job_with_matrix("        os: [ubuntu-latest, macos-latest]\n        node: [18, 20]\n");
    assert!(hits(&text).is_empty(), "{:?}", hits(&text));
}

#[test]
fn the_same_value_in_different_rows_is_not_a_repeat() {
    // Two rows may legitimately hold the same value.
    let text = job_with_matrix("        os: [x]\n        arch: [x]\n");
    assert!(hits(&text).is_empty(), "{:?}", hits(&text));
}

#[test]
fn an_exclude_with_rows_to_filter_is_not_reported() {
    let text = job_with_matrix(
        "        os: [ubuntu-latest, windows-latest]\n        exclude:\n          - os: windows-latest\n",
    );
    assert!(hits(&text).is_empty(), "{:?}", hits(&text));
}

#[test]
fn an_exclude_alongside_an_include_is_not_reported() {
    // `include` can add a value the `exclude` then removes, so the pair is meaningful.
    let text = job_with_matrix(
        "        exclude:\n          - os: windows-latest\n        include:\n          - os: windows-latest\n",
    );
    assert!(hits(&text).is_empty(), "{:?}", hits(&text));
}

#[test]
fn a_matrix_built_from_an_expression_is_not_judged() {
    let text = job_with_matrix("        os: ${{ fromJSON(inputs.matrix) }}\n");
    assert!(hits(&text).is_empty(), "{:?}", hits(&text));
}

#[test]
fn the_declared_severity_is_warning() {
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "matrix")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Warning);
}
