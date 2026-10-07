//! Inline suppressions, end to end: the directive is the only difference between the two runs.

use muhtesip::directives::Directives;
use muhtesip::{Finding, Registry, RuleOverride, RuleSettings, Severity, lint, lint_str};

/// The fixture: two unpinned actions, the first one suppressed.
const WITH: &str = include_str!("fixtures/directives.yaml");

/// The findings that survive the text's own directives.
fn kept(text: &str) -> Vec<Finding> {
    let directives = Directives::parse(text);
    lint(text)
        .expect("the fixture parses")
        .into_iter()
        .filter(|finding| !directives.ignores(finding.line, finding.rule))
        .collect()
}

/// The fixture with its directive removed — the control.
fn control() -> String {
    WITH.replace(" # muhtesip: ignore[unpinned-action]", "")
}

#[test]
fn a_directive_suppresses_exactly_the_finding_it_covers() {
    let before = kept(&control());
    let after = kept(WITH);

    assert_eq!(before.len(), 2, "the control reports both actions");
    assert_eq!(after.len(), 1, "the directive removes one of them");
    assert_eq!(before.len() - after.len(), 1, "and only one");
    assert_eq!(after[0].line, 11, "the untouched step still reports");
    assert!(
        after[0].message.contains("other/toolbox"),
        "the surviving finding is the one without the directive: {}",
        after[0].message
    );
}

#[test]
fn a_directive_for_a_different_rule_suppresses_nothing() {
    let text = WITH.replace("unpinned-action]", "missing-timeout]");
    assert_eq!(
        kept(&text).len(),
        2,
        "naming another rule must not suppress this one"
    );
}

#[test]
fn the_marker_has_to_be_exact() {
    for wrong in [
        "# muhtesip: ignore [unpinned-action]", // a space before the bracket
        "#muhtesip: ignore[unpinned-action]",   // no space after the hash
        "# Muhtesip: ignore[unpinned-action]",  // wrong case
        "# muhtesip ignore[unpinned-action]",   // no colon
        "# muhtesip: ignores[unpinned-action]", // misspelt verb
    ] {
        let text = WITH.replace("# muhtesip: ignore[unpinned-action]", wrong);
        let workflow = kept(&text)
            .into_iter()
            .filter(|finding| finding.rule != "directive")
            .count();
        assert_eq!(workflow, 2, "must not suppress: {wrong}");
    }
}

#[test]
fn a_space_before_the_bracket_is_reported_rather_than_obeyed() {
    // The marker has to be exact, so this suppresses nothing — but it reads as a suppression, which is
    // why it is reported instead of silently doing nothing.
    let text = WITH.replace(
        "# muhtesip: ignore[unpinned-action]",
        "# muhtesip: ignore [unpinned-action]",
    );
    let findings = kept(&text);
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding.rule == "directive")
            .count(),
        1,
        "the unusable directive is reported: {findings:?}"
    );
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding.rule != "directive")
            .count(),
        2,
        "and it still suppresses nothing"
    );
}

#[test]
fn a_directive_cannot_reach_outside_the_block_it_leads() {
    // The directive is on the `build` job's key line: it covers that job and stops at `other`.
    let text = "\
name: ci

jobs:
  build: # muhtesip: ignore[unpinned-action]
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: example/toolbox@v4
  other:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: second/toolbox@v4
";
    let findings = kept(text);
    assert_eq!(findings.len(), 1, "one of the two jobs is covered");
    assert!(
        findings[0].message.contains("second/toolbox"),
        "the following job is not: {}",
        findings[0].message
    );
}

// --- A directive that cannot do anything is reported, not ignored (decision D5) ---

/// A document whose directive misspells the rule it means to suppress.
const TYPO: &str = "\
name: ci
on: [push]
jobs:
  build:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      # muhtesip: ignore[unpinned-actionn]
      - uses: example/toolbox@v4
";

/// The same document with a directive that names a rule this build has.
fn correct() -> String {
    TYPO.replace("unpinned-actionn", "unpinned-action")
}

/// The findings about the document's comments.
fn directive_findings(text: &str) -> Vec<Finding> {
    lint(text)
        .expect("the fixture parses")
        .into_iter()
        .filter(|finding| finding.rule == "directive")
        .collect()
}

#[test]
fn a_directive_naming_an_unknown_rule_is_reported() {
    let findings = directive_findings(TYPO);
    assert_eq!(
        findings.len(),
        1,
        "one finding about the comment: {findings:?}"
    );
    assert_eq!(findings[0].line, 8, "it points at the comment's own line");
    assert!(
        findings[0].message.contains("unpinned-actionn"),
        "the message names what was written: {}",
        findings[0].message
    );
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn a_misspelled_rule_name_still_suppresses_nothing() {
    // The reason to report it at all: the author believed a finding was silenced, so their red build
    // would otherwise look like a bug rather than a typo.
    let findings = lint(TYPO).expect("the fixture parses");
    assert!(
        findings
            .iter()
            .any(|finding| finding.rule == "unpinned-action"),
        "a typo must not silence anything: {findings:?}"
    );
}

#[test]
fn a_directive_naming_a_real_rule_is_silent() {
    assert!(
        directive_findings(&correct()).is_empty(),
        "nothing to say about a directive that works"
    );
}

#[test]
fn prose_is_not_a_directive_but_a_directive_without_an_argument_is() {
    let prose = TYPO.replace(
        "# muhtesip: ignore[unpinned-actionn]",
        "# muhtesip: ignore this for now",
    );
    assert!(
        directive_findings(&prose).is_empty(),
        "a sentence mentioning the marker is not an attempt at one"
    );

    let bare = TYPO.replace("# muhtesip: ignore[unpinned-actionn]", "# muhtesip: ignore");
    assert_eq!(
        directive_findings(&bare).len(),
        1,
        "a directive that lost its argument reads as a suppression and is reported"
    );
}

#[test]
fn the_same_unknown_name_twice_is_one_finding() {
    let text = TYPO.replace("[unpinned-actionn]", "[nope, nope]");
    assert_eq!(directive_findings(&text).len(), 1, "one mistake, not two");
}

#[test]
fn the_rule_itself_can_be_suppressed_by_name() {
    // It is a rule like any other, so naming it is the explicit way to accept the typo.
    let text = TYPO.replace(
        "# muhtesip: ignore[unpinned-actionn]",
        "# muhtesip: ignore[directive, unpinned-actionn]",
    );
    let directives = Directives::parse(&text);
    let kept: Vec<Finding> = lint(&text)
        .expect("the fixture parses")
        .into_iter()
        .filter(|finding| !directives.ignores(finding.line, finding.rule))
        .collect();

    assert!(
        kept.iter().all(|finding| finding.rule != "directive"),
        "named explicitly, it can be silenced: {kept:?}"
    );
    assert!(
        kept.iter().any(|finding| finding.rule == "unpinned-action"),
        "and silencing the report does not silence the typo: {kept:?}"
    );
}

#[test]
fn the_rule_can_be_disabled_like_any_other() {
    let settings = RuleSettings::new().with(
        "directive",
        RuleOverride {
            enabled: Some(false),
            ..Default::default()
        },
    );
    let registry = Registry::all().configure(settings);
    let findings = lint_str(TYPO, &registry).expect("the fixture parses");

    assert!(
        findings.iter().all(|finding| finding.rule != "directive"),
        "config reaches it like any rule: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.rule == "unpinned-action"),
        "disabling the comment check does not change what the workflow does"
    );
}
