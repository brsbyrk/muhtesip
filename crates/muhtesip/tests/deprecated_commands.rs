//! The deprecated-commands rule: retired workflow commands inside a `run:` script.

use muhtesip::{Severity, lint};

/// A workflow using `::set-output` on one line, and `::save-state` + `::add-path` in a script.
const BAD: &str = include_str!("fixtures/deprecated_commands.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "deprecated-commands")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

#[test]
fn the_fixture_reports_each_retired_command() {
    let found = hits(BAD);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![11, 14, 15], "{found:?}");
    assert!(found[0].1.contains("set-output"), "{}", found[0].1);
    assert!(found[1].1.contains("save-state"), "{}", found[1].1);
    assert!(found[2].1.contains("add-path"), "{}", found[2].1);
}

#[test]
fn a_finding_inside_a_multi_line_script_points_at_the_command_line() {
    // The script starts on line 13; `::save-state` is on 14 and `::add-path` on 15.
    let found = hits(BAD);
    let save_state = found
        .iter()
        .find(|(_, message)| message.contains("save-state"))
        .expect("one");
    assert_eq!(
        save_state.0, 14,
        "the command's own line, not the script's first"
    );
}

#[test]
fn each_message_names_the_replacement() {
    let found = hits(BAD);
    assert!(found[0].1.contains("$GITHUB_OUTPUT"), "{}", found[0].1);
    assert!(found[1].1.contains("$GITHUB_STATE"), "{}", found[1].1);
    assert!(found[2].1.contains("$GITHUB_PATH"), "{}", found[2].1);
}

#[test]
fn set_env_is_detected_too() {
    let text = "jobs:\n  a:\n    steps:\n      - run: echo \"::set-env name=FOO::bar\"\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("$GITHUB_ENV"), "{}", found[0].1);
}

#[test]
fn a_longer_word_is_not_the_retired_command() {
    let text = "jobs:\n  a:\n    steps:\n      - run: echo \"::set-outputs is not a command\"\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_script_without_retired_commands_is_clean() {
    let text = "jobs:\n  a:\n    steps:\n      - run: echo \"::notice title=hi::there\"\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_step_without_a_script_is_not_this_rules_business() {
    let text = "jobs:\n  a:\n    steps:\n      - uses: example/toolbox@0123456789abcdef0123456789abcdef01234567\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
    // Control: a step that *does* run a retired command is reported.
    assert_eq!(
        hits("jobs:\n  a:\n    steps:\n      - run: echo \"::set-output name=x::1\"\n").len(),
        1
    );
}

#[test]
fn the_declared_severity_is_warning() {
    // Deprecated, not yet fatal: `set-output` still works today, `set-env` no longer does. A
    // warning is the honest tier for "this will break", not "this is broken".
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "deprecated-commands")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Warning);
}

// --- The command form matters: a mention is not a command ---

/// A retired name with no closing `::` is not a command: the platform reads `::name params::data`, so
/// this is echoed text and the platform would not act on it either.
///
/// Found by the labelled set (a hand-written clean workflow kept as the control there): the rule used
/// to warn about any occurrence of the name, which is the shape that turns a precision claim into a
/// nuisance.
#[test]
fn a_name_without_a_closing_separator_is_not_a_command() {
    for text in [
        // echoed text that mentions the name
        "jobs:\n  a:\n    steps:\n      - run: echo \"::set-output test\"\n",
        // a migration note in a comment inside the script
        "jobs:\n  a:\n    steps:\n      - run: |\n          # replaced ::set-output with $GITHUB_OUTPUT\n          echo hi\n",
        // the bare name, which sets nothing
        "jobs:\n  a:\n    steps:\n      - run: echo \"::set-output\"\n",
        // a longer word (as before), now for two reasons
        "jobs:\n  a:\n    steps:\n      - run: echo \"::set-outputs x::y\"\n",
    ] {
        assert!(
            hits(text).is_empty(),
            "must not report: {text:?} -> {:?}",
            hits(text)
        );
    }
}

/// One name, three real command forms — each with the closing `::` the platform needs.
#[test]
fn every_command_form_the_platform_reads_is_still_reported() {
    for text in [
        "jobs:\n  a:\n    steps:\n      - run: echo \"::set-output name=x::1\"\n",
        "jobs:\n  a:\n    steps:\n      - run: echo \"::save-state name=x::1\"\n",
        "jobs:\n  a:\n    steps:\n      - run: echo \"::add-path::/opt/bin\"\n",
    ] {
        assert_eq!(
            hits(text).len(),
            1,
            "must report: {text:?} -> {:?}",
            hits(text)
        );
    }
}
