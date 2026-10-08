//! Vendor facts as data: the runner labels the platform hosts, and the operating system each
//! provides.
//!
//! Facts about a vendor's product belong in tables, never in branches. The label set is the union of
//! GitHub's own documentation (`data/reusables/actions/*.md` in `github/docs`) and actionlint's
//! curated list (`rule_runner_label.go`), each read 2026-10-03. Neither is a superset, so a label
//! either accepts is accepted here — a linter must not false-positive on a real label.
//!
//! The timezone names live in their own module beside this one: 598 of them is a table, not a file's
//! worth of editing, and they come from the tz database rather than from a vendor's docs.

mod timezones;

pub(crate) use timezones::is_known as is_known_timezone;

use std::sync::LazyLock;

/// The shortest interval the platform runs a scheduled workflow at.
///
/// From the docs' own reusable (`data/reusables/repositories/actions-scheduled-workflow-example.md`):
/// "The shortest interval you can run scheduled workflows is once every 5 minutes." A schedule that
/// fires more often than this is silently stretched, so the author gets fewer runs than they wrote.
pub(crate) const SCHEDULE_MINIMUM_GAP_MINUTES: u32 = 5;

use regex::Regex;

/// The operating system a runner provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OsFamily {
    /// GitHub-hosted Ubuntu runners.
    Linux,
    /// GitHub-hosted Windows runners.
    Windows,
    /// GitHub-hosted macOS runners.
    Macos,
}

impl OsFamily {
    /// The name used in a finding's message.
    pub(crate) fn name(&self) -> &'static str {
        match self {
            OsFamily::Linux => "Linux",
            OsFamily::Windows => "Windows",
            OsFamily::Macos => "macOS",
        }
    }
}

/// Hosted labels providing Linux.
pub(crate) const LINUX_LABELS: &[&str] = &[
    "ubuntu-slim",
    "ubuntu-latest",
    "ubuntu-latest-4-cores",
    "ubuntu-latest-8-cores",
    "ubuntu-latest-16-cores",
    "ubuntu-26.04",
    "ubuntu-26.04-arm",
    "ubuntu-24.04",
    "ubuntu-24.04-arm",
    "ubuntu-22.04",
    "ubuntu-22.04-arm",
];

/// Hosted labels providing Windows.
pub(crate) const WINDOWS_LABELS: &[&str] = &[
    "windows-latest",
    "windows-latest-8-cores",
    "windows-2025",
    "windows-2025-vs2026",
    "windows-2022",
    "windows-11-arm",
    "windows-11-vs2026-arm",
];

/// Hosted labels providing macOS.
pub(crate) const MACOS_LABELS: &[&str] = &[
    "macos-latest",
    "macos-latest-large",
    "macos-latest-xlarge",
    "macos-26",
    "macos-26-intel",
    "macos-26-large",
    "macos-26-xlarge",
    "macos-15",
    "macos-15-intel",
    "macos-15-large",
    "macos-15-xlarge",
    "macos-14",
    "macos-14-large",
    "macos-14-xlarge",
    "xcode-27",
    "xcode-27-xlarge",
];

/// The label that marks a job as running on a self-hosted runner.
pub(crate) const SELF_HOSTED_LABEL: &str = "self-hosted";

/// The preset labels every self-hosted runner answers to, whatever custom labels it also carries.
pub(crate) const SELF_HOSTED_PRESET_LABELS: &[&str] = &[
    "self-hosted",
    "linux",
    "macos",
    "windows",
    "x64",
    "arm",
    "arm64",
];

/// The operating system a label provides, if the label is one this crate knows to provide one.
///
/// This covers hosted labels and the self-hosted operating-system presets (`linux`, `macos`,
/// `windows`); the non-operating-system presets (`x64`, `arm`, `arm64`, `self-hosted`) return
/// `None`, because they say nothing about the platform.
pub(crate) fn os_family(label: &str) -> Option<OsFamily> {
    if in_table(LINUX_LABELS, label) {
        Some(OsFamily::Linux)
    } else if in_table(WINDOWS_LABELS, label) {
        Some(OsFamily::Windows)
    } else if in_table(MACOS_LABELS, label) {
        Some(OsFamily::Macos)
    } else {
        None
    }
}

/// Whether a label is one this crate knows: hosted, or a self-hosted preset.
pub(crate) fn is_known(label: &str) -> bool {
    os_family(label).is_some() || in_table(SELF_HOSTED_PRESET_LABELS, label)
}

/// Case-insensitive membership, so a label's capitalisation never decides the outcome.
fn in_table(table: &[&str], label: &str) -> bool {
    table.iter().any(|known| known.eq_ignore_ascii_case(label))
}

/// The permission scopes the platform exposes.
///
/// Union of GitHub's docs table
/// (`data/reusables/actions/github-token-available-permissions.md`, read 2026-10-03) and actionlint's
/// `rule_permissions.go`; neither is a superset.
pub(crate) const PERMISSION_SCOPES: &[&str] = &[
    "actions",
    "artifact-metadata",
    "attestations",
    "checks",
    "code-quality",
    "contents",
    "deployments",
    "discussions",
    "id-token",
    "issues",
    "models",
    "packages",
    "pages",
    "pull-requests",
    "repository-projects",
    "security-events",
    "statuses",
    "vulnerability-alerts",
];

/// The access a single permission may take.
///
/// Deliberately the general set the docs state, not actionlint's per-scope narrowing (`id-token`
/// takes only `write`/`none`): a per-scope claim goes stale and false-positives, and a false report
/// costs more than a missing one.
pub(crate) const PERMISSION_ACCESS: &[&str] = &["read", "write", "none"];

/// The whole-block values that set every permission at once.
pub(crate) const PERMISSION_ALL_VALUES: &[&str] = &["read-all", "write-all"];

/// The event filters whose values are git ref patterns.
pub(crate) const REF_FILTER_KEYS: &[&str] = &["branches", "branches-ignore", "tags", "tags-ignore"];

/// The event filters whose values are file path patterns.
pub(crate) const PATH_FILTER_KEYS: &[&str] = &["paths", "paths-ignore"];

/// Whether a scope name is one the platform exposes.
pub(crate) fn is_known_scope(scope: &str) -> bool {
    in_table(PERMISSION_SCOPES, scope)
}

/// Whether an access value is one a permission may take.
pub(crate) fn is_known_access(access: &str) -> bool {
    in_table(PERMISSION_ACCESS, access)
}

/// Whether a whole-block value sets every permission at once.
pub(crate) fn is_known_permissions_value(value: &str) -> bool {
    in_table(PERMISSION_ALL_VALUES, value)
}

/// The platform's grammar for a job or step id.
///
/// A letter or underscore, then letters, digits, hyphens, or underscores. ASCII on purpose: the
/// platform's ids are ASCII, so `café` is not a valid id even though it is a valid letter sequence.
const ID_GRAMMAR: &str = r"^[a-zA-Z_][a-zA-Z0-9_-]*$";

/// The compiled id grammar (compiled once, not per document).
static ID_GRAMMAR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(ID_GRAMMAR).expect("the id grammar is a valid regex"));

/// Whether an id matches the platform's grammar.
pub(crate) fn is_valid_id(id: &str) -> bool {
    ID_GRAMMAR_REGEX.is_match(id)
}

/// The platform a job's runner provides, as far as available shell names are concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellPlatform {
    /// The runner's platform is unknown, so only shells every platform has are certain.
    Any,
    /// A Windows runner.
    Windows,
    /// A macOS or Linux runner.
    Unix,
}

/// The shells a runner provides, by platform.
///
/// Source: the `shell` section of the workflow syntax reference — `sh` is Unix only, `cmd` and
/// `powershell` are Windows only, and the rest are everywhere.
pub(crate) const SHELLS_ANY: &[&str] = &["bash", "cmd", "powershell", "pwsh", "python", "sh"];
/// The shells a Windows runner provides.
pub(crate) const SHELLS_WINDOWS: &[&str] = &["bash", "cmd", "powershell", "pwsh", "python"];
/// The shells a macOS or Linux runner provides.
pub(crate) const SHELLS_UNIX: &[&str] = &["bash", "pwsh", "python", "sh"];

/// The shells a runner of the given platform provides.
pub(crate) fn shells_for(platform: ShellPlatform) -> &'static [&'static str] {
    match platform {
        ShellPlatform::Any => SHELLS_ANY,
        ShellPlatform::Windows => SHELLS_WINDOWS,
        ShellPlatform::Unix => SHELLS_UNIX,
    }
}

/// Whether a name is a shell on any platform, even if not on this one.
pub(crate) fn is_known_shell(name: &str) -> bool {
    in_table(SHELLS_ANY, name)
}

/// The platform a job's `runs-on` labels select.
///
/// A job naming no runner, or naming both a Windows and a Unix label, is `Any`: the labels decide
/// nothing here, and the conflicting-labels case belongs to the runner-label rule.
pub(crate) fn shell_platform(labels: &[String]) -> ShellPlatform {
    let mut found: Option<ShellPlatform> = None;
    for label in labels {
        let kind = match os_family(label) {
            Some(OsFamily::Windows) => ShellPlatform::Windows,
            Some(OsFamily::Linux | OsFamily::Macos) => ShellPlatform::Unix,
            None => continue,
        };
        match found {
            None => found = Some(kind),
            Some(seen) if seen == kind => {}
            Some(_) => return ShellPlatform::Any,
        }
    }
    found.unwrap_or(ShellPlatform::Any)
}

/// Workflow commands the platform retired, each with the form that replaces it.
///
/// Sources: the deprecation announcements of 2020-10-01 (`set-env`, `add-path`) and 2022-10-11
/// (`save-state`, `set-output`) — the same two actionlint cites.
pub(crate) const DEPRECATED_COMMANDS: &[(&str, &str)] = &[
    ("set-output", r#"echo "{name}={value}" >> $GITHUB_OUTPUT"#),
    ("save-state", r#"echo "{name}={value}" >> $GITHUB_STATE"#),
    ("set-env", r#"echo "{name}={value}" >> $GITHUB_ENV"#),
    ("add-path", r#"echo "{path}" >> $GITHUB_PATH"#),
];

/// The replacement for a retired command, if the name is one.
pub(crate) fn replacement_for(command: &str) -> Option<&'static str> {
    DEPRECATED_COMMANDS
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, replacement)| *replacement)
}

/// A retired command inside a script.
///
/// The platform reads a command from a script's OUTPUT, and only in one form:
/// `::name parameter=value::data`. The closing `::` is what makes the text a command — without it the
/// line is ordinary output that merely mentions the name, so `echo "::set-output test"` must not be
/// reported. Requiring the closing `::` is therefore the difference between reading the script and
/// pattern-matching it.
///
/// The trailing class keeps a longer word from matching the retired token: `::set-outputs` is not
/// `::set-output`. The `regex` crate has no lookaround, so that character is consumed by the group
/// after it, and `[^\n]` keeps the closing `::` on the command's own line.
const DEPRECATED_COMMAND_PATTERN: &str =
    r"::(save-state|set-output|set-env|add-path)(::|[^a-zA-Z0-9_\n-][^\n]*::)";

/// The compiled retired-command pattern.
static DEPRECATED_COMMAND_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(DEPRECATED_COMMAND_PATTERN).expect("the retired-command pattern is valid")
});

/// Every retired command used in a script, with the **byte offset** of each command.
///
/// The offset lets a finding point at the command's own line inside a multi-line script rather
/// than at the script's first line.
pub(crate) fn deprecated_commands_in(script: &str) -> Vec<(&str, usize)> {
    DEPRECATED_COMMAND_REGEX
        .captures_iter(script)
        .filter_map(|captures| {
            let command = captures.get(1)?;
            Some((command.as_str(), command.start()))
        })
        .collect()
}

/// The activity types each trigger event accepts, as documented.
///
/// Source: GitHub's "Events that trigger workflows" (read 2026-10-03), cross-checked against
/// actionlint's table. An event mapped to an empty list accepts no activity type.
pub(crate) const TRIGGER_ACTIVITY_TYPES: &[(&str, &[&str])] = &[
    ("branch_protection_rule", &["created", "edited", "deleted"]),
    (
        "check_run",
        &["created", "rerequested", "completed", "requested_action"],
    ),
    ("check_suite", &["completed"]),
    ("create", &[]),
    ("delete", &[]),
    ("deployment", &[]),
    ("deployment_status", &[]),
    (
        "discussion",
        &[
            "created",
            "edited",
            "deleted",
            "transferred",
            "pinned",
            "unpinned",
            "labeled",
            "unlabeled",
            "locked",
            "unlocked",
            "category_changed",
            "answered",
            "unanswered",
        ],
    ),
    ("discussion_comment", &["created", "edited", "deleted"]),
    ("fork", &[]),
    ("gollum", &[]),
    ("image_version", &[]),
    ("issue_comment", &["created", "edited", "deleted"]),
    (
        "issues",
        &[
            "opened",
            "edited",
            "deleted",
            "transferred",
            "pinned",
            "unpinned",
            "closed",
            "reopened",
            "assigned",
            "unassigned",
            "labeled",
            "unlabeled",
            "locked",
            "unlocked",
            "milestoned",
            "demilestoned",
            "typed",
            "untyped",
        ],
    ),
    ("label", &["created", "edited", "deleted"]),
    ("merge_group", &["checks_requested"]),
    (
        "milestone",
        &["created", "closed", "opened", "edited", "deleted"],
    ),
    ("page_build", &[]),
    ("public", &[]),
    (
        "pull_request",
        &[
            "assigned",
            "unassigned",
            "labeled",
            "unlabeled",
            "opened",
            "edited",
            "closed",
            "reopened",
            "synchronize",
            "converted_to_draft",
            "locked",
            "unlocked",
            "enqueued",
            "dequeued",
            "milestoned",
            "demilestoned",
            "ready_for_review",
            "review_requested",
            "review_request_removed",
            "auto_merge_enabled",
            "auto_merge_disabled",
        ],
    ),
    ("pull_request_review", &["submitted", "edited", "dismissed"]),
    (
        "pull_request_review_comment",
        &["created", "edited", "deleted"],
    ),
    (
        "pull_request_target",
        &[
            "assigned",
            "unassigned",
            "labeled",
            "unlabeled",
            "opened",
            "edited",
            "closed",
            "reopened",
            "synchronize",
            "converted_to_draft",
            "locked",
            "unlocked",
            "enqueued",
            "dequeued",
            "milestoned",
            "demilestoned",
            "ready_for_review",
            "review_requested",
            "review_request_removed",
            "auto_merge_enabled",
            "auto_merge_disabled",
        ],
    ),
    ("push", &[]),
    ("registry_package", &["published", "updated"]),
    (
        "release",
        &[
            "published",
            "unpublished",
            "created",
            "edited",
            "deleted",
            "prereleased",
            "released",
        ],
    ),
    ("schedule", &[]),
    ("status", &[]),
    ("watch", &["started"]),
    ("workflow_call", &[]),
    ("workflow_dispatch", &[]),
    ("workflow_run", &["completed", "requested", "in_progress"]),
];

/// Trigger events whose activity types the caller defines, so any type is valid.
pub(crate) const UNCONSTRAINED_ACTIVITY_TYPES: &[&str] = &["repository_dispatch"];

/// The events that filter on branches.
const BRANCH_FILTER_EVENTS: &[&str] = &[
    "merge_group",
    "push",
    "pull_request",
    "pull_request_target",
    "workflow_run",
];

/// The events that filter on changed paths.
const PATH_FILTER_EVENTS: &[&str] = &["push", "pull_request", "pull_request_target"];

/// The events that filter on tags.
const TAG_FILTER_EVENTS: &[&str] = &["push"];

/// The events each filter is available for, including the negated form of the same filter.
///
/// A filter used on any other event is refused by the platform.
pub(crate) const FILTER_EVENTS: &[(&str, &[&str])] = &[
    ("branches", BRANCH_FILTER_EVENTS),
    ("branches-ignore", BRANCH_FILTER_EVENTS),
    ("paths", PATH_FILTER_EVENTS),
    ("paths-ignore", PATH_FILTER_EVENTS),
    ("tags", TAG_FILTER_EVENTS),
    ("tags-ignore", TAG_FILTER_EVENTS),
];

/// What activity types a trigger event accepts.
pub(crate) enum ActivityTypes {
    /// The name is not one the platform can trigger on.
    UnknownEvent,
    /// Any activity type is valid; the caller defines them.
    Any,
    /// Only these types are valid. An empty list means the event accepts none.
    Only(&'static [&'static str]),
}

/// The activity types a trigger event accepts.
pub(crate) fn activity_types(event: &str) -> ActivityTypes {
    if UNCONSTRAINED_ACTIVITY_TYPES.contains(&event) {
        return ActivityTypes::Any;
    }
    match TRIGGER_ACTIVITY_TYPES
        .iter()
        .find(|(name, _)| *name == event)
    {
        Some((_, types)) => ActivityTypes::Only(types),
        None => ActivityTypes::UnknownEvent,
    }
}

/// The events a filter is available for, or `None` when the name is not a filter.
pub(crate) fn filter_events(filter: &str) -> Option<&'static [&'static str]> {
    FILTER_EVENTS
        .iter()
        .find(|(name, _)| *name == filter)
        .map(|(_, events)| *events)
}

/// The annotation level each severity is reported as.
///
/// The runner's log parser recognises `error`, `warning`, and `notice`; our `note` is the third.
/// Source: the `::error`/`::warning`/`::notice` command forms in GitHub's "Workflow commands",
/// read 2026-10-04. A severity missing from this table would silently lose its annotation, so a
/// test asserts the table covers every severity.
pub(crate) const ANNOTATION_LEVELS: &[(&str, &str)] = &[
    ("error", "error"),
    ("warning", "warning"),
    ("note", "notice"),
];

/// The level used when a severity has no table entry: the loudest one, never a quiet fallback.
pub(crate) const DEFAULT_ANNOTATION_LEVEL: &str = "error";

/// The annotation level for a severity name.
pub(crate) fn annotation_level(severity: &str) -> Option<&'static str> {
    ANNOTATION_LEVELS
        .iter()
        .find(|(name, _)| *name == severity)
        .map(|(_, level)| *level)
}

/// The SARIF result level each severity is reported as.
///
/// SARIF 2.1.0 spells its levels `none`, `note`, `warning`, and `error`; our three severities happen
/// to share three of those words, so the correspondence is an identity. It is declared as data
/// anyway, for the same reason as the annotation levels: the vocabulary lives in one place, and a
/// severity added later cannot silently lose its level.
pub(crate) const SARIF_LEVELS: &[(&str, &str)] =
    &[("error", "error"), ("warning", "warning"), ("note", "note")];

/// The SARIF result level for a severity name.
pub(crate) fn sarif_level(severity: &str) -> Option<&'static str> {
    SARIF_LEVELS
        .iter()
        .find(|(name, _)| *name == severity)
        .map(|(_, level)| *level)
}

/// The input types a `workflow_call` block accepts.
///
/// Quoted from the workflow-syntax reference: `on.workflow_call.inputs.<input_id>.type` must be one of
/// `boolean`, `number`, or `string`. The docs also make it **required**: "In addition to the standard
/// input parameters that are available, `on.workflow_call.inputs` requires a `type` parameter."
pub(crate) const WORKFLOW_CALL_INPUT_TYPES: &[&str] = &["boolean", "number", "string"];

/// The input types a `workflow_dispatch` block accepts.
///
/// Quoted from the workflow-syntax reference: `on.workflow_dispatch.inputs.<input_id>.type` "must be
/// one of: `boolean`, `choice`, `number`, `environment` or `string`". Unlike the reusable-workflow
/// case, the docs do not make it required here — an input without a `type` is a string.
pub(crate) const WORKFLOW_DISPATCH_INPUT_TYPES: &[&str] =
    &["boolean", "choice", "number", "environment", "string"];

/// The most inputs one `workflow_dispatch` block may declare.
///
/// The platform's limit, quoted in actionlint's `rule_events.go` from the documentation: "maximum
/// number of inputs for `workflow_dispatch` event is 25".
pub(crate) const WORKFLOW_DISPATCH_INPUT_CAP: usize = 25;

/// One field of a cron expression: what it is called, the values it accepts, and its names.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CronField {
    /// The field's name, for a message that says which field is wrong.
    pub name: &'static str,
    /// The lowest value it accepts.
    pub min: u32,
    /// The highest value it accepts.
    pub max: u32,
    /// The names it accepts for its values, if any.
    pub names: &'static [(&'static str, u32)],
}

/// The names accepted in the month field.
const MONTH_NAMES: [(&str, u32); 12] = [
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

/// The names accepted in the day-of-week field, numbered as the platform numbers them: Sunday is 0.
const DAY_NAMES: [(&str, u32); 7] = [
    ("sun", 0),
    ("mon", 1),
    ("tue", 2),
    ("wed", 3),
    ("thu", 4),
    ("fri", 5),
    ("sat", 6),
];

/// The five fields of a cron expression, in the order they are written.
///
/// The bounds are the usual crontab ones. The platform's docs name the operators (`*`, `,`, `-`, `/`)
/// but not the bounds; names are accepted, as actionlint accepts them:
/// rejecting a cron the platform accepts is a false positive, which is the worse of the two errors.
pub(crate) const CRON_FIELDS: [CronField; 5] = [
    CronField {
        name: "minute",
        min: 0,
        max: 59,
        names: &[],
    },
    CronField {
        name: "hour",
        min: 0,
        max: 23,
        names: &[],
    },
    CronField {
        name: "day-of-month",
        min: 1,
        max: 31,
        names: &[],
    },
    CronField {
        name: "month",
        min: 1,
        max: 12,
        names: &MONTH_NAMES,
    },
    CronField {
        name: "day-of-week",
        min: 0,
        max: 6,
        names: &DAY_NAMES,
    },
];
