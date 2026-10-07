//! The credentials rule: a container password written literally instead of from a secret.

use muhtesip::{Severity, lint};

/// A job whose own container has a literal password, and whose service uses a secret.
const BAD: &str = include_str!("fixtures/credentials.yaml");

/// Run the linter and keep only this rule's findings, as `(line, message)`.
fn hits(text: &str) -> Vec<(usize, String)> {
    lint(text)
        .expect("document parses")
        .into_iter()
        .filter(|finding| finding.rule == "credentials")
        .map(|finding| (finding.line, finding.message))
        .collect()
}

/// A job with one container, whose password is whatever `password` says.
fn job_with_container(password: &str) -> String {
    format!(
        "jobs:\n  a:\n    container:\n      image: postgres:16\n      credentials:\n        username: admin\n        password: {password}\n    steps: []\n"
    )
}

#[test]
fn the_fixture_reports_the_container_password_and_not_the_secret_one() {
    let found = hits(BAD);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, 13, "the password's own line");
    assert!(
        found[0].1.contains("container's password"),
        "{:?}",
        found[0]
    );
}

#[test]
fn a_service_password_written_literally_is_reported_by_service_name() {
    let text = "jobs:\n  a:\n    services:\n      db:\n        image: postgres:16\n        credentials:\n          username: admin\n          password: hunter2\n    steps: []\n";
    let found = hits(text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("'db' service"), "{:?}", found[0]);
}

#[test]
fn a_password_from_a_secret_is_not_reported() {
    for password in [
        "${{ secrets.DB_PASSWORD }}",
        "${{ env.PASSWORD }}",
        "${{ inputs.db_password }}",
    ] {
        let found = hits(&job_with_container(password));
        assert!(found.is_empty(), "{password}: {found:?}");
    }
}

#[test]
fn a_job_without_containers_is_not_this_rules_business() {
    let text = "jobs:\n  a:\n    steps:\n      - run: echo hi\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
    // Control: the rule is live on a job that does have a literal password, so the silence above
    // is the rule's judgement and not a disabled rule.
    assert_eq!(hits(&job_with_container("hunter2")).len(), 1);
}

#[test]
fn a_container_without_credentials_is_not_reported() {
    let text = "jobs:\n  a:\n    container:\n      image: postgres:16\n    steps: []\n";
    assert!(hits(text).is_empty(), "{:?}", hits(text));
    // Control: the same container *with* a literal password is reported.
    assert_eq!(hits(&job_with_container("hunter2")).len(), 1);
}

#[test]
fn the_declared_severity_is_error() {
    // The password is committed to the repository in plain text.
    let finding = lint(BAD)
        .expect("parses")
        .into_iter()
        .find(|finding| finding.rule == "credentials")
        .expect("one finding");
    assert_eq!(finding.severity, Severity::Error);
}
