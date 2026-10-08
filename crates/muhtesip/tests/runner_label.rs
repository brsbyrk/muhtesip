//! The runner-label rule: labels GitHub hosts pass, everything else is reported.

use muhtesip::{Severity, lint};

/// A workflow whose only defect is a misspelt runner label.
const UNKNOWN: &str = include_str!("fixtures/runner.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn runner_findings(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "runner-label")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

/// A document with one job per label, so a whole family can be checked at once.
fn jobs_over(labels: &[&str]) -> String {
    let mut text = String::from("jobs:\n");
    for (index, label) in labels.iter().enumerate() {
        text.push_str(&format!(
            "  j{index}:\n    runs-on: {label}\n    steps: []\n"
        ));
    }
    text
}

#[test]
fn the_fixture_reports_exactly_one_unknown_label_at_the_value_line() {
    let hits = runner_findings(UNKNOWN);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].0, 8, "the finding points at the `runs-on` value");
    assert!(hits[0].1.contains("ubuntu-latests"), "{}", hits[0].1);
}

#[test]
fn the_declared_severity_is_error() {
    let finding = lint(UNKNOWN)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "runner-label")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Error);
}

#[test]
fn documented_hosted_labels_are_accepted_across_every_family() {
    // One per family, including the labels actionlint's list lacks.
    let labels = [
        "ubuntu-latest",
        "ubuntu-26.04-arm",
        "windows-2025-vs2026",
        "windows-11-vs2026-arm",
        "macos-26-xlarge",
        "xcode-27",
        "ubuntu-slim",
    ];
    assert!(
        runner_findings(&jobs_over(&labels)).is_empty(),
        "a documented label must never be flagged"
    );
}

#[test]
fn a_misspelt_label_in_a_sequence_is_reported() {
    let text = "jobs:\n  build:\n    runs-on: [ubuntu-latest, ubuntu-24.4]\n    steps: []\n";
    let hits = runner_findings(text);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].1.contains("ubuntu-24.4"), "{}", hits[0].1);
}

#[test]
fn a_self_hosted_list_is_not_checked_for_custom_labels() {
    let text =
        "jobs:\n  build:\n    runs-on: [self-hosted, linux, x64, my-custom-label]\n    steps: []\n";
    assert!(
        runner_findings(text).is_empty(),
        "a self-hosted runner's own labels are unknowable, so nothing may be flagged"
    );
}

#[test]
fn an_expression_is_not_a_label() {
    let text = "jobs:\n  build:\n    runs-on: ${{ matrix.os }}\n    steps: []\n";
    assert!(runner_findings(text).is_empty());
}

#[test]
fn a_self_hosted_preset_alone_is_accepted() {
    assert!(runner_findings(&jobs_over(&["linux"])).is_empty());
    assert!(runner_findings(&jobs_over(&["macos"])).is_empty());
}

#[test]
fn a_job_without_runs_on_is_not_this_rules_business() {
    let text = "jobs:\n  build:\n    steps: []\n";
    assert!(runner_findings(text).is_empty());
    // Control: a misspelt label *is* reported.
    assert_eq!(runner_findings(&jobs_over(&["ubuntu-latests"])).len(), 1);
}

#[test]
fn labels_naming_different_systems_conflict() {
    let text = "jobs:\n  build:\n    runs-on: [ubuntu-latest, windows-2022]\n    steps: []\n";
    let hits = runner_findings(text);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(
        hits[0].1.contains("different operating systems"),
        "{}",
        hits[0].1
    );
    assert!(
        hits[0].1.contains("Linux") && hits[0].1.contains("Windows"),
        "{}",
        hits[0].1
    );
}

#[test]
fn two_versions_of_one_system_do_not_conflict() {
    // Deliberately narrower than actionlint: a version-level conflict claim false-positives on real
    // files, and a false report costs more than a missing one.
    let text = "jobs:\n  build:\n    runs-on: [ubuntu-latest, ubuntu-22.04]\n    steps: []\n";
    assert!(
        runner_findings(text).is_empty(),
        "{:?}",
        runner_findings(text)
    );
}

#[test]
fn a_single_label_never_conflicts() {
    assert!(runner_findings(&jobs_over(&["ubuntu-latest"])).is_empty());
}

#[test]
fn a_self_hosted_list_is_not_checked_for_conflicts_either() {
    // The list is skipped wholesale, so a mixed self-hosted list is silent rather than wrong.
    let text = "jobs:\n  build:\n    runs-on: [self-hosted, linux, windows]\n    steps: []\n";
    assert!(runner_findings(text).is_empty());
}
