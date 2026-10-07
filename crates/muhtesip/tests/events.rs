//! The events rule: unknown triggers, and filters an event does not accept.

use muhtesip::{Severity, lint};

/// A workflow that misuses four separate things.
const BAD: &str = include_str!("fixtures/events.yaml");

/// Lint a workflow string.
fn findings(source: &str) -> Vec<muhtesip::Finding> {
    lint(source).expect("the fixture parses")
}

/// The lines findings point at, in order.
fn lines(source: &str) -> Vec<usize> {
    findings(source).iter().map(|f| f.line).collect()
}

#[test]
fn reports_every_misuse_in_the_fixture() {
    let found = findings(BAD);
    assert_eq!(lines(BAD), vec![6, 8, 10, 11], "one finding per misuse");
    assert!(
        found
            .iter()
            .all(|f| f.rule == "events" && f.severity == Severity::Error),
        "every finding is an events error"
    );
    assert!(
        found[0]
            .message
            .contains("'branches' and 'branches-ignore' cannot be used"),
        "{}",
        found[0].message
    );
    assert!(
        found[1]
            .message
            .contains("'paths' is not available for the 'issues'"),
        "{}",
        found[1].message
    );
    assert!(
        found[2].message.contains("'bogus' is not valid"),
        "{}",
        found[2].message
    );
    assert!(
        found[3].message.contains("'schedulez' is unknown"),
        "{}",
        found[3].message
    );
}

#[test]
fn a_known_event_with_filters_it_accepts_is_clean() {
    let source = "\
on:
  push:
    branches: [main, 'releases/**']
    paths: [src/**]
  pull_request:
    branches-ignore: [legacy]
    types: [opened, synchronize]
  workflow_run:
    workflows: [build]
    branches: [main]
  schedule:
    - cron: '0 3 * * *'
  workflow_dispatch:
  repository_dispatch:
";
    assert_eq!(lines(source), Vec::<usize>::new());
}

#[test]
fn any_activity_type_is_valid_for_a_dispatch_event() {
    let source = "\
on:
  repository_dispatch:
    types: [anything-at-all, another]
";
    assert_eq!(lines(source), Vec::<usize>::new());
}

#[test]
fn types_is_refused_for_an_event_that_accepts_none() {
    let source = "\
on:
  push:
    types: [opened]
";
    let found = findings(source);
    assert_eq!(found.len(), 1);
    assert!(
        found[0]
            .message
            .contains("'types' cannot be specified for the 'push'"),
        "{}",
        found[0].message
    );
    assert_eq!(found[0].line, 2, "points at the event, not the filter");
}

#[test]
fn workflows_is_refused_outside_a_workflow_run_event() {
    let source = "\
on:
  push:
    workflows: [build]
";
    let found = findings(source);
    assert_eq!(found.len(), 1);
    assert!(
        found[0]
            .message
            .contains("only available for the 'workflow_run'"),
        "{}",
        found[0].message
    );
    assert_eq!(found[0].line, 3, "points at the filter, not the event");
}

#[test]
fn a_workflow_run_must_name_a_workflow() {
    let source = "\
on:
  workflow_run:
    types: [completed]
";
    let found = findings(source);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].message.contains("must name at least one workflow"),
        "{}",
        found[0].message
    );
    assert_eq!(found[0].line, 2);
}

#[test]
fn a_filter_pair_is_reported_once_not_twice() {
    // The pair could be found from either direction; only the negated side reports it.
    let source = "\
on:
  push:
    branches: [main]
    branches-ignore: [dev]
";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding for the pair");
    assert_eq!(found[0].line, 4, "points at the '-ignore' side");
}

#[test]
fn an_empty_filter_constrains_nothing() {
    // A declared-but-empty filter is not a misuse of the event.
    let source = "\
on:
  issues:
    paths: []
";
    assert_eq!(lines(source), Vec::<usize>::new());
}

#[test]
fn an_unknown_event_is_reported_once() {
    let source = "\
on:
  workflow_disptch:
    branches: [main]
";
    let found = findings(source);
    assert_eq!(found.len(), 1, "the event is reported, not its filters");
    assert!(
        found[0].message.contains("is unknown"),
        "{}",
        found[0].message
    );
}

// --- `schedule`: the expression, the platform's interval, and the timezone name ---

/// One job, so a schedule document is about its schedule and nothing else.
fn with_schedule(entries: &str) -> String {
    format!(
        "on:\n  schedule:\n{entries}jobs:\n  build:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n      - run: echo hi\n"
    )
}

/// The messages of this rule's findings for a document, in order.
fn messages(source: &str) -> Vec<String> {
    findings(source).iter().map(|f| f.message.clone()).collect()
}

#[test]
fn a_schedule_faster_than_the_platforms_floor_is_reported() {
    // Every minute, every four minutes, and the cross-hour case where a naive reading sees an hour.
    for cron in ["*/1 * * * *", "* * * * *", "*/4 * * * *", "59,0 * * * *"] {
        let text = with_schedule(&format!("    - cron: \"{cron}\"\n"));
        let found = findings(&text);
        assert_eq!(found.len(), 1, "{cron}: {found:?}");
        assert!(
            found[0].message.contains("at most once every 5 min"),
            "{cron}: {}",
            found[0].message
        );
        assert_eq!(found[0].line, 3, "the finding points at the expression");
    }
}

#[test]
fn the_documented_examples_are_silent() {
    // The docs' own schedule, and the docs' own timezone example.
    let text = with_schedule("    - cron: \"15 4,5 * * *\"\n      timezone: \"UTC\"\n");
    assert!(findings(&text).is_empty(), "{:?}", messages(&text));

    // `*/5` is exactly the floor, so it is allowed rather than rounded up.
    let text = with_schedule("    - cron: \"*/5 * * * *\"\n");
    assert!(findings(&text).is_empty(), "{:?}", messages(&text));
}

#[test]
fn a_malformed_expression_is_reported_with_its_reason() {
    for (cron, expected) in [
        ("75 * * * *", "75 is outside 0-59 for minute"),
        ("0 0 * *", "expected 5 fields, found 4"),
        ("@daily", "does not support the non-standard syntax"),
    ] {
        let text = with_schedule(&format!("    - cron: \"{cron}\"\n"));
        let found = findings(&text);
        assert_eq!(found.len(), 1, "{cron}: {found:?}");
        assert!(
            found[0].message.contains(expected),
            "{cron}: {}",
            found[0].message
        );
        assert!(
            found[0].message.contains(cron),
            "the message quotes it: {}",
            found[0].message
        );
    }
}

#[test]
fn an_unknown_timezone_is_reported_and_a_known_one_is_not() {
    let text = with_schedule("    - cron: \"0 3 * * *\"\n      timezone: \"Mars/Olympus\"\n");
    let found = findings(&text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].line, 4, "the finding points at the timezone value");

    // A legacy-but-valid link, and a lowercase name (the database is case-sensitive).
    let text = with_schedule("    - cron: \"0 3 * * *\"\n      timezone: \"Asia/Calcutta\"\n");
    assert!(findings(&text).is_empty(), "{:?}", messages(&text));
    let text = with_schedule("    - cron: \"0 3 * * *\"\n      timezone: \"asia/tokyo\"\n");
    assert_eq!(
        findings(&text).len(),
        1,
        "a case-insensitive name is not a name"
    );
}

#[test]
fn an_entry_with_no_cron_schedules_nothing_and_says_so() {
    let text = with_schedule("    - timezone: \"Asia/Tokyo\"\n");
    let found = findings(&text);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].message.contains("needs a `cron`"),
        "{}",
        found[0].message
    );

    // The schedule written as a string instead of a list of mappings loses the entry, so it is kept
    // as an entry with no values rather than dropped.
    let text = "on:\n  schedule: \"0 3 * * *\"\njobs:\n  build:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n      - run: echo hi\n";
    assert_eq!(findings(text).len(), 1, "{:?}", messages(text));
}

#[test]
fn a_schedule_with_no_entries_is_not_a_problem() {
    // An empty list declares no schedules, which is a workflow with no schedule rather than a defect.
    let text = "on:\n  schedule: []\njobs:\n  build:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n      - run: echo hi\n";
    assert!(findings(text).is_empty(), "{:?}", messages(text));
    // Control: a schedule that *is* declared and too fast is reported.
    assert_eq!(
        findings(&with_schedule("    - cron: \"* * * * *\"\n")).len(),
        1
    );
}

#[test]
fn a_broken_expression_does_not_hide_a_broken_timezone() {
    // Two independent defects in one entry: fixing the expression must not leave the author unaware of
    // the timezone, so neither check suppresses the other.
    let text = with_schedule("    - cron: \"0 25 * * *\"\n      timezone: \"Mars/Olympus\"\n");
    let found = findings(&text);
    assert_eq!(found.len(), 2, "{:?}", messages(&text));
    assert_eq!(found[0].line, 3, "the expression");
    assert_eq!(found[1].line, 4, "the timezone");
}

// --- declared inputs: the schema of the two events that take them ---

/// The documented example for a manual workflow, verbatim from the events reference.
const DISPATCH_EXAMPLE: &str = "\
on:
  workflow_dispatch:
    inputs:
      logLevel:
        description: 'Log level'
        required: true
        default: 'warning'
        type: choice
        options:
        - info
        - warning
        - debug
      tags:
        description: 'Test scenario tags'
        required: false
        type: boolean
      environment:
        description: 'Environment to run tests against'
        type: environment
        required: true
";

/// The documented example for a reusable workflow, verbatim from the syntax reference.
const CALL_EXAMPLE: &str = "\
on:
  workflow_call:
    inputs:
      username:
        description: 'A username passed from the caller workflow'
        default: 'john-doe'
        required: false
        type: string
";

/// A document with the given `on:` block and one boring job.
fn with_on(block: &str) -> String {
    format!(
        "{block}jobs:\n  build:\n    runs-on: ubuntu-latest\n    timeout-minutes: 5\n    steps:\n      - run: echo hi\n"
    )
}

#[test]
fn the_documented_input_examples_are_silent() {
    // The control that keeps the asymmetry honest: a manual input may be required AND have a default
    // (the browser pre-fills it), and these are the platform's own examples.
    for (name, block) in [("dispatch", DISPATCH_EXAMPLE), ("call", CALL_EXAMPLE)] {
        let text = with_on(block);
        assert!(findings(&text).is_empty(), "{name}: {:?}", messages(&text));
    }
}

#[test]
fn a_reusable_input_must_declare_a_type_and_a_dead_default_is_reported() {
    let text = with_on(
        "on:\n  workflow_call:\n    inputs:\n      needed:\n        required: true\n        default: \"hi\"\n      untyped:\n        description: y\n",
    );
    let found = findings(&text);
    assert_eq!(found.len(), 3, "{:?}", messages(&text));
    let lines: Vec<usize> = found.iter().map(|f| f.line).collect();
    assert_eq!(
        lines,
        vec![4, 6, 7],
        "the name line, the default line, and the second name"
    );
    assert!(
        found[0].message.contains("must declare a `type`"),
        "{}",
        found[0].message
    );
    assert!(
        found[1].message.contains("is never used"),
        "{}",
        found[1].message
    );
}

#[test]
fn a_required_default_is_fine_for_a_manual_input() {
    // The asymmetry, pinned on its own so it cannot be "tidied" into the reusable rule.
    let text = with_on(
        "on:\n  workflow_dispatch:\n    inputs:\n      level:\n        required: true\n        default: 'warning'\n",
    );
    assert!(findings(&text).is_empty(), "{:?}", messages(&text));
}

#[test]
fn an_unknown_type_names_the_types_that_event_takes() {
    let text = with_on("on:\n  workflow_dispatch:\n    inputs:\n      x:\n        type: choise\n");
    let found = findings(&text);
    assert_eq!(found.len(), 1, "{:?}", messages(&text));
    assert!(
        found[0]
            .message
            .contains("boolean, choice, number, environment, string"),
        "the message lists them: {}",
        found[0].message
    );

    // `choice` belongs to the manual event only, so the reusable one must not accept it.
    let text = with_on("on:\n  workflow_call:\n    inputs:\n      x:\n        type: choice\n");
    let found = findings(&text);
    assert_eq!(found.len(), 1, "{:?}", messages(&text));
    assert!(
        found[0].message.contains("boolean, number, string"),
        "{}",
        found[0].message
    );
}

#[test]
fn choice_and_options_have_to_agree() {
    for (block, expected) in [
        (
            "on:\n  workflow_dispatch:\n    inputs:\n      x:\n        type: choice\n",
            "nothing to choose",
        ),
        (
            "on:\n  workflow_dispatch:\n    inputs:\n      x:\n        type: string\n        options: [a, b]\n",
            "only apply to a `choice` input",
        ),
        (
            "on:\n  workflow_dispatch:\n    inputs:\n      x:\n        type: choice\n        options: [a, a]\n",
            "appears twice",
        ),
        (
            "on:\n  workflow_dispatch:\n    inputs:\n      x:\n        type: choice\n        default: c\n        options: [a, b]\n",
            "is not one of its options",
        ),
    ] {
        let text = with_on(block);
        let found = findings(&text);
        assert_eq!(found.len(), 1, "{block:?}: {:?}", messages(&text));
        assert!(
            found[0].message.contains(expected),
            "{block:?}: {}",
            found[0].message
        );
    }
}

#[test]
fn a_default_is_checked_against_the_type_it_declares() {
    for (block, expected) in [
        (
            "on:\n  workflow_dispatch:\n    inputs:\n      x:\n        type: number\n        default: \"abc\"\n",
            "cannot be read as one",
        ),
        (
            "on:\n  workflow_dispatch:\n    inputs:\n      x:\n        type: boolean\n        default: \"yes\"\n",
            "must be `true` or `false`",
        ),
    ] {
        let text = with_on(block);
        let found = findings(&text);
        assert_eq!(found.len(), 1, "{block:?}: {:?}", messages(&text));
        assert!(
            found[0].message.contains(expected),
            "{block:?}: {}",
            found[0].message
        );
    }

    // A default built from an expression is resolved by the platform, so it has no literal to judge.
    let text = with_on(
        "on:\n  workflow_dispatch:\n    inputs:\n      x:\n        type: number\n        default: ${{ vars.LIMIT }}\n",
    );
    assert!(findings(&text).is_empty(), "{:?}", messages(&text));
}

#[test]
fn a_manual_block_is_capped_at_twenty_five_inputs() {
    let mut block = String::from("on:\n  workflow_dispatch:\n    inputs:\n");
    for index in 0..26 {
        block.push_str(&format!("      in{index}:\n        description: d\n"));
    }
    let text = with_on(&block);
    let found = findings(&text);
    assert_eq!(found.len(), 1, "one finding, not one per later input");
    assert!(
        found[0].message.contains("at most 25 inputs"),
        "{}",
        found[0].message
    );
    // The twenty-sixth input is the one that crosses the cap.
    assert_eq!(found[0].line, 4 + 25 * 2, "the input that crosses the cap");
}
