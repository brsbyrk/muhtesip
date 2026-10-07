//! End-to-end lint tests over real workflow fixtures.

use muhtesip::{Registry, Severity, lint, lint_str};

const GOOD: &str = include_str!("fixtures/good.yaml");
const BAD: &str = include_str!("fixtures/bad.yaml");

#[test]
fn good_document_is_clean() {
    let findings = lint(GOOD).expect("good.yaml parses");
    assert!(
        findings.is_empty(),
        "expected no findings, got {findings:#?}"
    );
}

#[test]
fn bad_document_reports_both_rules_in_line_order() {
    let findings = lint(BAD).expect("bad.yaml parses");
    let summary: Vec<(&str, usize)> = findings.iter().map(|f| (f.rule, f.line)).collect();
    assert_eq!(
        summary,
        vec![("missing-timeout", 6), ("unpinned-action", 10)],
        "findings must be sorted by line, then rule — and a job-level finding points at the job's key"
    );
    assert_eq!(findings[0].severity, Severity::Note);
    assert_eq!(findings[1].severity, Severity::Warning);
}

#[test]
fn default_registry_ships_every_rule() {
    let mut ids = Registry::default().ids();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![
            "action-ref",
            "credentials",
            "deprecated-commands",
            "directive",
            "env-var",
            "events",
            "glob",
            "id",
            "if-cond",
            "job-needs",
            "matrix",
            "missing-timeout",
            "permissions",
            "runner-label",
            "shell-name",
            "unpinned-action"
        ]
    );
}

#[test]
fn empty_registry_reports_nothing() {
    let findings = lint_str(BAD, &Registry::new()).expect("bad.yaml parses");
    assert!(findings.is_empty());
}

#[test]
fn malformed_document_is_an_error_not_a_panic() {
    assert!(lint("jobs: [unclosed").is_err());
}

/// A KNOWN DEFECT, recorded so it cannot change silently.
///
/// A multi-line flow sequence whose first item is a quoted scalar is valid YAML — libyaml reads it,
/// and the platform runs workflows that contain it — but the `yaml-rust2` parser rejects it, so
/// muhtesip answers with a parse refusal and reports nothing for that file. Found by a run over a
/// pinned corpus of real workflows; `yaml-rust2` is
/// already at its newest release.
///
/// This test asserts the WRONG behaviour on purpose. When the dependency is fixed or replaced, it
/// will start failing, and that failure is the instruction: invert the assertion and delete the
/// note about it.
#[test]
fn a_quoted_first_item_in_a_multiline_flow_sequence_is_rejected() {
    let text = "jobs:\n  a:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    x: [\n      \"one\",\n    ]\n";
    assert!(
        lint(text).is_err(),
        "the flow-sequence defect is fixed: invert this test and remove the documented defect"
    );
}

/// An aliased value is a value: it must reach every rule that looks at it.
///
/// This was defect 2 — the event builder ignored `Event::Alias`, so a step
/// written `uses: *action` was *absent from the document*: not un-flagged, just gone, with no error
/// and no refusal. Fixed in our own code by keeping the dependency's anchor ids and resolving the
/// alias, so no new dependency and no `unsafe` was needed.
///
/// It also pins the line: an aliased node is *used* where the alias is written, so each of the three
/// steps is reported at its own line. Resolving to the anchor's line instead produced two identical
/// findings on line 6, which reads as a duplicate rather than two usages.
#[test]
fn an_aliased_step_is_flagged_like_any_other() {
    let text = "\
jobs:
  a:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: &action example/toolbox@v4
      - uses: *action
      - uses: other/toolbox@v4
";
    let findings = lint(text).expect("the document parses");
    let unpinned: Vec<&muhtesip::Finding> = findings
        .iter()
        .filter(|finding| finding.rule == "unpinned-action")
        .collect();

    assert_eq!(
        unpinned.len(),
        3,
        "all three steps are real: an independent parser sees 3, so 3 must be checked"
    );
    let lines: Vec<usize> = unpinned.iter().map(|finding| finding.line).collect();
    assert_eq!(
        lines,
        vec![6, 7, 8],
        "one finding per usage site, each at the line it is written on"
    );
}

/// An alias to an anchor the document never defines is refused, not dropped.
#[test]
fn a_dangling_alias_is_refused() {
    let text = "\
jobs:
  a:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - uses: *nowhere
";
    let error = lint(text).expect_err("a value that resolves to nothing cannot be linted");
    assert!(
        error.to_string().contains("anchor"),
        "the refusal names the reason: {error}"
    );
}

/// A merge key inserts the anchored mapping's entries, and the rules see them.
///
/// The control is the same job without the merge: a merged entry that reached no rule would leave the
/// `missing-timeout` finding in place, which is exactly the silent loss that made this construct a
/// refusal until now.
#[test]
fn a_merged_entry_reaches_the_rules() {
    let merged = "\
base: &base
  timeout-minutes: 5
jobs:
  build:
    <<: *base
    runs-on: ubuntu-latest
    steps:
      - run: echo hi
";
    let findings = lint(merged).expect("a merged document parses");
    assert!(
        findings
            .iter()
            .all(|finding| finding.rule != "missing-timeout"),
        "the merge supplied the timeout: {findings:?}"
    );

    let control = "\
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: echo hi
";
    assert!(
        lint(control)
            .expect("the control parses")
            .iter()
            .any(|finding| finding.rule == "missing-timeout"),
        "without the merge the finding is there, so the test above means something"
    );
}

/// A key the mapping writes itself wins over a merged one, wherever it is written.
///
/// Both orders are here because applying the merge when the mapping *ends* is what makes the second
/// one true.
#[test]
fn a_written_key_wins_over_a_merged_one_in_both_orders() {
    for text in [
        // the merge key first, the mapping's own key after it
        "base: &base\n  runs-on: nope-not-a-real-runner\njobs:\n  build:\n    <<: *base\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n      - run: echo hi\n",
        // the mapping's own key first, the merge key after it
        "base: &base\n  runs-on: nope-not-a-real-runner\njobs:\n  build:\n    runs-on: ubuntu-latest\n    <<: *base\n    timeout-minutes: 5\n    steps:\n      - run: echo hi\n",
    ] {
        let findings = lint(text).expect("a merged document parses");
        assert!(
            findings
                .iter()
                .all(|finding| finding.rule != "runner-label"),
            "the mapping's own key wins: {findings:?}"
        );
    }
}

/// In a list of mappings, the earlier one wins — and the finding points at the line the value is
/// written on, which is the anchor, not the merge.
#[test]
fn the_first_mapping_of_a_merge_list_wins() {
    let text = "\
first: &first
  runs-on: nope-not-a-real-runner
second: &second
  runs-on: ubuntu-latest
jobs:
  build:
    <<: [*first, *second]
    timeout-minutes: 5
    steps:
      - run: echo hi
";
    let findings: Vec<muhtesip::Finding> = lint(text)
        .expect("a merged document parses")
        .into_iter()
        .filter(|finding| finding.rule == "runner-label")
        .collect();
    assert_eq!(findings.len(), 1, "the earlier mapping wins: {findings:?}");
    assert_eq!(
        findings[0].line, 2,
        "the finding points at the line the value is written on"
    );
}

/// A merge key whose value has no mapping in it cannot be merged, so it is refused by name.
#[test]
fn a_merge_key_that_cannot_merge_is_refused() {
    for text in [
        "jobs:\n  build:\n    <<: 5\n",
        "one: &one\n  runs-on: ubuntu-latest\njobs:\n  build:\n    <<: [*one, 5]\n",
        "scalar: &scalar hello\njobs:\n  build:\n    <<: *scalar\n",
    ] {
        let error = lint(text).expect_err("an unusable merge must be refused");
        assert!(
            error.to_string().contains("merge key"),
            "the refusal names the construct: {error}"
        );
    }
}

/// An alias in key position that names a collection is not a merge — it is a key that cannot be a
/// key, and it is refused as its own construct rather than as a merge.
///
/// The spelling matters: the space before the colon is what lets the parser see an alias where a key
/// belongs. Without it (`*base: value`) the parser reads the alias name as `base:` and refuses it as
/// an unknown anchor — also an error, but the dependency's, and not the path under test here.
#[test]
fn a_key_that_is_not_a_scalar_is_refused_as_such() {
    let text = "\
base: &base
  runs-on: ubuntu-latest
jobs:
  build:
    *base : value
    timeout-minutes: 5
    steps:
      - run: echo hi
";
    let error = lint(text).expect_err("a collection cannot be a key");
    assert!(
        error.to_string().contains("key must be a scalar"),
        "the refusal names the construct: {error}"
    );
}

#[test]
fn a_reusable_workflow_job_call_is_checked_for_pinning_too() {
    // The one `uses` that does not sit on a step. It reached no rule at all before the model carried
    // it: a job calling another workflow was invisible to every check, which is the silent-loss class.
    let document = "on: push\njobs:\n  call:\n    uses: owner/repo/.github/workflows/ci.yml@v1\n";
    let summary: Vec<(&str, usize)> = lint(document)
        .expect("the document parses")
        .into_iter()
        .map(|finding| (finding.rule, finding.line))
        .collect();
    assert_eq!(
        summary,
        vec![("missing-timeout", 3), ("unpinned-action", 4)],
        "the job's own `uses` line is line 4, and the job's key line is 3"
    );
}

#[test]
fn a_job_calling_a_workflow_pinned_to_a_commit_reports_nothing_but_the_timeout() {
    // The control for the test above: without it, a rule that always reported would pass.
    let document = "on: push\njobs:\n  call:\n    uses: owner/repo/.github/workflows/ci.yml@11d5960a326750d5838078e36cf38b85af677262\n";
    let rules: Vec<&str> = lint(document)
        .expect("the document parses")
        .into_iter()
        .map(|finding| finding.rule)
        .collect();
    assert_eq!(rules, vec!["missing-timeout"], "{rules:?}");
}
