//! The rule-metadata contract: every rule declares who it is, and findings name declared rules.

use muhtesip::{Registry, lint};

const BAD: &str = include_str!("fixtures/bad.yaml");

#[test]
fn every_shipped_rule_publishes_complete_metadata() {
    let metas = Registry::all().metas();
    assert_eq!(metas.len(), 16, "one meta per shipped rule");

    let repository = env!("CARGO_PKG_REPOSITORY");
    for meta in &metas {
        assert!(!meta.id.is_empty(), "a rule must have an id");
        assert!(
            !meta.description.is_empty(),
            "{}: needs a description",
            meta.id
        );
        assert!(
            !meta.description.ends_with('.'),
            "{}: description is one line, no trailing period",
            meta.id
        );

        let url = meta.docs_url();
        assert!(url.starts_with(repository), "{}: docs url {url}", meta.id);
        assert!(
            url.ends_with(&format!("#{}", meta.id)),
            "{}: docs url {url}",
            meta.id
        );
    }
}

#[test]
fn rule_ids_are_unique() {
    let metas = Registry::all().metas();
    let mut ids: Vec<&str> = metas.iter().map(|meta| meta.id).collect();
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), total, "rule ids must be unique");
}

#[test]
fn every_finding_names_a_declared_rule() {
    let declared: Vec<&str> = Registry::all().metas().iter().map(|meta| meta.id).collect();
    let findings = lint(BAD).expect("bad.yaml parses");
    assert!(!findings.is_empty(), "bad.yaml must produce findings");
    for finding in &findings {
        assert!(
            declared.contains(&finding.rule),
            "finding names an undeclared rule: {}",
            finding.rule
        );
    }
}

#[test]
fn meta_lookup_finds_a_rule_and_rejects_an_unknown_id() {
    let registry = Registry::all();
    assert_eq!(
        registry.meta("missing-timeout").map(|meta| meta.id),
        Some("missing-timeout")
    );
    assert!(registry.meta("no-such-rule").is_none());
}

#[test]
fn emitted_severity_matches_the_declared_default() {
    let registry = Registry::all();
    let findings = lint(BAD).expect("bad.yaml parses");
    for finding in &findings {
        let declared = registry.meta(finding.rule).expect("declared rule").severity;
        assert_eq!(
            finding.severity, declared,
            "{}: emitted severity",
            finding.rule
        );
    }
}
