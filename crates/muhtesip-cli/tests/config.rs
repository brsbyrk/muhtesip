//! The configuration document: parsing, validation, and its effect on findings.

// The fixtures live with the library's tests — they are the documents the rules are tested against,
// and a second copy would be a different document that drifts. This crate reaches across for them.
use std::process::Command;

use muhtesip::{Config, Registry, Severity, lint_str};

const BAD: &str = include_str!("../../muhtesip/tests/fixtures/bad.yaml");
const CONFIG: &str = include_str!("../../muhtesip/tests/fixtures/muhtesip.yaml");
const IGNORES: &str = include_str!("../../muhtesip/tests/fixtures/ignores.yaml");

#[test]
fn a_config_document_escalates_one_rule_and_disables_another() {
    let config = Config::parse(CONFIG).expect("fixture config parses");
    let registry = Registry::all().configure(config.settings().clone());
    let findings = lint_str(BAD, &registry).expect("bad.yaml parses");

    let summary: Vec<(&str, Severity)> = findings.iter().map(|f| (f.rule, f.severity)).collect();
    assert_eq!(
        summary,
        vec![("unpinned-action", Severity::Error)],
        "unpinned-action escalated, missing-timeout disabled"
    );
}

#[test]
fn an_unknown_rule_id_is_rejected_by_validation() {
    let config = Config::parse("rules:\n  no-such-rule:\n    severity: error\n").expect("syntax");
    let error = Registry::all()
        .validate(config.settings())
        .expect_err("an unknown id must be reported");
    assert!(format!("{error}").contains("no-such-rule"), "{error}");
}

#[test]
fn a_misspelt_severity_is_rejected_with_the_offending_name() {
    let error = Config::parse("rules:\n  missing-timeout:\n    severity: loud\n")
        .expect_err("a bad severity must be reported");
    assert!(format!("{error}").contains("loud"), "{error}");
}

#[test]
fn an_unknown_setting_is_rejected() {
    assert!(Config::parse("rules:\n  missing-timeout:\n    colour: red\n").is_err());
}

#[test]
fn an_unknown_top_level_key_is_rejected() {
    assert!(Config::parse("pathz: {}\n").is_err());
}

#[test]
fn a_rules_value_that_is_not_a_mapping_is_rejected() {
    assert!(Config::parse("rules: 3\n").is_err());
}

#[test]
fn an_empty_document_is_the_default_configuration() {
    let config = Config::parse("").expect("empty document parses");
    assert!(config.settings().is_empty());
}

#[test]
fn the_binary_applies_a_config_file() {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .args([
            "--config",
            &format!("{root}/../muhtesip/tests/fixtures/muhtesip.yaml"),
            &format!("{root}/../muhtesip/tests/fixtures/bad.yaml"),
        ])
        .output()
        .expect("binary runs");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "findings, not an error");
    assert!(
        stdout.contains("error:"),
        "severity was escalated: {stdout}"
    );
    assert!(
        !stdout.contains("missing-timeout"),
        "rule was disabled: {stdout}"
    );
}

#[test]
fn the_binary_rejects_a_config_that_names_an_unknown_rule() {
    let root = env!("CARGO_MANIFEST_DIR");
    let bogus = std::env::temp_dir().join("muhtesip-bogus-config.yaml");
    std::fs::write(&bogus, "rules:\n  no-such-rule:\n    enabled: false\n").expect("write");

    let output = Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .args([
            "--config",
            bogus.to_str().expect("path"),
            &format!("{root}/../muhtesip/tests/fixtures/bad.yaml"),
        ])
        .output()
        .expect("binary runs");

    assert_eq!(
        output.status.code(),
        Some(2),
        "a bad config is an error, not findings"
    );
}

#[test]
fn a_config_is_discovered_by_walking_up_from_the_working_directory() {
    let root = std::env::temp_dir().join("muhtesip-discovery");
    let nested = root.join("sub/dir");
    std::fs::create_dir_all(&nested).expect("make dirs");
    std::fs::write(
        root.join(".muhtesip.yaml"),
        "rules:\n  missing-timeout:\n    enabled: false\n",
    )
    .expect("write discovered config");
    std::fs::write(nested.join("ci.yaml"), BAD).expect("write workflow");

    let output = Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .current_dir(&nested)
        .arg("ci.yaml")
        .output()
        .expect("binary runs");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("unpinned-action"), "{stdout}");
    assert!(
        !stdout.contains("missing-timeout"),
        "the discovered config was applied: {stdout}"
    );
}

#[test]
fn an_explicit_config_wins_over_a_discovered_one() {
    let root = std::env::temp_dir().join("muhtesip-discovery-override");
    std::fs::create_dir_all(&root).expect("make dir");
    std::fs::write(
        root.join(".muhtesip.yaml"),
        "rules:\n  unpinned-action:\n    enabled: false\n",
    )
    .expect("write discovered config");
    let explicit = root.join("explicit.yaml");
    std::fs::write(
        &explicit,
        "rules:\n  missing-timeout:\n    enabled: false\n",
    )
    .expect("write explicit config");
    std::fs::write(root.join("ci.yaml"), BAD).expect("write workflow");

    let output = Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .current_dir(&root)
        .args(["--config", explicit.to_str().expect("path"), "ci.yaml"])
        .output()
        .expect("binary runs");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unpinned-action"),
        "--config must win, so unpinned-action is still reported: {stdout}"
    );
    assert!(!stdout.contains("missing-timeout"), "{stdout}");
}

#[test]
fn a_path_scoped_ignore_suppresses_only_matching_paths() {
    let config = Config::parse(IGNORES).expect("fixture parses");
    let findings = lint_str(BAD, &Registry::all()).expect("bad.yaml parses");

    let suppressed: Vec<&str> = findings
        .iter()
        .filter(|finding| config.is_ignored("tests/fixtures/bad.yaml", finding))
        .map(|finding| finding.rule)
        .collect();
    assert_eq!(suppressed, vec!["unpinned-action"]);

    assert!(
        findings
            .iter()
            .all(|finding| !config.is_ignored("somewhere/else.yaml", finding)),
        "a path the glob does not match suppresses nothing"
    );
}

#[test]
fn the_binary_applies_path_scoped_ignores() {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .args([
            "--config",
            &format!("{root}/../muhtesip/tests/fixtures/ignores.yaml"),
            &format!("{root}/../muhtesip/tests/fixtures/bad.yaml"),
        ])
        .output()
        .expect("binary runs");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("unpinned-action"), "ignored: {stdout}");
    assert!(stdout.contains("missing-timeout"), "not ignored: {stdout}");

    // The only finding left is a note, which is below the default threshold, so this run is clean by
    // the default rule. Asking for notes to fail gives the same run exit 1 — pinning both that the
    // ignore worked and that the exit code follows the THRESHOLD, not the mere presence of findings.
    assert_eq!(
        output.status.code(),
        Some(0),
        "only a note remains, and the default threshold is warning"
    );

    let strict = Command::new(env!("CARGO_BIN_EXE_muhtesip"))
        .args([
            "--fail-on",
            "note",
            "--config",
            &format!("{root}/../muhtesip/tests/fixtures/ignores.yaml"),
            &format!("{root}/../muhtesip/tests/fixtures/bad.yaml"),
        ])
        .output()
        .expect("binary runs");
    assert_eq!(
        strict.status.code(),
        Some(1),
        "asked to fail on notes, the same run does"
    );
}

#[test]
fn an_invalid_regex_is_rejected() {
    let error = Config::parse("paths:\n  \"**/*.yaml\":\n    ignore:\n      - \"(\"\n")
        .expect_err("an invalid regex must be reported");
    assert!(format!("{error}").contains("invalid regex"), "{error}");
}

#[test]
fn an_invalid_glob_is_rejected() {
    let error = Config::parse("paths:\n  \"[\":\n    ignore:\n      - \"x\"\n")
        .expect_err("an invalid glob must be reported");
    assert!(format!("{error}").contains("invalid glob"), "{error}");
}

#[test]
fn an_ignore_that_is_not_a_list_is_rejected() {
    assert!(Config::parse("paths:\n  \"**/*.yaml\":\n    ignore: nope\n").is_err());
}

#[test]
fn an_unknown_setting_under_a_path_is_rejected() {
    assert!(Config::parse("paths:\n  \"**/*.yaml\":\n    ignores: []\n").is_err());
}

// --- the starter configuration that `--init-config` writes ---

#[test]
fn the_template_parses_back_and_describes_every_rule() {
    let registry = Registry::all();
    let document = Config::template(&registry);

    // A template the tool's own strict reader rejects would be worse than no template at all.
    let parsed = Config::parse(&document).expect("the template the tool writes is readable");
    registry
        .validate(parsed.settings())
        .expect("every rule the template names exists and every severity is spellable");

    let metas = registry.metas();
    assert!(!metas.is_empty(), "the check must not be vacuous");
    for meta in &metas {
        assert!(
            document.contains(&format!("  {}:", meta.id)),
            "the template omits {}",
            meta.id
        );
    }
    assert_eq!(
        document.matches("    severity: ").count(),
        metas.len(),
        "one severity per rule, and no extras"
    );
}

#[test]
fn the_template_is_byte_stable() {
    let registry = Registry::all();
    assert_eq!(
        Config::template(&registry),
        Config::template(&registry),
        "a template that reorders itself would make every diff noisy"
    );
}
