//! End-to-end tests of the `muhtesip` binary: exit codes and output shape.
//!
//! The library is tested in `lint.rs`; here we only assert the thin CLI contract — what it
//! prints, what it writes to stderr, and the exit code it returns.

// The fixtures live with the library's tests — they are the documents the rules are tested against,
// and a second copy would be a different document that drifts. This crate reaches across for them.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../muhtesip/tests/fixtures")
        .join(name)
        .to_str()
        .expect("fixture path is valid UTF-8")
        .to_string()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .args(args)
        .output()
        .expect("the muhtesip binary runs")
}

/// Run the binary with `input` on standard input and no file arguments.
fn run_stdin(input: &str) -> Output {
    run_stdin_with(&[], input)
}

/// Run the binary with `args`, feeding `input` on standard input.
fn run_stdin_with(args: &[&str], input: &str) -> Output {
    run_env(args, input, &[])
}

/// Run the binary with `args`, `input` on standard input, and `envs` added to its environment.
///
/// The two variables that decide color for a child are cleared first, so a test sets exactly what it
/// means to test rather than inheriting a developer's terminal preferences.
fn run_env(args: &[&str], input: &str, envs: &[(&str, &str)]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .args(args)
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR_FORCE")
        .envs(envs.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the muhtesip binary spawns");
    // The child may refuse its arguments and exit before reading a byte of this, which makes the write
    // fail with EPIPE. That is the behaviour some of these tests are *about* — `--fail-on` with no value
    // is refused immediately — so a broken pipe is not a test failure, and asserting otherwise made this
    // helper flaky: the write won the race on a fast machine and lost it on CI. Any other write error is
    // still a failure, because it would mean something unrelated went wrong.
    if let Err(error) = child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
    {
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::BrokenPipe,
            "stdin accepts the document: {error}"
        );
    }
    child.wait_with_output().expect("the child exits")
}

#[test]
fn clean_document_exits_zero_and_prints_nothing() {
    let out = run(&[&fixture("good.yaml")]);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty(), "a clean file prints no findings");
    assert!(out.stderr.is_empty());
}

#[test]
fn problems_exit_one_with_one_line_per_finding_in_line_order() {
    let out = run(&[&fixture("bad.yaml")]);
    assert_eq!(out.status.code(), Some(1));

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "one line per finding: {stdout:?}");
    assert!(lines[0].contains("missing-timeout"), "first: {}", lines[0]);
    assert!(lines[0].contains("bad.yaml"), "path prefix: {}", lines[0]);
    assert!(lines[1].contains("unpinned-action"), "second: {}", lines[1]);
}

#[test]
fn clean_document_on_stdin_exits_zero() {
    let out = run_stdin(include_str!("../../muhtesip/tests/fixtures/good.yaml"));
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty());
}

#[test]
fn malformed_document_exits_two_not_one() {
    let out = run_stdin("jobs: [unclosed\n");
    assert_eq!(out.status.code(), Some(2), "a parse error is 2, not 1");
    assert!(out.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("muhtesip:"),
        "the error is reported on stderr"
    );
}

#[test]
fn missing_file_exits_two() {
    let out = run(&["/nonexistent/definitely-not-a-workflow.yaml"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
}

// --- --format json: the machine surface, asserted through the real binary ---

/// Parse stdout as the envelope, insisting it is well formed.
///
/// No `panic!` here: an integration test file is its own crate, so the library's
/// `cfg(test)` lint allowances do not reach it, and this crate denies panics.
fn envelope(out: &Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let parsed = serde_json::from_str(&stdout);
    assert!(parsed.is_ok(), "stdout is valid JSON: {stdout:?}");
    parsed.expect("parsing was just asserted to have succeeded")
}

#[test]
fn json_reports_the_same_findings_as_the_plain_renderer() {
    let plain = run(&[&fixture("bad.yaml")]);
    let json = run(&["--format", "json", &fixture("bad.yaml")]);

    assert_eq!(json.status.code(), Some(1));
    let value = envelope(&json);
    assert_eq!(value["envelope"], "muhtesip/1");

    // The envelope's own text fields must reassemble exactly what the plain renderer printed.
    let texts: Vec<String> = value["sources"][0]["findings"]
        .as_array()
        .expect("findings is an array")
        .iter()
        .map(|f| f["text"].as_str().expect("text is a string").to_owned())
        .collect();
    assert!(!texts.is_empty(), "the proof must not be vacuous");
    assert_eq!(
        texts.join("\n") + "\n",
        String::from_utf8_lossy(&plain.stdout),
        "one derivation, two encodings"
    );
}

#[test]
fn json_carries_a_named_refusal_when_the_document_will_not_parse() {
    let out = run_stdin_with(&["--format", "json"], "jobs: [unclosed\n");
    assert_eq!(out.status.code(), Some(2));
    let value = envelope(&out);
    assert_eq!(value["sources"][0]["refusal"]["name"], "PARSE_ERROR");
    assert_eq!(
        value["sources"][0]["findings"].as_array().map(Vec::len),
        Some(0),
        "a refused source reports no findings"
    );
}

#[test]
fn json_on_a_clean_file_is_a_valid_empty_envelope_and_exits_zero() {
    let out = run(&["--format", "json", &fixture("good.yaml")]);
    assert_eq!(out.status.code(), Some(0));
    let value = envelope(&out);
    assert!(value["refusal"].is_null(), "clean is not a refusal");
    assert_eq!(
        value["sources"][0]["findings"].as_array().map(Vec::len),
        Some(0)
    );
}

#[test]
fn json_lints_every_source_given() {
    let out = run(&[
        "--format",
        "json",
        &fixture("good.yaml"),
        &fixture("bad.yaml"),
    ]);
    assert_eq!(out.status.code(), Some(1));
    let value = envelope(&out);
    assert_eq!(value["sources"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        value["sources"][0]["findings"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        value["sources"][1]["findings"].as_array().map(Vec::len),
        Some(2)
    );
}

#[test]
fn an_unknown_format_is_refused_rather_than_replaced() {
    // Every format the roadmap names is now implemented, so this covers names that are not formats
    // at all. A consumer asking for a format must never silently receive a different one.
    for name in ["xml", "yaml", "csv"] {
        let out = run(&["--format", name, &fixture("good.yaml")]);
        assert_eq!(out.status.code(), Some(2), "format {name}");
        assert!(
            out.stdout.is_empty(),
            "a refused format must not print another encoding: {name}"
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(name),
            "the message names the format: {name}"
        );
    }
}

#[test]
fn a_repeated_format_flag_is_refused() {
    let out = run(&[
        "--format",
        "json",
        "--format",
        "plain",
        &fixture("good.yaml"),
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("more than once"));
}

// --- inline suppressions, through the real binary ---

#[test]
fn an_inline_directive_suppresses_its_finding() {
    let out = run(&[&fixture("directives.yaml")]);
    assert_eq!(out.status.code(), Some(1), "one finding survives");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().count(), 1, "{stdout:?}");
    assert!(stdout.contains("other/toolbox"), "{stdout}");
    assert!(
        !stdout.contains("example/toolbox"),
        "the suppressed finding is gone: {stdout}"
    );
}

#[test]
fn json_reflects_inline_suppression() {
    let out = run(&["--format", "json", &fixture("directives.yaml")]);
    assert_eq!(out.status.code(), Some(1));
    let value = envelope(&out);
    assert_eq!(
        value["sources"][0]["findings"].as_array().map(Vec::len),
        Some(1),
        "the envelope carries only the surviving finding"
    );
}

#[test]
fn a_directive_can_suppress_everything_and_the_run_is_clean() {
    let text = include_str!("../../muhtesip/tests/fixtures/directives.yaml").replace(
        "      - uses: other/toolbox@v4",
        "      - uses: other/toolbox@v4 # muhtesip: ignore[unpinned-action]",
    );
    let out = run_stdin_with(&[], &text);
    assert_eq!(out.status.code(), Some(0), "nothing left to report");
    assert!(out.stdout.is_empty());
}

// --- --format annotations, through the real binary ---

#[test]
fn annotations_emit_one_log_command_per_finding() {
    let out = run(&["--format", "annotations", &fixture("bad.yaml")]);
    assert_eq!(out.status.code(), Some(1));

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout:?}");
    assert!(lines[0].starts_with("::notice "), "{}", lines[0]);
    assert!(lines[1].starts_with("::warning "), "{}", lines[1]);
    assert!(lines[0].contains("title=missing-timeout"), "{}", lines[0]);
}

#[test]
fn annotations_report_a_parse_failure_as_a_named_annotation() {
    let out = run_stdin_with(&["--format", "annotations"], "jobs: [unclosed\n");
    assert_eq!(out.status.code(), Some(2));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("::error "), "{stdout}");
    assert!(stdout.contains("title=PARSE_ERROR"), "{stdout}");
}

// --- --format sarif, through the real binary ---

#[test]
fn sarif_emits_one_document_and_exits_one() {
    let out = run(&["--format", "sarif", &fixture("bad.yaml")]);
    assert_eq!(out.status.code(), Some(1));

    let value = envelope(&out);
    assert_eq!(value["version"].as_str(), Some("2.1.0"));
    assert_eq!(
        value["runs"][0]["results"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        value["runs"][0]["invocations"][0]["executionSuccessful"].as_bool(),
        Some(true)
    );
}

#[test]
fn sarif_marks_a_parse_failure_unsuccessful() {
    let out = run_stdin_with(&["--format", "sarif"], "jobs: [unclosed\n");
    assert_eq!(out.status.code(), Some(2));

    let value = envelope(&out);
    let invocation = &value["runs"][0]["invocations"][0];
    assert_eq!(invocation["executionSuccessful"].as_bool(), Some(false));
    assert_eq!(
        invocation["toolExecutionNotifications"][0]["descriptor"]["id"].as_str(),
        Some("PARSE_ERROR")
    );
}

// --- directory arguments (C3) and --init-config (C4), through the real binary ---

/// A counter so two tests never share a directory, even in one process.
static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh directory under the system temp dir.
fn temp_dir(label: &str) -> PathBuf {
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "muhtesip-cli-{label}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("the temp directory is created");
    dir
}

/// Run the binary with `dir` as its working directory.
fn run_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("the muhtesip binary runs")
}

#[test]
fn a_project_directory_expands_to_its_workflow_directory() {
    let dir = temp_dir("expand");
    let workflows = dir.join(".github/workflows");
    std::fs::create_dir_all(&workflows).expect("the workflows directory is created");
    std::fs::copy(fixture("bad.yaml"), workflows.join("b.yaml")).expect("copy");
    std::fs::copy(fixture("good.yaml"), workflows.join("a.yaml")).expect("copy");
    // A YAML file outside the workflow directory must not be linted as a workflow.
    std::fs::copy(fixture("bad.yaml"), dir.join("docker-compose.yaml")).expect("copy");

    let out = run(&[dir.to_str().expect("the temp path is UTF-8")]);
    assert_eq!(out.status.code(), Some(1));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(stdout.lines().count(), 2, "{stdout}");
    assert!(stdout.contains(".github/workflows/b.yaml"), "{stdout}");
    assert!(
        !stdout.contains("docker-compose"),
        "only the workflow directory is searched: {stdout}"
    );
}

#[test]
fn the_workflow_directory_itself_can_be_given() {
    let dir = temp_dir("expand-direct");
    let workflows = dir.join(".github/workflows");
    std::fs::create_dir_all(&workflows).expect("the workflows directory is created");
    std::fs::copy(fixture("bad.yaml"), workflows.join("a.yml")).expect("copy");

    let out = run(&[workflows.to_str().expect("UTF-8")]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("a.yml"));
}

#[test]
fn a_directory_with_no_workflow_files_answers_empty_and_exits_zero() {
    let dir = temp_dir("empty");
    let out = run(&[dir.to_str().expect("UTF-8")]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "nothing to lint is not a failure: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty(), "and there is nothing to report");
}

#[test]
fn a_directory_argument_reports_every_file_it_checked_in_order() {
    let dir = temp_dir("order");
    let workflows = dir.join(".github/workflows");
    std::fs::create_dir_all(&workflows).expect("the workflows directory is created");
    for name in ["c.yaml", "a.yaml", "b.yml"] {
        std::fs::copy(fixture("good.yaml"), workflows.join(name)).expect("copy");
    }

    let out = run(&["--format", "json", dir.to_str().expect("UTF-8")]);
    assert_eq!(out.status.code(), Some(0));
    let value = envelope(&out);
    let sources: Vec<&str> = value["sources"]
        .as_array()
        .expect("sources is an array")
        .iter()
        .map(|source| source["source"].as_str().expect("a source label"))
        .collect();
    let names: Vec<&str> = sources
        .iter()
        .map(|s| s.rsplit('/').next().unwrap_or(s))
        .collect();
    assert_eq!(
        names,
        ["a.yaml", "b.yml", "c.yaml"],
        "sorted, so two runs can be diffed"
    );
}

#[test]
fn init_config_writes_a_file_the_tool_then_reads() {
    let dir = temp_dir("init");
    let out = run_in(&dir, &["--init-config"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains(".muhtesip.yaml"));

    let written =
        std::fs::read_to_string(dir.join(".muhtesip.yaml")).expect("the file was written");
    assert!(written.contains("rules:"), "{written}");

    // The point of the file is that the tool can read it back: discovery finds it in the working
    // directory, and it must not be a configuration error.
    let lint = run_in(&dir, &[&fixture("good.yaml")]);
    assert_eq!(
        lint.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&lint.stderr)
    );
}

#[test]
fn init_config_refuses_to_overwrite_an_existing_file() {
    let dir = temp_dir("init-twice");
    assert_eq!(run_in(&dir, &["--init-config"]).status.code(), Some(0));

    let again = run_in(&dir, &["--init-config"]);
    assert_eq!(again.status.code(), Some(2), "never clobber a file");
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already exists"),
        "and say why"
    );
}

#[test]
fn init_config_refuses_combinations_it_has_no_meaning_for() {
    let dir = temp_dir("init-combos");
    for args in [
        vec!["--init-config", "--format", "json"],
        vec!["--init-config", "some.yml"],
        vec!["--init-config", "--config", "other.yaml"],
    ] {
        let out = run_in(&dir, &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            out.stdout.is_empty(),
            "an unusable command line produces no report: {args:?}"
        );
    }
}

// --- --fail-on: the exit code follows severity, not the mere presence of findings ---

/// A document whose only finding is an advisory note.
const NOTE_ONLY: &str = "\
name: ci
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo hi
";

/// A document whose only finding is a warning.
const WARNING_ONLY: &str = "\
name: ci
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: example/toolbox@v4
";

#[test]
fn a_note_does_not_fail_the_run_by_default() {
    let out = run_stdin(NOTE_ONLY);
    assert_eq!(
        out.status.code(),
        Some(0),
        "advice must not fail a build: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    // The finding is still reported — it just does not fail the run.
    assert!(String::from_utf8_lossy(&out.stdout).contains("missing-timeout"));
}

#[test]
fn asking_to_fail_on_notes_makes_a_note_fail() {
    let out = run_stdin_with(&["--fail-on", "note"], NOTE_ONLY);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn a_warning_fails_by_default() {
    let out = run_stdin(WARNING_ONLY);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn raising_the_threshold_to_error_lets_a_warning_pass() {
    let out = run_stdin_with(&["--fail-on", "error"], WARNING_ONLY);
    assert_eq!(out.status.code(), Some(0), "below the stated threshold");
    // Reported either way: the threshold decides the exit code, not the output.
    assert!(String::from_utf8_lossy(&out.stdout).contains("unpinned-action"));
}

#[test]
fn an_unknown_severity_is_refused_and_the_message_names_all_of_them() {
    let out = run_stdin_with(&["--fail-on", "nope"], NOTE_ONLY);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Derived from the type, so a new severity cannot leave this message incomplete.
    for name in ["error", "warning", "note"] {
        assert!(stderr.contains(name), "message omits '{name}': {stderr}");
    }
}

#[test]
fn fail_on_without_a_value_is_refused() {
    let out = run_stdin_with(&["--fail-on"], NOTE_ONLY);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("severity name"));
}

#[test]
fn fail_on_does_not_combine_with_init_config() {
    let out = run(&["--fail-on", "note", "--init-config"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--init-config"));
}

// --- the two flags that answer and stop ---

#[test]
fn version_prints_the_crate_version_and_exits_clean() {
    let out = run_stdin_with(&["--version"], "");
    assert_eq!(
        out.status.code(),
        Some(0),
        "asking for the version is not a failure"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("muhtesip {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_prints_the_usage_and_exits_clean() {
    let out = run_stdin_with(&["--help"], "");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    for expected in [
        "usage:",
        "--format",
        "--fail-on",
        "--init-config",
        "--version",
        "exit codes",
    ] {
        assert!(
            stdout.contains(expected),
            "the usage names {expected}: {stdout}"
        );
    }
}

#[test]
fn two_actions_at_once_are_refused() {
    // Two actions cannot both happen, and the message names all three rather than whichever pair the
    // caller happened to type.
    for args in [
        ["--version", "--help"],
        ["--init-config", "--help"],
        ["--version", "--version"],
    ] {
        let out = run_stdin_with(&args, "");
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("only one of"), "{args:?}: {stderr}");
    }
}

// --- argument forms ---

#[test]
fn an_inline_value_is_the_same_request_as_a_separate_one() {
    // `--flag=value` and `--flag value` must be one request, not two similar ones: the same bytes,
    // and the same exit code, or the `=` form is a second parser nobody tested.
    let inline = run(&["--format=json", &fixture("bad.yaml")]);
    let separate = run(&["--format", "json", &fixture("bad.yaml")]);
    assert_eq!(inline.stdout, separate.stdout);
    assert_eq!(inline.status.code(), separate.status.code());
}

#[test]
fn two_dashes_end_the_options() {
    let out = run(&["--", &fixture("bad.yaml")]);
    assert_eq!(out.status.code(), Some(1), "the file after `--` is linted");
    assert!(String::from_utf8_lossy(&out.stdout).contains("missing-timeout"));
}

#[test]
fn a_value_on_a_flag_that_takes_none_is_refused() {
    let out = run(&["--version=x"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("takes no value"),
        "{:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn an_empty_inline_value_is_refused() {
    // `--format=` is not `--format` with no value: it is a name that is not a name, and saying so
    // beats a message about a missing argument.
    let out = run(&["--format=", &fixture("bad.yaml")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("empty value"));
}

#[test]
fn a_typo_names_the_flag_they_meant() {
    let out = run(&["--formt", "json", &fixture("bad.yaml")]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("did you mean '--format'"), "{stderr}");
}

#[test]
fn a_flag_resembling_nothing_is_not_guessed_at() {
    let out = run(&["--nonsense", &fixture("bad.yaml")]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("did you mean"), "{stderr}");
}

#[test]
fn a_prefix_of_a_flag_is_suggested() {
    // `--fail` is not a typo, it is half a flag; suggesting the whole one is still the useful answer.
    let out = run(&["--fail", "error", &fixture("bad.yaml")]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("'--fail-on'"));
}

// --- color ---

#[test]
fn color_is_off_when_the_destination_is_not_a_terminal() {
    // Captured output is not a terminal, which is the case that must stay clean: a log file, a step
    // capturing the output, or a scrollback someone will grep.
    let out = run_env(&[&fixture("bad.yaml")], "", &[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("missing-timeout"),
        "the findings still print"
    );
    assert!(
        !stdout.contains('\u{1b}'),
        "but with no escapes: {stdout:?}"
    );
}

#[test]
fn color_can_be_forced_and_changes_nothing_but_the_color() {
    let plain = run_env(&[&fixture("bad.yaml")], "", &[]);
    let colored = run_env(&[&fixture("bad.yaml")], "", &[("CLICOLOR_FORCE", "1")]);
    let colored = String::from_utf8_lossy(&colored.stdout).to_string();
    assert!(colored.contains("\u{1b}["), "color on request: {colored:?}");
    // Color is presentation, not a second rendering: strip the escapes and the bytes are the same.
    assert_eq!(
        strip_escapes(&colored),
        String::from_utf8_lossy(&plain.stdout)
    );
}

#[test]
fn no_color_outranks_a_request_for_color() {
    // A person saying no once outranks a program asking: the person is the one who has to read it.
    let out = run_env(
        &[&fixture("bad.yaml")],
        "",
        &[("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains('\u{1b}'), "NO_COLOR wins: {stdout:?}");
}

/// `text` with SGR sequences removed, so colored and plain output can be compared as content.
fn strip_escapes(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            for character in chars.by_ref() {
                if character == 'm' {
                    break;
                }
            }
        } else {
            out.push(character);
        }
    }
    out
}
