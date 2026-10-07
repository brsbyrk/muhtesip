//! The glob rule: filter patterns the platform cannot match.

use muhtesip::{Severity, lint};

/// A workflow with an invalid branch pattern and an invalid path pattern.
const BAD: &str = include_str!("fixtures/glob.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "glob")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

/// A document triggering `push` with the given filter lines.
fn with_filters(filter: &str) -> String {
    format!("on:\n  push:\n{filter}\njobs:\n  a:\n    steps: []\n")
}

#[test]
fn the_fixture_reports_the_bad_branch_and_path_patterns() {
    let found = hits(BAD);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![5, 6], "{found:?}");
    assert!(found[0].1.contains("'foo bar'"), "{:?}", found[0]);
    assert!(found[1].1.contains("' src/x'"), "{:?}", found[1]);
}

#[test]
fn a_repetition_with_nothing_before_it_is_reported() {
    let found = hits(&with_filters("    branches: [\"?main\"]\n"));
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_valid_repetition_is_accepted() {
    // `v?` is "optionally v", not a mistake: the preceding character is ordinary.
    let found = hits(&with_filters(
        "    branches: [\"v?\", \"v1.+\", \"releases/**\"]\n",
    ));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn every_ref_filter_is_checked() {
    for key in ["branches", "branches-ignore", "tags", "tags-ignore"] {
        let found = hits(&with_filters(&format!("    {key}: [\"a b\"]\n")));
        assert_eq!(found.len(), 1, "{key}: {found:?}");
    }
}

#[test]
fn a_path_filter_is_checked_as_a_path_not_a_ref() {
    // A space is fine in a path pattern and fatal in a ref name.
    let found = hits(&with_filters("    paths: [\"my dir/**\"]\n"));
    assert!(found.is_empty(), "{found:?}");

    let found = hits(&with_filters("    paths-ignore: [\" src/**\"]\n"));
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn a_filter_that_is_not_a_pattern_is_ignored() {
    // `types` holds event activity names, not patterns.
    let found = hits(&with_filters("    types: [opened, \"a b\"]\n"));
    assert!(found.is_empty(), "{found:?}");
    // Control: a pattern filter with the same bad value is reported.
    assert_eq!(hits(&with_filters("    branches: [\"a b\"]\n")).len(), 1);
}

#[test]
fn a_pattern_built_from_an_expression_is_not_judged() {
    let found = hits(&with_filters("    branches: [\"${{ github.ref }}\"]\n"));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_document_without_filters_is_not_this_rules_business() {
    let text = "on: push\njobs:\n  a:\n    steps: []\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
    // Control: a document *with* an invalid filter is reported.
    assert_eq!(hits(&with_filters("    branches: [\"a b\"]\n")).len(), 1);
}

#[test]
fn the_declared_severity_is_error() {
    // The platform rejects the workflow before it ever runs.
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "glob")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Error);
}
