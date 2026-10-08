//! The inputs the two events declare, judged against the platform's schema.
//!
//! Data drives it: the type lists and the input cap live in `src/data/`. The judgement is a pure
//! function of the declarations, so it is testable without a registry, a file, or a finding.
//!
//! Two sources, and where one is silent the other decides: GitHub's docs say which types each event
//! accepts and that a `workflow_call` input must declare one, and actionlint's `rule_events.go` gives
//! the consistency checks — a default that contradicts its type, `choice` with no `options`, options
//! on anything else, a duplicated option, and a default that is not one of them.
//!
//! One asymmetry is deliberate and is the reason the required-plus-default check names one event only:
//! a **reusable workflow's** required input must be passed by the caller, so its default can never
//! apply; a **manual dispatch** pre-fills the default in the browser, so the same pair is fine there.

use crate::data::{
    WORKFLOW_CALL_INPUT_TYPES, WORKFLOW_DISPATCH_INPUT_CAP, WORKFLOW_DISPATCH_INPUT_TYPES,
};
use crate::model::InputDecl;
use crate::rules::EXPRESSION_OPEN;

/// Whether a value is built from an expression, which the platform resolves for a default and so has
/// no literal value to judge.
fn is_expression(value: &str) -> bool {
    value.contains(EXPRESSION_OPEN)
}

/// Every problem with the declared inputs, as `(line, message)` in document order.
pub(crate) fn problems(inputs: &[InputDecl]) -> Vec<(usize, String)> {
    let mut problems = Vec::new();
    for input in inputs {
        check_one(input, &mut problems);
    }

    // The cap is reported once, at the input that crosses it: repeating it for every later input would
    // turn one over-long block into a wall.
    let dispatch: Vec<&InputDecl> = inputs
        .iter()
        .filter(|input| input.event == DISPATCH_EVENT)
        .collect();
    if let Some(over) = dispatch.get(WORKFLOW_DISPATCH_INPUT_CAP) {
        problems.push((
            over.line,
            format!(
                "a `{DISPATCH_EVENT}` block may declare at most {WORKFLOW_DISPATCH_INPUT_CAP} inputs, and this is number {}",
                WORKFLOW_DISPATCH_INPUT_CAP + 1
            ),
        ));
    }
    problems
}

/// The name of the manual-dispatch event, whose inputs the platform caps.
const DISPATCH_EVENT: &str = "workflow_dispatch";

/// The name of the reusable-workflow event, whose inputs must declare a type.
const REUSED_EVENT: &str = "workflow_call";

/// The problems with one declaration.
fn check_one(input: &InputDecl, problems: &mut Vec<(usize, String)>) {
    let reused = input.event == REUSED_EVENT;
    let kind = input.kind.as_ref().map(|declared| declared.value.as_str());

    // Is the declared type one this event accepts? A manual input with no type is a string, which the
    // docs state; a reusable one with no type is already an error of its own. Everything that depends
    // on the type — the choice/options agreement, a default's shape — waits on this, because an
    // invalid type is the root cause and reporting its consequences too turns one mistake into three.
    let type_ok = match kind {
        Some(declared) => allowed_for(input.event).contains(&declared),
        None => !reused,
    };

    match kind {
        // A reusable workflow's input without a type is refused; a manual one without a type is a
        // string, which the docs state.
        None if reused => problems.push((
            input.line,
            format!(
                "input '{}' of a `{REUSED_EVENT}` block must declare a `type`",
                input.id
            ),
        )),
        Some(declared) if !type_ok => problems.push((
            input.kind.as_ref().map_or(input.line, |value| value.line),
            format!(
                "'{declared}' is not a type a `{}` input accepts; it takes {}",
                input.event,
                allowed_for(input.event).join(", ")
            ),
        )),
        _ => {}
    }

    if let Some(default) = &input.default {
        if type_ok && !is_expression(&default.value) {
            match kind {
                Some("number") if default.value.parse::<f64>().is_err() => problems.push((
                    default.line,
                    format!(
                        "input '{}' is typed `number`, so its default '{}' cannot be read as one",
                        input.id, default.value
                    ),
                )),
                Some("boolean")
                    if !matches!(
                        default.value.to_ascii_lowercase().as_str(),
                        "true" | "false"
                    ) =>
                {
                    problems.push((
                        default.line,
                        format!(
                            "input '{}' is typed `boolean`, so its default must be `true` or `false`, not '{}'",
                            input.id, default.value
                        ),
                    ));
                }
                _ => {}
            }
        }
    }

    // A required reusable-workflow input is passed by every caller, so its default never applies.
    let required = input
        .required
        .as_ref()
        .is_some_and(|value| value.value.eq_ignore_ascii_case("true"));
    if reused && required {
        if let Some(default) = &input.default {
            problems.push((
                default.line,
                format!(
                    "input '{}' is required, so its default '{}' is never used",
                    input.id, default.value
                ),
            ));
        }
    }

    if kind == Some("choice") && input.options.is_empty() && type_ok {
        problems.push((
            input.line,
            format!(
                "input '{}' is a `choice` with no `options`, so there is nothing to choose",
                input.id
            ),
        ));
    }
    if kind != Some("choice") && !input.options.is_empty() && type_ok {
        problems.push((
            input.line,
            format!(
                "`options` only apply to a `choice` input, and '{}' is not one",
                input.id
            ),
        ));
    }

    let mut seen: Vec<&str> = Vec::new();
    for option in &input.options {
        if seen.contains(&option.value.as_str()) {
            problems.push((
                option.line,
                format!(
                    "option '{}' appears twice in input '{}'",
                    option.value, input.id
                ),
            ));
        }
        seen.push(&option.value);
    }

    if kind == Some("choice") && type_ok {
        if let Some(default) = &input.default {
            let listed = input
                .options
                .iter()
                .any(|option| option.value == default.value);
            if !listed && !is_expression(&default.value) {
                problems.push((
                    default.line,
                    format!(
                        "default '{}' of input '{}' is not one of its options",
                        default.value, input.id
                    ),
                ));
            }
        }
    }
}

/// The input types one event accepts.
fn allowed_for(event: &str) -> &'static [&'static str] {
    if event == REUSED_EVENT {
        WORKFLOW_CALL_INPUT_TYPES
    } else {
        WORKFLOW_DISPATCH_INPUT_TYPES
    }
}
