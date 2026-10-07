//! The events rule: an `on` trigger must name a known event and use filters it accepts.

use crate::cron;
use crate::data::{
    ActivityTypes, SCHEDULE_MINIMUM_GAP_MINUTES, activity_types, filter_events, is_known_timezone,
};
use crate::model::{Document, EventFilter, Trigger};
use crate::rule::{Finding, Rule, RuleMeta, Severity};

/// The event whose `workflows` filter names the runs to follow.
const WORKFLOW_RUN_EVENT: &str = "workflow_run";

/// The filter naming an event's activity types.
const TYPES_FILTER: &str = "types";

/// The filter naming the workflows a `workflow_run` event follows.
const WORKFLOWS_FILTER: &str = "workflows";

/// A document must trigger on events the platform has, using filters those events accept.
pub struct TriggerEvents;

impl TriggerEvents {
    /// The rule's metadata.
    pub const META: RuleMeta = RuleMeta {
        id: "events",
        severity: Severity::Error,
        description: "an `on` event is unknown, uses a filter it does not accept, or has an unusable `schedule`",
    };

    /// Record one finding for this rule.
    fn push(&self, findings: &mut Vec<Finding>, message: String, line: usize) {
        findings.push(Finding {
            rule: self.id(),
            severity: self.meta().severity,
            message,
            line,
        });
    }

    /// Check one filter list against the event that declares it.
    fn check_filter(
        &self,
        trigger: &Trigger,
        filter: &EventFilter,
        types: &ActivityTypes,
        findings: &mut Vec<Finding>,
    ) {
        if filter.name == TYPES_FILTER {
            self.check_activity_types(trigger, filter, types, findings);
            return;
        }

        if filter.name == WORKFLOWS_FILTER {
            if trigger.name != WORKFLOW_RUN_EVENT {
                self.push(
                    findings,
                    format!("'{WORKFLOWS_FILTER}' is only available for the '{WORKFLOW_RUN_EVENT}' event"),
                    filter.line,
                );
            }
            return;
        }

        let Some(events) = filter_events(&filter.name) else {
            return;
        };

        // A filter with no values constrains nothing, so the platform never refuses it. This also
        // keeps a declared-but-empty filter from being reported twice for one misuse.
        if filter.values.is_empty() {
            return;
        }

        if !events.contains(&trigger.name.as_str()) {
            self.push(
                findings,
                format!(
                    "filter '{}' is not available for the '{}' event",
                    filter.name, trigger.name
                ),
                filter.line,
            );
        }

        // A filter and its negated form cannot both be used on one event. Report from the negated
        // side only, so a pair is reported once rather than once per direction.
        if let Some(base) = filter.name.strip_suffix("-ignore") {
            let conflicts = trigger
                .filters
                .iter()
                .any(|other| other.name == base && !other.values.is_empty());
            if conflicts {
                self.push(
                    findings,
                    format!(
                        "filters '{base}' and '{}' cannot be used for the same event",
                        filter.name
                    ),
                    filter.line,
                );
            }
        }
    }

    /// Check a `types` filter against the activity types the event accepts.
    fn check_activity_types(
        &self,
        trigger: &Trigger,
        filter: &EventFilter,
        types: &ActivityTypes,
        findings: &mut Vec<Finding>,
    ) {
        let ActivityTypes::Only(allowed) = types else {
            // Any activity type is valid for this event, or the event was already reported.
            return;
        };

        if allowed.is_empty() {
            self.push(
                findings,
                format!(
                    "'types' cannot be specified for the '{}' event",
                    trigger.name
                ),
                trigger.line,
            );
            return;
        }

        for value in &filter.values {
            if !allowed.contains(&value.value.as_str()) {
                self.push(
                    findings,
                    format!(
                        "activity type '{}' is not valid for the '{}' event",
                        value.value, trigger.name
                    ),
                    value.line,
                );
            }
        }
    }

    /// A `workflow_run` event must name at least one workflow to follow.
    fn check_workflow_run(&self, trigger: &Trigger, findings: &mut Vec<Finding>) {
        let named = trigger
            .filters
            .iter()
            .any(|filter| filter.name == WORKFLOWS_FILTER && !filter.values.is_empty());
        if !named {
            self.push(
                findings,
                format!(
                    "the '{WORKFLOW_RUN_EVENT}' event must name at least one workflow in '{WORKFLOWS_FILTER}'"
                ),
                trigger.line,
            );
        }
    }

    /// Check the `on.schedule` entries: the expression, its interval, and the timezone name.
    ///
    /// These belong with the event checks because that is where the platform puts them — a malformed
    /// schedule is refused with the workflow, before any job is scheduled. A schedule the platform
    /// cannot run is not a style question.
    fn check_schedules(&self, doc: &Document, findings: &mut Vec<Finding>) {
        for entry in &doc.schedules {
            let Some(cron_value) = &entry.cron else {
                self.push(
                    findings,
                    "a schedule entry needs a `cron` expression".to_owned(),
                    entry.line,
                );
                continue;
            };

            // The expression and the timezone are judged independently: one being wrong does not make
            // the other right, and a reader who fixes only the first should not have to run it again
            // to learn about the second.
            let parses = match cron::invalid(&cron_value.value) {
                Some(problem) => {
                    self.push(
                        findings,
                        format!("invalid cron expression '{}': {problem}", cron_value.value),
                        cron_value.line,
                    );
                    false
                }
                None => true,
            };

            // The interval means nothing for an expression that does not parse.
            //
            // Written without a let-chain on purpose: this crate declares `rust-version = "1.85"`, and a
            // chain needs 1.88. A published floor is worth more than the tidier syntax, and the check
            // above is what keeps the declaration true.
            if parses {
                let short = cron::minimum_gap_minutes(&cron_value.value)
                    .filter(|gap| *gap < SCHEDULE_MINIMUM_GAP_MINUTES);
                if let Some(gap) = short {
                    self.push(
                        findings,
                        format!(
                            "this expression fires every {gap} min, and the platform runs a scheduled workflow at most once every {SCHEDULE_MINIMUM_GAP_MINUTES} min"
                        ),
                        cron_value.line,
                    );
                }
            }

            let unknown_timezone = entry
                .timezone
                .as_ref()
                .filter(|timezone| !is_known_timezone(&timezone.value));
            if let Some(timezone) = unknown_timezone {
                self.push(
                    findings,
                    format!(
                        "'{}' is not a timezone name the platform knows",
                        timezone.value
                    ),
                    timezone.line,
                );
            }
        }
    }
}

impl Rule for TriggerEvents {
    fn meta(&self) -> &'static RuleMeta {
        &Self::META
    }

    fn check(&self, doc: &Document, findings: &mut Vec<Finding>) {
        self.check_schedules(doc, findings);
        // The declared inputs are judged by a module of their own: the schema is data, and the
        // judgement is a pure function of the declarations, so it needs no registry and no file.
        for (line, message) in crate::inputs::problems(&doc.inputs) {
            self.push(findings, message, line);
        }
        for trigger in &doc.triggers {
            let types = activity_types(&trigger.name);
            if matches!(types, ActivityTypes::UnknownEvent) {
                self.push(
                    findings,
                    format!("event '{}' is unknown", trigger.name),
                    trigger.line,
                );
                continue;
            }

            for filter in &trigger.filters {
                self.check_filter(trigger, filter, &types, findings);
            }

            if trigger.name == WORKFLOW_RUN_EVENT {
                self.check_workflow_run(trigger, findings);
            }
        }
    }
}
