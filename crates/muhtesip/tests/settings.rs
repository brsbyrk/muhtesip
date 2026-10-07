//! Per-rule adjustments: escalating, downgrading, and disabling, applied as data.

use muhtesip::{Registry, RuleOverride, RuleSettings, Severity, lint, lint_str};

const BAD: &str = include_str!("fixtures/bad.yaml");

fn with_severity(id: &str, severity: Severity) -> RuleSettings {
    RuleSettings::new().with(
        id,
        RuleOverride {
            severity: Some(severity),
            ..Default::default()
        },
    )
}

fn disable(id: &str) -> RuleSettings {
    RuleSettings::new().with(
        id,
        RuleOverride {
            enabled: Some(false),
            ..Default::default()
        },
    )
}

#[test]
fn escalating_one_rule_leaves_the_other_at_its_default() {
    let registry = Registry::all().configure(with_severity("unpinned-action", Severity::Error));
    let findings = lint_str(BAD, &registry).expect("bad.yaml parses");

    let summary: Vec<(&str, Severity)> = findings.iter().map(|f| (f.rule, f.severity)).collect();
    assert_eq!(
        summary,
        vec![
            ("missing-timeout", Severity::Note),
            ("unpinned-action", Severity::Error)
        ],
        "only unpinned-action's severity may change"
    );
}

#[test]
fn escalating_a_note_rule_leaves_the_other_at_its_default() {
    // `missing-timeout`'s declared default is `Note`; raising it to `Error` is an escalation.
    let registry = Registry::all().configure(with_severity("missing-timeout", Severity::Error));
    let findings = lint_str(BAD, &registry).expect("bad.yaml parses");

    let summary: Vec<(&str, Severity)> = findings.iter().map(|f| (f.rule, f.severity)).collect();
    assert_eq!(
        summary,
        vec![
            ("missing-timeout", Severity::Error),
            ("unpinned-action", Severity::Warning)
        ],
    );
}

#[test]
fn downgrading_a_warning_rule_leaves_the_other_at_its_default() {
    // The other direction: `unpinned-action`'s default is `Warning`, lowered here to `Note`.
    // A rename alone would have left downgrading untested, which is how the mismatch hid.
    let registry = Registry::all().configure(with_severity("unpinned-action", Severity::Note));
    let findings = lint_str(BAD, &registry).expect("bad.yaml parses");

    let summary: Vec<(&str, Severity)> = findings.iter().map(|f| (f.rule, f.severity)).collect();
    assert_eq!(
        summary,
        vec![
            ("missing-timeout", Severity::Note),
            ("unpinned-action", Severity::Note)
        ],
        "only unpinned-action's severity may change"
    );
}

#[test]
fn disabling_one_rule_removes_only_its_findings() {
    let registry = Registry::all().configure(disable("unpinned-action"));
    let findings = lint_str(BAD, &registry).expect("bad.yaml parses");

    let rules: Vec<&str> = findings.iter().map(|f| f.rule).collect();
    assert_eq!(rules, vec!["missing-timeout"], "the other rule still runs");
}

#[test]
fn empty_settings_change_nothing() {
    let configured = Registry::all().configure(RuleSettings::new());
    assert_eq!(
        lint_str(BAD, &configured).expect("parses"),
        lint(BAD).expect("parses")
    );
}

#[test]
fn explicitly_enabling_a_rule_is_a_no_op() {
    let registry = Registry::all().configure(RuleSettings::new().with(
        "unpinned-action",
        RuleOverride {
            enabled: Some(true),
            ..Default::default()
        },
    ));
    assert_eq!(
        lint_str(BAD, &registry).expect("parses"),
        lint(BAD).expect("parses")
    );
}

#[test]
fn an_unknown_rule_id_is_inert() {
    let registry = Registry::all().configure(with_severity("no-such-rule", Severity::Error));
    assert_eq!(
        lint_str(BAD, &registry).expect("parses"),
        lint(BAD).expect("parses")
    );
    assert!(!registry.settings().is_empty());
}
