//! Parse workflow text into a small, position-carrying document model.
//!
//! This module owns the YAML dependency and nothing else. It builds the minimum tree the
//! rules need — mappings, sequences and scalars, each with the 1-based source line it
//! starts on — rather than a general YAML value type, because a linter needs positions
//! and the rules never ask about anything else.

use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::Marker;

use std::collections::HashMap;

use crate::Error;

/// The key naming a job's steps in the document format.
const KEY_STEPS: &str = "steps";

/// The YAML merge key: `<<: *base` inserts the anchored mapping's entries into this one.
///
/// Implemented, with YAML's two precedence rules: a key the mapping writes itself always wins, and in
/// a list of mappings the earlier one wins. Both are applied when the mapping *ends* rather than when
/// `<<` is read, so a map's own key wins whether it is written above or below the merge.
const MERGE_KEY: &str = "<<";
/// The key naming the top-level job collection.
const KEY_JOBS: &str = "jobs";
/// The key naming the action reference on a step.
const KEY_USES: &str = "uses";
/// The key naming a step's display name.
const KEY_NAME: &str = "name";
/// The key naming a job's or a step's identifier.
const KEY_ID: &str = "id";
/// The key carrying a step's script.
const KEY_RUN: &str = "run";
/// The key carrying a job's or a step's condition.
const KEY_IF: &str = "if";
/// The key naming the shell a step's script runs in.
const KEY_SHELL: &str = "shell";
/// The key declaring environment variables.
const KEY_ENV: &str = "env";
/// The key naming a job's container.
const KEY_CONTAINER: &str = "container";
/// The key naming a job's service containers.
const KEY_SERVICES: &str = "services";
/// The key holding a container's credentials.
const KEY_CREDENTIALS: &str = "credentials";
/// The key holding a credential's password.
const KEY_PASSWORD: &str = "password";
/// The key holding a job's strategy.
const KEY_STRATEGY: &str = "strategy";
/// The key holding a strategy's matrix.
const KEY_MATRIX: &str = "matrix";
/// The key holding a matrix's additions.
const KEY_INCLUDE: &str = "include";
/// The key holding a matrix's removals.
const KEY_EXCLUDE: &str = "exclude";
/// The key declaring which events trigger the workflow.
const KEY_ON: &str = "on";
/// The key bounding a job's runtime.
const KEY_TIMEOUT: &str = "timeout-minutes";
/// The key naming the labels a job's runner must carry.
const KEY_RUNS_ON: &str = "runs-on";
/// The key declaring what the workflow token may do.
const KEY_PERMISSIONS: &str = "permissions";
/// The key naming the jobs a job depends on.
const KEY_NEEDS: &str = "needs";
/// The key naming the scheduled times, and the two keys an entry may carry.
const KEY_SCHEDULE: &str = "schedule";
/// The key naming a schedule entry's cron expression.
const KEY_CRON: &str = "cron";
/// The key naming a schedule entry's timezone.
const KEY_TIMEZONE: &str = "timezone";
/// The two events that declare inputs, and the keys inside them.
const KEY_WORKFLOW_CALL: &str = "workflow_call";
/// The manual-dispatch event, the other event that declares inputs.
const KEY_WORKFLOW_DISPATCH: &str = "workflow_dispatch";
/// The key naming an event's declared inputs.
const KEY_INPUTS: &str = "inputs";
/// The key naming an input's declared type.
const KEY_TYPE: &str = "type";
/// The key marking an input as required.
const KEY_REQUIRED: &str = "required";
/// The key naming an input's default value.
const KEY_DEFAULT: &str = "default";
/// The key listing the allowed values of a `choice` input.
const KEY_OPTIONS: &str = "options";

/// A node in the document, with the line it starts on.
///
/// A mapping keeps each entry's **key line** as well as its value: a rule about an id (a job's id
/// is its key) must point at the id, not at whatever key happens to follow it.
#[derive(Debug, Clone, PartialEq)]
enum Node {
    /// A scalar value and the line it starts on.
    Scalar(String, usize),
    /// A sequence's items and the line the sequence starts on.
    Sequence(Vec<Node>, usize),
    /// A mapping's `(key, key line, value)` entries, and the line it starts on.
    Mapping(Vec<(String, usize, Node)>, usize),
}

impl Node {
    /// The line this node starts on.
    fn line(&self) -> usize {
        match self {
            Node::Scalar(_, l) | Node::Sequence(_, l) | Node::Mapping(_, l) => *l,
        }
    }

    /// This mapping's value for `key`, or `None` when it is not a mapping that has one.
    fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::Mapping(entries, _) => {
                entries.iter().find(|(k, _, _)| k == key).map(|(_, _, v)| v)
            }
            _ => None,
        }
    }

    /// This node's items, or `None` when it is not a sequence.
    fn as_seq(&self) -> Option<&[Node]> {
        match self {
            Node::Sequence(items, _) => Some(items),
            _ => None,
        }
    }

    /// This node's text, or `None` when it is not a scalar.
    fn as_str(&self) -> Option<&str> {
        match self {
            Node::Scalar(s, _) => Some(s),
            _ => None,
        }
    }
}

/// One stack frame while assembling the tree from the event stream.
enum Frame {
    /// A sequence being collected.
    Seq {
        /// The items collected so far.
        items: Vec<Node>,
        /// The line the sequence starts on.
        line: usize,
        /// The anchor this sequence carries, or 0 for none.
        anchor: usize,
    },
    /// A mapping being collected.
    Map {
        /// The `(key, key line, value)` entries collected so far.
        entries: Vec<(String, usize, Node)>,
        /// The line the mapping starts on.
        line: usize,
        /// The key whose value is being collected, with the key's own line.
        key: Option<(String, usize)>,
        /// The anchor this mapping carries, or 0 for none.
        anchor: usize,
        /// The values of this mapping's `<<:` keys, in document order. Held until the mapping ends,
        /// because a key the mapping writes itself wins wherever it is written.
        merges: Vec<Node>,
    },
}

/// Assembles the `Node` tree from the parser's event stream, resolving anchors and merge keys.
#[derive(Default)]
struct Builder {
    /// The frames currently open, outermost first.
    stack: Vec<Frame>,
    /// The finished tree, set once the outermost frame closes.
    root: Option<Node>,
    /// Every node that carried an anchor, by the id the parser gave it, so an alias can be resolved
    /// to a copy of the node it names.
    ///
    /// Without this the alias event was ignored and the value simply vanished from the document: a
    /// step written `uses: *action` reached no rule at all, with no error and no refusal. That is
    /// the silent-finding-loss class, and a linter must not do it.
    anchors: HashMap<usize, Node>,
    /// Set when an alias names an anchor the document never defines. YAML says that is an error, so
    /// the document is refused rather than quietly losing the value.
    dangling_alias: bool,
    /// Set when a merge key names something that cannot be merged: not a mapping, or a list holding
    /// something that is not one. The merge has no meaning, so the document is refused rather than
    /// linted against a mapping that quietly lost entries.
    bad_merge: bool,
    /// Set when a key resolves to a mapping or a list — an alias in key position naming a collection.
    /// YAML's merge key is a plain `<<` scalar, so this is not a merge: it is a key that is not a key.
    complex_key: bool,
}

impl Builder {
    /// Place a finished node where it belongs: on the open frame, or as the root.
    fn attach(&mut self, node: Node) {
        match self.stack.last_mut() {
            None => self.root = Some(node),
            Some(Frame::Seq { items, .. }) => items.push(node),
            Some(Frame::Map {
                entries,
                key,
                merges,
                ..
            }) => {
                if let Some((k, key_line)) = key.take() {
                    // `<<: *base` merges the anchored mapping's entries into this one. The merge is
                    // recorded and applied when the mapping ends, so that a key written by the
                    // mapping itself wins whether it sits above or below the merge key.
                    if k == MERGE_KEY {
                        if matches!(node, Node::Mapping(..) | Node::Sequence(..)) {
                            merges.push(node);
                            return;
                        }
                        self.bad_merge = true;
                    }
                    entries.push((k, key_line, node));
                }
            }
        }
    }

    /// Remember an anchored node, so a later alias can be resolved to a copy of it.
    fn remember(&mut self, anchor: usize, node: &Node) {
        if anchor != 0 {
            self.anchors.insert(anchor, node.clone());
        }
    }

    /// Merge the mappings a `<<:` key names into the mapping's entries.
    ///
    /// Both of YAML's precedence rules live here: a key the mapping writes itself always wins, and in
    /// a list of mappings the earlier one wins. Because this runs when the mapping *ends*, a key
    /// written above the merge key wins exactly as a key written below it does.
    fn merge_into(&mut self, entries: &mut Vec<(String, usize, Node)>, merges: Vec<Node>) {
        for merge in merges {
            for source in self.merge_sources(merge) {
                for (key, line, value) in source {
                    // Written by the mapping, or already brought in by an earlier merge in the list:
                    // either way this one does not override it.
                    if entries.iter().any(|(written, ..)| *written == key) {
                        continue;
                    }
                    entries.push((key, line, value));
                }
            }
        }
    }

    /// The mapping, or the mappings of a list, that a merge value names.
    ///
    /// The merged entries keep the lines they were written on — those really are the lines holding the
    /// text — so a finding about a merged key points at the anchor rather than at the merge.
    fn merge_sources(&mut self, merge: Node) -> Vec<Vec<(String, usize, Node)>> {
        match merge {
            Node::Mapping(entries, _) => vec![entries],
            Node::Sequence(items, _) => {
                let mut sources = Vec::new();
                for item in items {
                    match item {
                        Node::Mapping(entries, _) => sources.push(entries),
                        // A list member that is not a mapping has no merge meaning.
                        _ => self.bad_merge = true,
                    }
                }
                sources
            }
            // Unreachable: `attach` records only a mapping or a list as a merge. The arm stays because
            // what makes a merge value usable is its shape, so the shape is checked where it is used.
            _ => {
                self.bad_merge = true;
                Vec::new()
            }
        }
    }

    /// Resolve an alias, attaching either the named node or, in key position, its scalar value.
    fn resolve_alias(&mut self, anchor: usize, line: usize) {
        let Some(resolved) = self.anchors.get(&anchor).cloned() else {
            // The dependency refuses an unknown anchor before this point, so this is a backstop
            // rather than the usual path — and it reports rather than dropping the value.
            self.dangling_alias = true;
            return;
        };

        // An aliased node is *used* at the alias's position, so its own line becomes that position:
        // a finding about an aliased step must point at the step, not at the anchor it borrowed from
        // (which produced two identical findings on the anchor's line). Nested nodes keep their own
        // lines, because those really are written where the anchor is.
        let resolved = match resolved {
            Node::Scalar(value, _) => Node::Scalar(value, line),
            Node::Sequence(items, _) => Node::Sequence(items, line),
            Node::Mapping(entries, _) => Node::Mapping(entries, line),
        };

        if let Some(Frame::Map { key, .. }) = self.stack.last_mut() {
            if key.is_none() {
                if let Node::Scalar(value, _) = &resolved {
                    *key = Some((value.clone(), line));
                    return;
                }
                // A key must be a scalar, and this one resolves to a mapping or a list. That is not
                // the merge key — `<<` is a plain scalar — it is a key that cannot be a key.
                self.complex_key = true;
                return;
            }
        }
        self.attach(resolved);
    }
}

impl MarkedEventReceiver for Builder {
    fn on_event(&mut self, ev: Event, mark: Marker) {
        let line = mark.line();
        match ev {
            Event::Scalar(value, _, anchor, _) => {
                if let Some(Frame::Map { key, .. }) = self.stack.last_mut() {
                    if key.is_none() {
                        *key = Some((value, line));
                        return;
                    }
                }
                let node = Node::Scalar(value, line);
                self.remember(anchor, &node);
                self.attach(node);
            }
            Event::SequenceStart(anchor, _) => self.stack.push(Frame::Seq {
                items: Vec::new(),
                line,
                anchor,
            }),
            Event::SequenceEnd => {
                if let Some(Frame::Seq {
                    items,
                    line,
                    anchor,
                }) = self.stack.pop()
                {
                    let node = Node::Sequence(items, line);
                    self.remember(anchor, &node);
                    self.attach(node);
                }
            }
            Event::MappingStart(anchor, _) => self.stack.push(Frame::Map {
                entries: Vec::new(),
                line,
                key: None,
                anchor,
                merges: Vec::new(),
            }),
            Event::MappingEnd => {
                if let Some(Frame::Map {
                    mut entries,
                    line,
                    anchor,
                    merges,
                    ..
                }) = self.stack.pop()
                {
                    // Merged before the mapping is remembered, so an alias to this mapping sees the
                    // entries it actually ends up with.
                    self.merge_into(&mut entries, merges);
                    let node = Node::Mapping(entries, line);
                    self.remember(anchor, &node);
                    self.attach(node);
                }
            }
            Event::Alias(anchor) => self.resolve_alias(anchor, line),
            _ => {}
        }
    }
}

/// One filter list under an event, such as `branches` or `paths`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventFilter {
    /// The filter's name (the key under the event).
    pub name: String,
    /// The 1-based line the filter's name sits on.
    pub line: usize,
    /// The filter's values, in document order.
    pub values: Vec<ListedValue>,
}

/// One entry of the document's trigger list (`on:`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    /// The event's name.
    pub name: String,
    /// The 1-based line the event's name sits on.
    pub line: usize,
    /// The filter lists the event declares.
    pub filters: Vec<EventFilter>,
}

/// A value in a list — a matrix value, an event filter pattern — with the line it sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedValue {
    /// The value as written.
    pub value: String,
    /// The 1-based line the value sits on.
    pub line: usize,
}

/// One entry of an `on.schedule` list — `- cron: "…"`, and optionally a `timezone`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleEntry {
    /// The 1-based line the entry starts on.
    pub line: usize,
    /// The `cron` expression and the line it sits on, if the entry declares one.
    ///
    /// Absent is meaningful: an entry with no `cron` schedules nothing, which is a rule's business to
    /// say rather than the model's to hide.
    pub cron: Option<ListedValue>,
    /// The `timezone` name and the line it sits on, if the entry declares one.
    pub timezone: Option<ListedValue>,
}

/// One input declared by `on.workflow_dispatch` or `on.workflow_call`.
///
/// Every field is a value *with its line*, because the judgements about them — a default that
/// contradicts its type, `options` on something that is not a choice — belong to a rule, which needs
/// to point at the line the author would change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDecl {
    /// The event that declares it: `workflow_dispatch` or `workflow_call`.
    ///
    /// The two are not interchangeable: they accept different `type` values, one of them requires a
    /// `type` at all, and only the reusable one makes a default on a required input dead.
    pub event: &'static str,
    /// The input's name — its key under `inputs`.
    pub id: String,
    /// The 1-based line the name sits on.
    pub line: usize,
    /// The declared `type`, if there is one.
    pub kind: Option<ListedValue>,
    /// The declared `required`, if there is one.
    pub required: Option<ListedValue>,
    /// The declared `default`, if there is one.
    pub default: Option<ListedValue>,
    /// The `options` of a `choice` input, in document order.
    pub options: Vec<ListedValue>,
}

/// One row of a job's matrix: its name and the values it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixRow {
    /// The row's name (the key under `matrix`).
    pub name: String,
    /// The row's scalar values, in document order.
    pub values: Vec<ListedValue>,
}

/// A job's `strategy.matrix`.
///
/// `include` and `exclude` are recorded as counts rather than parsed: their combination semantics
/// (subset matching over objects and arrays) are a rule's business, not the model's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Matrix {
    /// The rows, in document order.
    pub rows: Vec<MatrixRow>,
    /// How many combinations `include` declares.
    pub include_count: usize,
    /// How many combinations `exclude` declares.
    pub exclude_count: usize,
    /// The 1-based line the `exclude` key sits on, if the matrix declares one.
    pub exclude_line: Option<usize>,
}

/// A password a container declares, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerPassword {
    /// The service the password belongs to, or `None` for the job's own container.
    pub service: Option<String>,
    /// The password as written.
    pub value: String,
    /// The 1-based line the password value sits on.
    pub line: usize,
}

/// One declared environment variable: its name, and the line the name sits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvEntry {
    /// The variable's name (the key of the `env` mapping).
    pub name: String,
    /// The 1-based line the name sits on.
    pub line: usize,
}

/// A single step of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The 1-based line the step starts on.
    pub line: usize,
    /// The action reference, if this step runs one.
    pub uses: Option<String>,
    /// The line the `uses` value sits on, if this step has one.
    ///
    /// A rule about the reference wants this line, not the step's first line: pointing a
    /// finding at the wrong line is a smaller version of pointing it at the wrong thing.
    pub uses_line: Option<usize>,
    /// The step's identifier, if it declares one.
    pub id: Option<String>,
    /// The line the step's `id` value sits on, if it declares one.
    pub id_line: Option<usize>,
    /// The step's script, if this step runs one.
    pub run: Option<String>,
    /// The line the `run` value starts on, if this step has one.
    pub run_line: Option<usize>,
    /// The step's `if` condition, if it declares one.
    pub condition: Option<String>,
    /// The line the step's `if` value sits on, if it declares one.
    pub condition_line: Option<usize>,
    /// The shell the step's script runs in, if it names one.
    pub shell: Option<String>,
    /// The line the step's `shell` value sits on, if it names one.
    pub shell_line: Option<usize>,
    /// The step's own environment variables.
    pub env: Vec<EnvEntry>,
    /// The step's display name, if it has one.
    pub name: Option<String>,
}

/// One `scope: access` line of a permissions block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionEntry {
    /// The scope name (the key).
    pub scope: String,
    /// The access value.
    pub access: String,
    /// The 1-based line the access value sits on.
    pub line: usize,
}

/// A declared permissions block: either one whole-block token, or one entry per scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionsBlock {
    /// A bare token: `read-all`, `write-all`, `{}`, or an expression.
    Token {
        /// The token as written.
        value: String,
        /// The 1-based line the token sits on.
        line: usize,
    },
    /// One `scope: access` entry per element, in document order.
    Scopes {
        /// The entries, in document order.
        entries: Vec<PermissionEntry>,
        /// The 1-based line the block starts on.
        line: usize,
    },
}

/// A job in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// The job's identifier (its key under `jobs`).
    pub id: String,
    /// The 1-based line the job is written on — the line of its key under `jobs`.
    ///
    /// This is deliberately the KEY's line and not the first line of the job's body: a finding about
    /// the job as a whole (no timeout, a bad `needs`) must point at the job's name, which is what a
    /// reader scans for and where they would add the missing key. The body's first line is one line
    /// further down for every block job, which is how findings used to land on the wrong line.
    pub line: usize,
    /// The declared runtime bound, if any.
    pub timeout_minutes: Option<i64>,
    /// The runner labels the job requires, in document order (a scalar becomes one label).
    pub runs_on: Vec<String>,
    /// The line the `runs-on` value sits on, if the job has one.
    pub runs_on_line: Option<usize>,
    /// The job's own permissions block, if it declares one.
    pub permissions: Option<PermissionsBlock>,
    /// The job ids this job depends on, in document order.
    pub needs: Vec<String>,
    /// The line the `needs` value sits on, if the job has one.
    pub needs_line: Option<usize>,
    /// The job's `if` condition, if it declares one.
    pub condition: Option<String>,
    /// The line the job's `if` value sits on, if it declares one.
    pub condition_line: Option<usize>,
    /// The job's own environment variables.
    pub env: Vec<EnvEntry>,
    /// The passwords the job's containers declare, for the container and each service.
    pub container_passwords: Vec<ContainerPassword>,
    /// The job's `strategy.matrix`, if it declares one.
    pub matrix: Option<Matrix>,
    /// The reusable workflow this job calls, if it calls one instead of running steps.
    ///
    /// This is the one `uses` that does not sit on a step, and it used to reach no rule at all: a job
    /// calling another workflow was invisible to every check, which is the silent-loss class this
    /// model refuses everywhere else.
    pub uses: Option<String>,
    /// The line the job's `uses` value sits on, if it declares one.
    pub uses_line: Option<usize>,
    /// The job's steps, in document order.
    pub steps: Vec<Step>,
}

/// A parsed workflow document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// The workflow-level permissions block, if it declares one.
    pub permissions: Option<PermissionsBlock>,
    /// The workflow-level environment variables.
    pub env: Vec<EnvEntry>,
    /// The events that trigger the workflow, in document order.
    pub triggers: Vec<Trigger>,
    /// The `on.schedule` entries, in document order.
    ///
    /// Kept beside the triggers rather than inside one, because a schedule entry is a mapping of its
    /// own — `- cron: "…"` — and flattening it into the event's filter list would lose which entry a
    /// value belongs to.
    pub schedules: Vec<ScheduleEntry>,
    /// The inputs the two events that declare them declare, in document order.
    pub inputs: Vec<InputDecl>,
    /// The document's jobs, in document order.
    pub jobs: Vec<Job>,
}

impl Document {
    /// Parse a workflow document.
    pub fn parse(text: &str) -> Result<Document, Error> {
        let mut builder = Builder::default();
        let mut parser = Parser::new_from_str(text);
        parser
            .load(&mut builder, false)
            .map_err(|e| Error::Parse(e.to_string()))?;

        // An alias the document never defines, or a merge that names something unmergeable, would
        // otherwise drop part of the document silently. Refusing is loud; a lost finding is not.
        if builder.dangling_alias {
            return Err(Error::Parse(
                "an alias refers to an anchor this document never defines".to_owned(),
            ));
        }
        if builder.bad_merge {
            return Err(Error::Parse(
                "a merge key (`<<:`) must name a mapping, or a list of mappings".to_owned(),
            ));
        }
        if builder.complex_key {
            return Err(Error::Parse(
                "a key must be a scalar, but this one names a mapping or a list".to_owned(),
            ));
        }

        let root = builder
            .root
            .ok_or_else(|| Error::Parse("empty document".to_string()))?;
        Ok(Document {
            permissions: collect_permissions(root.get(KEY_PERMISSIONS)),
            env: collect_env(root.get(KEY_ENV)),
            triggers: collect_triggers(&root),
            schedules: collect_schedules(&root),
            inputs: collect_inputs(&root),
            jobs: collect_jobs(&root),
        })
    }
}

/// Read an `env:` mapping into its names, each with the line the name sits on.
fn collect_env(node: Option<&Node>) -> Vec<EnvEntry> {
    // A scalar or sequence `env` is an expression or is invalid; neither yields names.
    let Some(Node::Mapping(entries, _)) = node else {
        return Vec::new();
    };
    entries
        .iter()
        .map(|(name, line, _)| EnvEntry {
            name: name.clone(),
            line: *line,
        })
        .collect()
}

/// Read the passwords a job's containers declare: its own container, then each service's.
fn collect_container_passwords(job: &Node) -> Vec<ContainerPassword> {
    let mut found = Vec::new();
    if let Some((value, line)) = container_password(job.get(KEY_CONTAINER)) {
        found.push(ContainerPassword {
            service: None,
            value,
            line,
        });
    }
    if let Some(Node::Mapping(services, _)) = job.get(KEY_SERVICES) {
        for (name, _, service) in services {
            if let Some((value, line)) = container_password(Some(service)) {
                found.push(ContainerPassword {
                    service: Some(name.clone()),
                    value,
                    line,
                });
            }
        }
    }
    found
}

/// The `credentials.password` value inside a container mapping, with the line it sits on.
fn container_password(container: Option<&Node>) -> Option<(String, usize)> {
    let password = container?.get(KEY_CREDENTIALS)?.get(KEY_PASSWORD)?;
    Some((password.as_str()?.to_owned(), password.line()))
}

/// Read the `on.schedule` entries.
///
/// The documented shape is a list of mappings, one per schedule. Anything else is kept as one entry
/// carrying no values, so a rule can say the shape is wrong instead of the schedule vanishing from the
/// document — the silent-loss class this model refuses everywhere else.
fn collect_schedules(root: &Node) -> Vec<ScheduleEntry> {
    let Some(node) = root.get(KEY_ON).and_then(|on| on.get(KEY_SCHEDULE)) else {
        return Vec::new();
    };
    match node {
        Node::Sequence(items, _) => items.iter().map(collect_schedule_entry).collect(),
        other => vec![ScheduleEntry {
            line: other.line(),
            cron: None,
            timezone: None,
        }],
    }
}

/// Read one schedule entry's `cron` and `timezone`, with the line each value sits on.
fn collect_schedule_entry(node: &Node) -> ScheduleEntry {
    ScheduleEntry {
        line: node.line(),
        cron: scalar_of(node, KEY_CRON),
        timezone: scalar_of(node, KEY_TIMEZONE),
    }
}

/// A key's scalar value, with the line it sits on.
fn scalar_of(node: &Node, key: &str) -> Option<ListedValue> {
    let value = node.get(key)?;
    Some(ListedValue {
        value: value.as_str()?.to_owned(),
        line: value.line(),
    })
}

/// Read the inputs the two events declare.
///
/// Both are read into one list, each carrying the event that declared it: the two accept different
/// types and different requirements, so which block a declaration came from is part of the fact.
fn collect_inputs(root: &Node) -> Vec<InputDecl> {
    let Some(on) = root.get(KEY_ON) else {
        return Vec::new();
    };
    let mut inputs = Vec::new();
    for event in [KEY_WORKFLOW_DISPATCH, KEY_WORKFLOW_CALL] {
        let Some(block) = on.get(event) else {
            continue;
        };
        let Some(Node::Mapping(entries, _)) = block.get(KEY_INPUTS) else {
            continue;
        };
        inputs.extend(entries.iter().map(|(id, id_line, node)| InputDecl {
            event,
            id: id.clone(),
            line: *id_line,
            kind: scalar_of(node, KEY_TYPE),
            required: scalar_of(node, KEY_REQUIRED),
            default: scalar_of(node, KEY_DEFAULT),
            options: collect_values(node.get(KEY_OPTIONS)),
        }));
    }
    inputs
}

/// The scalar values of a list, each with the line it sits on.
fn collect_values(node: Option<&Node>) -> Vec<ListedValue> {
    let Some(Node::Sequence(items, _)) = node else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            Some(ListedValue {
                value: item.as_str()?.to_owned(),
                line: item.line(),
            })
        })
        .collect()
}

/// Read the `on:` trigger list: a scalar name, a sequence of names, or a mapping of names to
/// their filter lists.
fn collect_triggers(root: &Node) -> Vec<Trigger> {
    let Some(node) = root.get(KEY_ON) else {
        return Vec::new();
    };
    match node {
        Node::Scalar(name, line) => vec![Trigger {
            name: name.clone(),
            line: *line,
            filters: Vec::new(),
        }],
        Node::Sequence(items, _) => items
            .iter()
            .filter_map(|item| item.as_str().map(|name| (name, item.line())))
            .map(|(name, line)| Trigger {
                name: name.to_owned(),
                line,
                filters: Vec::new(),
            })
            .collect(),
        Node::Mapping(entries, _) => entries
            .iter()
            .map(|(name, key_line, value)| Trigger {
                name: name.clone(),
                line: *key_line,
                filters: collect_filters(value),
            })
            .collect(),
    }
}

/// Read the filter lists declared under one event.
///
/// Every key under the event is kept, with its values: which of them are globs is the rule's
/// business, not the model's.
fn collect_filters(event: &Node) -> Vec<EventFilter> {
    let Node::Mapping(entries, _) = event else {
        return Vec::new();
    };
    entries
        .iter()
        .map(|(name, key_line, value)| EventFilter {
            name: name.clone(),
            line: *key_line,
            values: filter_values(value),
        })
        .collect()
}

/// The values of an event filter: a sequence, or a single scalar treated as one value.
fn filter_values(value: &Node) -> Vec<ListedValue> {
    match value {
        Node::Scalar(text, line) => vec![ListedValue {
            value: text.clone(),
            line: *line,
        }],
        Node::Sequence(items, _) => items
            .iter()
            .filter_map(|item| {
                item.as_str().map(|text| ListedValue {
                    value: text.to_owned(),
                    line: item.line(),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Read a job's `strategy.matrix`, if it declares one.
fn collect_matrix(job: &Node) -> Option<Matrix> {
    let matrix = job.get(KEY_STRATEGY)?.get(KEY_MATRIX)?;
    // A matrix built from an expression has no rows to judge.
    let Node::Mapping(entries, _) = matrix else {
        return None;
    };

    let mut collected = Matrix::default();
    for (name, key_line, value) in entries {
        if name == KEY_INCLUDE {
            collected.include_count = combination_count(value);
            continue;
        }
        if name == KEY_EXCLUDE {
            collected.exclude_count = combination_count(value);
            collected.exclude_line = Some(*key_line);
            continue;
        }
        collected.rows.push(MatrixRow {
            name: name.clone(),
            values: matrix_values(value),
        });
    }
    Some(collected)
}

/// How many combinations an `include` or `exclude` list declares.
fn combination_count(value: &Node) -> usize {
    value.as_seq().map_or(0, <[Node]>::len)
}

/// The scalar values of a matrix row, each with its line.
///
/// A row that is not a sequence — a row built from an expression, say — has no values to judge.
fn matrix_values(value: &Node) -> Vec<ListedValue> {
    let Some(items) = value.as_seq() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            item.as_str().map(|value| ListedValue {
                value: value.to_owned(),
                line: item.line(),
            })
        })
        .collect()
}

/// Read a permissions block: a bare token, or one entry per scope.
fn collect_permissions(node: Option<&Node>) -> Option<PermissionsBlock> {
    match node? {
        Node::Scalar(value, line) => Some(PermissionsBlock::Token {
            value: value.clone(),
            line: *line,
        }),
        Node::Mapping(entries, line) => Some(PermissionsBlock::Scopes {
            entries: entries
                .iter()
                .filter_map(|(scope, _, value)| {
                    value.as_str().map(|access| PermissionEntry {
                        scope: scope.clone(),
                        access: access.to_owned(),
                        line: value.line(),
                    })
                })
                .collect(),
            line: *line,
        }),
        Node::Sequence(_, _) => None,
    }
}

/// Read every job from the document's `jobs` mapping, in document order.
///
/// A job's id is its key, so each job also carries the line that key is written on — the line a
/// finding about the job must point at.
fn collect_jobs(root: &Node) -> Vec<Job> {
    let Some(Node::Mapping(entries, _)) = root.get(KEY_JOBS) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|(id, id_line, node)| {
            let Node::Mapping(fields, _) = node else {
                return None;
            };
            let runs_on = node.get(KEY_RUNS_ON);
            let needs = node.get(KEY_NEEDS);
            let condition = node.get(KEY_IF);
            let uses = node.get(KEY_USES);
            Some(Job {
                id: id.clone(),
                line: *id_line,
                timeout_minutes: node
                    .get(KEY_TIMEOUT)
                    .and_then(Node::as_str)
                    .and_then(|s| s.parse().ok()),
                runs_on: collect_labels(runs_on),
                runs_on_line: runs_on.map(Node::line),
                permissions: collect_permissions(node.get(KEY_PERMISSIONS)),
                needs: collect_labels(needs),
                needs_line: needs.map(Node::line),
                condition: condition.and_then(Node::as_str).map(str::to_owned),
                condition_line: condition.map(Node::line),
                env: collect_env(node.get(KEY_ENV)),
                container_passwords: collect_container_passwords(node),
                matrix: collect_matrix(node),
                uses: uses.and_then(Node::as_str).map(str::to_owned),
                uses_line: uses.map(Node::line),
                steps: fields
                    .iter()
                    .find(|(k, _, _)| k == KEY_STEPS)
                    .map(|(_, _, v)| collect_steps(v))
                    .unwrap_or_default(),
            })
        })
        .collect()
}

/// Gather the labels from a `runs-on` value: one for a scalar, many for a sequence.
fn collect_labels(node: Option<&Node>) -> Vec<String> {
    match node {
        Some(Node::Scalar(value, _)) => vec![value.clone()],
        Some(Node::Sequence(items, _)) => items
            .iter()
            .filter_map(Node::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// Read a `steps` sequence into the model, keeping each step's own lines.
fn collect_steps(steps: &Node) -> Vec<Step> {
    steps
        .as_seq()
        .unwrap_or_default()
        .iter()
        .map(|node| {
            let uses_node = node.get(KEY_USES);
            let id_node = node.get(KEY_ID);
            let run_node = node.get(KEY_RUN);
            let condition = node.get(KEY_IF);
            let shell = node.get(KEY_SHELL);
            Step {
                line: node.line(),
                uses: uses_node.and_then(Node::as_str).map(str::to_owned),
                uses_line: uses_node.map(Node::line),
                id: id_node.and_then(Node::as_str).map(str::to_owned),
                id_line: id_node.map(Node::line),
                run: run_node.and_then(Node::as_str).map(str::to_owned),
                run_line: run_node.map(Node::line),
                condition: condition.and_then(Node::as_str).map(str::to_owned),
                condition_line: condition.map(Node::line),
                shell: shell.and_then(Node::as_str).map(str::to_owned),
                shell_line: shell.map(Node::line),
                env: collect_env(node.get(KEY_ENV)),
                name: node.get(KEY_NAME).and_then(Node::as_str).map(str::to_owned),
            }
        })
        .collect()
}
