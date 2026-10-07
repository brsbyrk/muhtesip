//! The shell-name rule: a `shell:` the job's runner does not provide.

use muhtesip::{Severity, lint};

/// A workflow with a Windows shell on Linux, a Unix shell on Windows, and a misspelt shell.
const BAD: &str = include_str!("fixtures/shell_name.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "shell-name")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

/// A workflow with one step using `shell`, on a runner of the given platform.
fn step_with(runner: &str, shell: &str) -> String {
    format!(
        "jobs:\n  a:\n    runs-on: {runner}\n    steps:\n      - shell: {shell}\n        run: echo hi\n"
    )
}

#[test]
fn the_fixture_reports_each_unavailable_shell() {
    let found = hits(BAD);
    let lines: Vec<usize> = found.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![14, 21, 24], "{found:?}");
    assert!(
        found[0].1.contains("'powershell'") && found[0].1.contains("on macOS or Linux"),
        "{:?}",
        found[0]
    );
    assert!(
        found[1].1.contains("'sh'") && found[1].1.contains("on Windows"),
        "{:?}",
        found[1]
    );
    // A misspelt name gets no platform qualifier: it is not a shell anywhere.
    assert!(
        found[2].1.contains("'bsh'") && !found[2].1.contains(" on "),
        "{:?}",
        found[2]
    );
}

#[test]
fn a_shell_available_on_the_runner_is_accepted() {
    for (runner, shell) in [
        ("ubuntu-latest", "sh"),
        ("ubuntu-latest", "bash"),
        ("ubuntu-latest", "pwsh"),
        ("windows-latest", "cmd"),
        ("windows-latest", "powershell"),
        ("windows-latest", "bash"),
    ] {
        let text = step_with(runner, shell);
        assert!(
            hits(&text).is_empty(),
            "{shell} on {runner}: {:?}",
            hits(&text)
        );
    }
}

#[test]
fn a_custom_shell_is_not_flagged() {
    // `{0}` marks a custom shell: the runner command is the user's business.
    assert!(hits(&step_with("ubuntu-latest", "\"perl {0}\"")).is_empty());
    assert!(hits(&step_with("windows-latest", "\"ruby {0}\"")).is_empty());
}

#[test]
fn an_expression_is_not_a_shell_name() {
    assert!(hits(&step_with("ubuntu-latest", "${{ inputs.shell }}")).is_empty());
}

#[test]
fn a_runner_that_names_no_platform_accepts_any_known_shell() {
    // With no runner label to decide the platform, both families are possible.
    let text = "jobs:\n  a:\n    steps:\n      - shell: sh\n        run: echo hi\n      - shell: powershell\n        run: echo hi\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
}

#[test]
fn a_step_that_runs_no_script_has_no_shell_to_check() {
    let text = "jobs:\n  a:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: example/toolbox@0123456789abcdef0123456789abcdef01234567\n        shell: powershell\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
    // Control: the same `shell` on a step that *does* run a script is reported.
    assert_eq!(hits(&step_with("ubuntu-latest", "powershell")).len(), 1);
}

#[test]
fn the_declared_severity_is_error() {
    // The step fails before its script runs, so this is not advice.
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "shell-name")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Error);
}
