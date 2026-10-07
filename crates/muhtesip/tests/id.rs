//! The id rule: job and step id uniqueness, and the platform's id grammar.

use muhtesip::{Severity, lint};

/// A workflow with a repeated step id, a malformed step id, a repeated job id, and a malformed
/// job id.
const BAD: &str = include_str!("fixtures/ids.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "id")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

#[test]
fn the_fixture_reports_every_id_problem_in_line_order() {
    let found = hits(BAD);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![12, 14, 16, 20], "{found:?}");
    assert!(
        found[0].1.contains("'SETUP'") && found[0].1.contains("line 10"),
        "{:?}",
        found[0]
    );
    assert!(found[1].1.contains("'has space'"), "{:?}", found[1]);
    assert!(
        found[2].1.contains("job id 'build'") && found[2].1.contains("line 6"),
        "{:?}",
        found[2]
    );
    assert!(found[3].1.contains("'2fast'"), "{:?}", found[3]);
}

#[test]
fn a_job_id_finding_points_at_the_id_key_not_the_mapping_below_it() {
    // The second `build:` key is on line 16; its value mapping starts on line 17.
    let duplicate = hits(BAD)
        .into_iter()
        .find(|(_, message)| message.contains("already used at line 6"))
        .expect("one duplicate job id");
    assert_eq!(duplicate.0, 16, "the finding must point at the id itself");
}

#[test]
fn step_ids_may_repeat_across_different_jobs() {
    // Step ids are unique *within* a job, not across the workflow.
    let text = "jobs:\n  a:\n    steps:\n      - id: setup\n        run: echo a\n  b:\n    steps:\n      - id: setup\n        run: echo b\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_needs_reference_answers_to_the_same_grammar() {
    let text = "jobs:\n  a:\n    steps: []\n  b:\n    needs: \"my job\"\n    steps: []\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("my job"), "{:?}", found[0]);
}

#[test]
fn ids_are_compared_case_insensitively() {
    let text = "jobs:\n  Build:\n    steps: []\n  other:\n    needs: BUILD\n    steps: []\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_non_ascii_letter_is_not_a_valid_id() {
    // The platform's ids are ASCII, so a valid-looking Unicode letter is still rejected.
    let text = "jobs:\n  a:\n    steps:\n      - id: caf\u{e9}\n        run: echo\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn expressions_are_not_ids() {
    let text = "jobs:\n  a:\n    steps:\n      - id: ${{ matrix.thing }}\n        run: echo\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_sound_workflow_is_clean() {
    let text = "jobs:\n  build:\n    steps:\n      - id: setup\n        run: echo a\n      - id: build-step\n        run: echo b\n  release:\n    needs: build\n    steps:\n      - id: setup\n        run: echo c\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn the_declared_severity_is_error() {
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "id")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Error);
}
