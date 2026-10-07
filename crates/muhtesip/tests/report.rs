//! The report's encodings: one derivation, and the proofs that they agree.
//!
//! The envelope and the SARIF document are written by hand in `src/report.rs`, so these tests parse
//! them with a real JSON parser (`serde_json`, a dev-dependency) instead of comparing them to a
//! string the same code produced. The central assertion reassembles the plain output from the
//! envelope's own `text` fields — that is what makes the two encodings provably the same answer
//! rather than two guesses.

use muhtesip::report::{Envelope, Refusal, SourceReport};
use muhtesip::{Finding, Registry, Severity, lint};
use serde_json::Value;

/// A workflow that trips two rules.
const BAD: &str = include_str!("fixtures/bad.yaml");

/// Every `findings[].text` of the first source, in order.
fn texts(value: &Value) -> Vec<String> {
    value["sources"][0]["findings"]
        .as_array()
        .expect("findings is an array")
        .iter()
        .map(|entry| entry["text"].as_str().expect("text is a string").to_owned())
        .collect()
}

/// Parse an envelope, insisting it is well formed.
fn parse(json: &str) -> Value {
    serde_json::from_str(json).expect("the envelope is valid JSON")
}

#[test]
fn the_envelope_reassembles_the_plain_output() {
    let findings = lint(BAD).expect("bad.yaml parses");
    let kept: Vec<&Finding> = findings.iter().collect();
    let envelope = Envelope::new(vec![SourceReport::linted("bad.yaml", kept)]);

    let text = envelope.text();
    let assembled = texts(&parse(&envelope.json()));

    assert!(
        !assembled.is_empty(),
        "the reassembly proof must not be vacuous: {text:?}"
    );
    assert_eq!(
        assembled.join("\n") + "\n",
        text,
        "the envelope's own fields must reassemble the text output byte for byte"
    );
}

#[test]
fn every_envelope_field_agrees_with_its_text_line() {
    let findings = lint(BAD).expect("bad.yaml parses");
    let kept: Vec<&Finding> = findings.iter().collect();
    let expected = kept.len();
    let envelope = Envelope::new(vec![SourceReport::linted("bad.yaml", kept)]);
    let value = parse(&envelope.json());

    let entries = value["sources"][0]["findings"]
        .as_array()
        .expect("findings is an array");
    assert_eq!(entries.len(), expected, "one entry per kept finding");

    for entry in entries {
        let line = entry["text"].as_str().expect("text is a string");
        let line_number = entry["line"]
            .as_u64()
            .expect("line is a number")
            .to_string();
        for field in ["file", "rule", "severity", "message"] {
            let expected = entry[field].as_str().expect("field is a string");
            assert!(
                line.contains(expected),
                "the text line must contain its own {field} {expected:?}: {line:?}"
            );
        }
        assert!(
            line.contains(&line_number),
            "the text line must contain its own line number {line_number}: {line:?}"
        );
    }
}

#[test]
fn a_refused_source_carries_a_named_refusal_and_no_findings() {
    let envelope = Envelope::new(vec![SourceReport::refused(
        "broken.yaml",
        Refusal {
            name: "PARSE_ERROR",
            message: "could not parse workflow: bad".to_owned(),
        },
    )]);

    assert!(envelope.has_refusal());
    assert!(!envelope.has_findings());
    assert_eq!(
        envelope.text(),
        "",
        "a refusal has never printed anything on stdout"
    );

    let value = parse(&envelope.json());
    assert_eq!(value["sources"][0]["refusal"]["name"], "PARSE_ERROR");
    assert_eq!(
        value["sources"][0]["findings"]
            .as_array()
            .expect("findings is present and empty")
            .len(),
        0
    );
    assert!(
        !value["sources"][0]["refusal"]["means"]
            .as_str()
            .expect("means is a sentence")
            .is_empty(),
        "a refusal says no document was produced, rather than leaving it to be inferred"
    );
}

#[test]
fn a_run_level_refusal_names_itself_and_has_no_sources() {
    let envelope = Envelope::refused("CONFIG_ERROR", "invalid configuration: nope".to_owned());

    assert!(envelope.has_refusal());
    let value = parse(&envelope.json());
    assert_eq!(value["envelope"], "muhtesip/1");
    assert_eq!(value["refusal"]["name"], "CONFIG_ERROR");
    assert_eq!(
        value["sources"]
            .as_array()
            .expect("sources is an array")
            .len(),
        0
    );
}

#[test]
fn an_empty_result_is_a_valid_empty_envelope_not_a_refusal() {
    let envelope = Envelope::new(vec![SourceReport::linted("clean.yaml", Vec::new())]);

    assert!(!envelope.has_refusal());
    assert!(!envelope.has_findings());
    assert_eq!(envelope.text(), "");

    let value = parse(&envelope.json());
    assert!(
        value["refusal"].is_null(),
        "nothing to report is not a refusal, and must not look like one"
    );
    assert_eq!(
        value["sources"][0]["findings"]
            .as_array()
            .expect("findings is present and empty")
            .len(),
        0
    );
}

#[test]
fn a_message_that_would_break_the_document_round_trips() {
    let nasty = Finding {
        rule: "test",
        severity: Severity::Error,
        message: "a \"quote\", a \\ backslash,\n a newline, a\ttab and a \u{1} control".to_owned(),
        line: 7,
    };
    let envelope = Envelope::new(vec![SourceReport::linted("nasty.yaml", vec![&nasty])]);

    let value = parse(&envelope.json());
    assert_eq!(
        value["sources"][0]["findings"][0]["message"]
            .as_str()
            .expect("message is a string"),
        nasty.message,
        "the message survives the encoding unchanged"
    );
}

#[test]
fn every_source_keeps_its_own_name_and_order() {
    let first = Finding {
        rule: "a",
        severity: Severity::Note,
        message: "first".to_owned(),
        line: 1,
    };
    let second = Finding {
        rule: "b",
        severity: Severity::Warning,
        message: "second".to_owned(),
        line: 2,
    };
    let envelope = Envelope::new(vec![
        SourceReport::linted("one.yaml", vec![&first]),
        SourceReport::refused(
            "two.yaml",
            Refusal {
                name: "READ_ERROR",
                message: "gone".to_owned(),
            },
        ),
        SourceReport::linted("three.yaml", vec![&second]),
    ]);

    assert!(envelope.has_findings());
    assert!(envelope.has_refusal());
    assert_eq!(
        envelope.text(),
        "one.yaml:1: note: first [a]\nthree.yaml:2: warning: second [b]\n"
    );

    let value = parse(&envelope.json());
    let sources = value["sources"].as_array().expect("sources is an array");
    assert_eq!(sources.len(), 3);
    assert_eq!(sources[0]["source"], "one.yaml");
    assert_eq!(sources[1]["source"], "two.yaml");
    assert_eq!(sources[2]["source"], "three.yaml");
    assert_eq!(sources[1]["refusal"]["name"], "READ_ERROR");
}

// --- the annotation encoding: a third reading of the same envelope ---

#[test]
fn the_annotation_encoding_reads_the_same_envelope() {
    let findings = lint(BAD).expect("bad.yaml parses");
    let kept: Vec<&Finding> = findings.iter().collect();
    let expected = kept.len();
    let envelope = Envelope::new(vec![SourceReport::linted("bad.yaml", kept)]);

    let annotations = envelope.annotations();
    let lines: Vec<&str> = annotations.lines().collect();
    assert_eq!(lines.len(), expected, "one annotation per finding");
    assert_eq!(
        lines.len(),
        envelope.text().lines().count(),
        "the same findings, counted once"
    );
    for line in &lines {
        assert!(line.starts_with("::"), "a log command: {line}");
        assert!(line.contains("file=bad.yaml"), "{line}");
        assert!(line.contains(",line="), "{line}");
        assert!(line.contains(",title="), "{line}");
    }
    // The levels come from the same severities the plain text printed.
    assert!(annotations.contains("::notice "), "{annotations}");
    assert!(annotations.contains("::warning "), "{annotations}");
}

#[test]
fn a_refused_source_annotates_with_its_refusal_name() {
    let envelope = Envelope::new(vec![SourceReport::refused(
        "broken.yaml",
        Refusal {
            name: "PARSE_ERROR",
            message: "could not parse workflow: bad".to_owned(),
        },
    )]);
    let annotations = envelope.annotations();
    assert!(annotations.contains("title=PARSE_ERROR"), "{annotations}");
    assert!(annotations.contains("file=broken.yaml"), "{annotations}");
    assert!(!annotations.contains(",line="), "a refusal has no line");
}

#[test]
fn an_empty_result_annotates_nothing() {
    let envelope = Envelope::new(vec![SourceReport::linted("clean.yaml", Vec::new())]);
    assert_eq!(envelope.annotations(), "");
}

#[test]
fn a_run_level_refusal_annotates_without_a_file() {
    let envelope = Envelope::refused("CONFIG_ERROR", "invalid configuration: nope".to_owned());
    assert_eq!(
        envelope.annotations(),
        "::error title=CONFIG_ERROR::invalid configuration: nope\n"
    );
}

// --- the SARIF encoding: structurally asserted here, schema-validated by hand ---
//
// `jsonschema` against the official 2.1.0 schema is the strong check, but it needs the 112 KB schema
// file and a network fetch, so it is deliberately NOT in the suite. These tests assert the
// invariants that validation would; docs/ROADMAP.md records the manual run.

/// A SARIF document for `text`, and the parsed value.
fn sarif_of(source: &str, text: &str) -> (String, serde_json::Value) {
    let findings = lint(text).expect("the fixture parses");
    let kept: Vec<&Finding> = findings.iter().collect();
    let envelope = Envelope::new(vec![SourceReport::linted(source, kept)]);
    let drawn = envelope.sarif(&Registry::all().metas());
    let value: serde_json::Value =
        serde_json::from_str(&drawn).expect("the SARIF document is valid JSON");
    (drawn, value)
}

#[test]
fn sarif_declares_the_version_and_one_run() {
    let (_, value) = sarif_of("bad.yaml", BAD);
    assert_eq!(value["version"].as_str(), Some("2.1.0"));
    assert_eq!(value["runs"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        value["runs"][0]["tool"]["driver"]["name"].as_str(),
        Some("muhtesip")
    );
}

#[test]
fn every_result_points_at_a_rule_the_document_declares() {
    let (_, value) = sarif_of("bad.yaml", BAD);
    let rules = value["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules is an array");
    let results = value["runs"][0]["results"]
        .as_array()
        .expect("results is an array");
    assert!(!results.is_empty(), "the check must not be vacuous");

    for result in results {
        let index = result["ruleIndex"].as_u64().expect("ruleIndex") as usize;
        assert_eq!(
            rules[index]["id"].as_str(),
            result["ruleId"].as_str(),
            "ruleIndex must point at the rule the result names"
        );
    }
}

#[test]
fn every_result_carries_a_level_a_message_and_a_location() {
    let (_, value) = sarif_of("bad.yaml", BAD);
    for result in value["runs"][0]["results"].as_array().expect("results") {
        assert!(result["level"].is_string(), "{result}");
        assert!(result["message"]["text"].is_string(), "{result}");
        let location = &result["locations"][0]["physicalLocation"];
        assert_eq!(
            location["artifactLocation"]["uri"].as_str(),
            Some("bad.yaml")
        );
        assert!(
            location["region"]["startLine"].as_u64().is_some(),
            "{result}"
        );
    }
}

#[test]
fn a_cited_rule_carries_its_description_its_docs_and_its_default_level() {
    let (_, value) = sarif_of("bad.yaml", BAD);
    let rules = value["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("rules");
    let unpinned = rules
        .iter()
        .find(|rule| rule["id"] == "unpinned-action")
        .expect("the rule the results cite is described");

    let description = unpinned["shortDescription"]["text"]
        .as_str()
        .expect("shortDescription.text");
    assert!(description.contains("immutable commit"), "{description}");
    assert_eq!(
        unpinned["defaultConfiguration"]["level"].as_str(),
        Some("warning")
    );
    assert!(
        unpinned["helpUri"]
            .as_str()
            .expect("helpUri")
            .ends_with("#unpinned-action"),
        "the docs link is derived from the rule id"
    );
}

#[test]
fn a_refusal_is_an_unsuccessful_invocation_not_a_result() {
    let envelope = Envelope::new(vec![SourceReport::refused(
        "broken.yaml",
        Refusal {
            name: "PARSE_ERROR",
            message: "could not parse workflow: bad".to_owned(),
        },
    )]);
    let drawn = envelope.sarif(&Registry::all().metas());
    let value: serde_json::Value = serde_json::from_str(&drawn).expect("valid JSON");
    let invocation = &value["runs"][0]["invocations"][0];

    assert_eq!(
        invocation["executionSuccessful"].as_bool(),
        Some(false),
        "a refusal is not success"
    );
    assert_eq!(
        value["runs"][0]["results"].as_array().map(Vec::len),
        Some(0),
        "and it is not a result either"
    );
    let notification = &invocation["toolExecutionNotifications"][0];
    assert_eq!(
        notification["descriptor"]["id"].as_str(),
        Some("PARSE_ERROR")
    );
    assert_eq!(notification["level"].as_str(), Some("error"));
}

#[test]
fn a_clean_run_is_successful_and_describes_no_rules() {
    let envelope = Envelope::new(vec![SourceReport::linted("clean.yaml", Vec::new())]);
    let drawn = envelope.sarif(&Registry::all().metas());
    let value: serde_json::Value = serde_json::from_str(&drawn).expect("valid JSON");

    assert_eq!(
        value["runs"][0]["invocations"][0]["executionSuccessful"].as_bool(),
        Some(true)
    );
    assert_eq!(
        value["runs"][0]["results"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        value["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
}

#[test]
fn the_sarif_document_is_byte_stable_for_the_same_input() {
    let (first, _) = sarif_of("bad.yaml", BAD);
    let (second, _) = sarif_of("bad.yaml", BAD);
    assert_eq!(
        first, second,
        "the rule order must not depend on a hash map"
    );
}
