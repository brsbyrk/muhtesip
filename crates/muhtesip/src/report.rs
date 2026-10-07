//! The report: what a run produced, and the encodings of it.
//!
//! There is **one derivation and three encodings**. Filtering (config ignores and the document's
//! own directives) happens once, before any encoding, and all of them read the resulting
//! [`SourceReport`]. The machine encoding carries, for every finding, the exact line the plain
//! encoding prints (`"text"`), so a test can prove the two agree by reassembling the text output
//! from the envelope rather than by trusting that they do.
//!
//! The machine encoding is a strict **superset** of the plain one: it adds the envelope version and
//! the per-finding fields the plain line only embeds. Reassembly therefore uses the `"text"` fields,
//! and `tests/report.rs` asserts it byte for byte over a non-empty document. The annotation encoding
//! is a third reading of the same envelope, for a runner that parses log commands.

use crate::data::{DEFAULT_ANNOTATION_LEVEL, annotation_level, sarif_level};
use crate::rule::{Finding, RuleMeta, Severity};

/// The envelope version. A field added later is a new envelope, not a silently changed shape.
pub const ENVELOPE: &str = "muhtesip/1";

/// The SARIF version this writer emits.
const SARIF_VERSION: &str = "2.1.0";

/// The schema the emitted SARIF document claims, so a consumer can validate it.
const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

/// One annotation, in the runner's log-command syntax: `::{level} {properties}::{message}`.
///
/// The message uses the data escaping rules and each property value the property rules, taken from
/// the runner's own implementation (`actions/toolkit`, `packages/core/src/command.ts`) rather than
/// from prose: `%`, CR and LF are escaped everywhere, and a property value also escapes `:` and `,`,
/// which separate the properties. A message carrying a newline would otherwise end the command and
/// the remainder would be read as a new log line.
fn annotation(
    level: &str,
    file: Option<&str>,
    line: Option<usize>,
    title: Option<&str>,
    message: &str,
) -> String {
    let mut properties = Vec::new();
    if let Some(file) = file {
        properties.push(format!("file={}", escape_property(file)));
    }
    if let Some(line) = line {
        properties.push(format!("line={line}"));
    }
    if let Some(title) = title {
        properties.push(format!("title={}", escape_property(title)));
    }

    let message = escape_data(message);
    if properties.is_empty() {
        format!("::{level}::{message}")
    } else {
        format!("::{level} {}::{message}", properties.join(","))
    }
}

/// Escape a command's message body.
fn escape_data(value: &str) -> String {
    escape(value, false)
}

/// Escape a command's property value, which additionally may not contain `:` or `,`.
fn escape_property(value: &str) -> String {
    escape(value, true)
}

/// The escaping both forms share.
fn escape(value: &str, property: bool) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            ':' if property => out.push_str("%3A"),
            ',' if property => out.push_str("%2C"),
            c => out.push(c),
        }
    }
    out
}

/// What the `means` field says on every refusal, so a consumer never has to infer it.
const REFUSAL_MEANS: &str = "no findings were produced for this source";

/// Why a source, or a whole run, produced no report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The refusal's name. Declared at the site that refuses, never parsed out of the message.
    pub name: &'static str,
    /// What went wrong, in the words the plain renderer puts on stderr.
    pub message: String,
}

/// What happened to one source.
#[derive(Debug)]
pub enum Outcome<'a> {
    /// The source was linted. These are the findings that survived filtering.
    Linted(Vec<&'a Finding>),
    /// The source could not be linted.
    Refused(Refusal),
}

/// The outcome for one source, and the name it is reported under.
#[derive(Debug)]
pub struct SourceReport<'a> {
    /// How the source is named in both encodings.
    pub source: &'a str,
    /// What happened to it.
    pub outcome: Outcome<'a>,
}

impl<'a> SourceReport<'a> {
    /// A source that was linted, with the findings that survived filtering.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::lint;
    /// use muhtesip::report::SourceReport;
    ///
    /// let findings =
    ///     lint("on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n").unwrap();
    /// let report = SourceReport::linted("ci.yaml", findings.iter().collect());
    ///
    /// assert_eq!(report.source, "ci.yaml");
    /// assert_eq!(report.lines().len(), 1);
    /// ```
    pub fn linted(source: &'a str, findings: Vec<&'a Finding>) -> Self {
        Self {
            source,
            outcome: Outcome::Linted(findings),
        }
    }

    /// A source that could not be linted.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::report::{Refusal, SourceReport};
    ///
    /// let report = SourceReport::refused("broken.yaml", Refusal {
    ///     name: "PARSE_ERROR",
    ///     message: "could not parse workflow: bad".to_owned(),
    /// });
    ///
    /// // A refusal contributes no plain lines: its message belongs on stderr.
    /// assert!(report.lines().is_empty());
    /// ```
    pub fn refused(source: &'a str, refusal: Refusal) -> Self {
        Self {
            source,
            outcome: Outcome::Refused(refusal),
        }
    }

    /// The plain lines for this source, exactly as the text renderer prints them.
    ///
    /// A refused source contributes no lines: its message belongs on stderr, which is where the
    /// plain renderer has always put it.
    pub fn lines(&self) -> Vec<String> {
        self.lines_styled(Style::Plain)
    }

    /// This source's plain lines, colored by `style`.
    fn lines_styled(&self, style: Style) -> Vec<String> {
        match &self.outcome {
            Outcome::Refused(_) => Vec::new(),
            Outcome::Linted(findings) => findings
                .iter()
                .map(|f| finding_line(self.source, f, style))
                .collect(),
        }
    }

    /// This source's annotations: one per finding, or one naming the refusal.
    fn annotation_lines(&self) -> Vec<String> {
        match &self.outcome {
            Outcome::Linted(findings) => findings
                .iter()
                .map(|finding| {
                    annotation(
                        annotation_level(finding.severity.name())
                            .unwrap_or(DEFAULT_ANNOTATION_LEVEL),
                        Some(self.source),
                        Some(finding.line),
                        Some(finding.rule),
                        &finding.message,
                    )
                })
                .collect(),
            Outcome::Refused(refusal) => vec![annotation(
                DEFAULT_ANNOTATION_LEVEL,
                Some(self.source),
                None,
                Some(refusal.name),
                &refusal.message,
            )],
        }
    }

    /// This source's block of the envelope.
    fn json(&self, level: usize) -> String {
        let pad = indent(level);
        let inner = indent(level + 1);
        let mut out = format!(
            "{pad}{{\n{inner}\"source\": {},\n",
            json_string(self.source)
        );

        match &self.outcome {
            Outcome::Refused(refusal) => {
                out.push_str(&format!(
                    "{inner}\"refusal\": {{\"name\": {}, \"message\": {}, \"means\": {}}},\n",
                    json_string(refusal.name),
                    json_string(&refusal.message),
                    json_string(REFUSAL_MEANS),
                ));
                out.push_str(&format!("{inner}\"findings\": []\n"));
            }
            Outcome::Linted(findings) => {
                out.push_str(&format!("{inner}\"findings\": ["));
                if findings.is_empty() {
                    out.push_str("]\n");
                } else {
                    out.push('\n');
                    let entries: Vec<String> = findings
                        .iter()
                        .map(|f| finding_json(self.source, f, level + 2))
                        .collect();
                    out.push_str(&entries.join(",\n"));
                    out.push('\n');
                    out.push_str(&format!("{inner}]\n"));
                }
            }
        }

        out.push_str(&format!("{pad}}}"));
        out
    }
}

/// A whole run: every source's outcome, and a refusal if the run could not start at all.
#[derive(Debug)]
pub struct Envelope<'a> {
    /// Why the run produced nothing, when the failure was not tied to one source (bad arguments,
    /// an unusable configuration). `None` when the run started.
    pub refusal: Option<Refusal>,
    /// One entry per source, in the order they were given.
    pub sources: Vec<SourceReport<'a>>,
}

impl<'a> Envelope<'a> {
    /// An envelope over the outcomes of a run that started.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::lint;
    /// use muhtesip::report::{Envelope, SourceReport};
    ///
    /// let findings =
    ///     lint("on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n").unwrap();
    /// let envelope = Envelope::new(vec![SourceReport::linted("ci.yaml", findings.iter().collect())]);
    ///
    /// assert!(envelope.has_findings());
    /// assert!(!envelope.has_refusal());
    /// ```
    pub fn new(sources: Vec<SourceReport<'a>>) -> Self {
        Self {
            refusal: None,
            sources,
        }
    }

    /// An envelope for a run that could not start: a named refusal, and no sources.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::report::Envelope;
    ///
    /// let envelope = Envelope::refused("CONFIG_ERROR", "invalid configuration: nope".to_owned());
    ///
    /// assert!(envelope.has_refusal());
    /// assert!(envelope.json().contains("CONFIG_ERROR"));
    /// ```
    pub fn refused(name: &'static str, message: String) -> Envelope<'static> {
        Envelope {
            refusal: Some(Refusal { name, message }),
            sources: Vec::new(),
        }
    }

    /// Whether any source, or the run itself, was refused.
    pub fn has_refusal(&self) -> bool {
        self.refusal.is_some()
            || self
                .sources
                .iter()
                .any(|s| matches!(s.outcome, Outcome::Refused(_)))
    }

    /// Whether any source produced a finding, whatever its severity.
    pub fn has_findings(&self) -> bool {
        self.sources.iter().any(|s| !s.lines().is_empty())
    }

    /// Whether any finding is **at least as serious as** `threshold`.
    ///
    /// This is what the exit code should be derived from, rather than the bare presence of findings:
    /// a `note` is advice, and a run that fails the build on advice is a run that gets muted. The
    /// caller picks the threshold; the library only knows severity. A refusal is not a finding — it
    /// is reported through [`Envelope::has_refusal`], because "could not read this" is a different
    /// kind of failure from "this file has a problem".
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{lint, Severity};
    /// use muhtesip::report::{Envelope, SourceReport};
    ///
    /// let findings =
    ///     lint("on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n").unwrap();
    /// let envelope = Envelope::new(vec![SourceReport::linted("ci.yaml", findings.iter().collect())]);
    ///
    /// // The only finding is a note, so the default (warning) threshold passes it.
    /// assert!(!envelope.fails(Severity::Warning));
    /// assert!(envelope.fails(Severity::Note));
    /// ```
    pub fn fails(&self, threshold: Severity) -> bool {
        self.sources.iter().any(|source| match &source.outcome {
            Outcome::Linted(findings) => findings.iter().any(|f| f.severity >= threshold),
            Outcome::Refused(_) => false,
        })
    }

    /// The plain rendering: one line per finding, over every source, in order.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::lint;
    /// use muhtesip::report::{Envelope, SourceReport};
    ///
    /// let findings =
    ///     lint("on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n").unwrap();
    /// let text =
    ///     Envelope::new(vec![SourceReport::linted("ci.yaml", findings.iter().collect())]).text();
    ///
    /// assert_eq!(
    ///     text,
    ///     "ci.yaml:3: note: job 'build' has no timeout-minutes bound [missing-timeout]\n",
    /// );
    /// ```
    pub fn text(&self) -> String {
        self.text_styled(Style::Plain)
    }

    /// The plain rendering, colored by `style`.
    ///
    /// Color is the only difference: the same lines, in the same order, byte for byte when the style
    /// is plain. Whether the destination is a terminal or a pipe is the caller's business, so the
    /// rendering does not have to guess.
    pub fn text_styled(&self, style: Style) -> String {
        let mut out = String::new();
        for source in &self.sources {
            for line in source.lines_styled(style) {
                out.push_str(&line);
                out.push('\n');
            }
        }
        out
    }

    /// The annotation rendering: one workflow command per finding, and one per refusal.
    ///
    /// The third reading of the same envelope — nothing is derived twice. A refusal keeps its name
    /// in the `title`, so a machine reading annotations can still tell a refusal from a finding.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::lint;
    /// use muhtesip::report::{Envelope, SourceReport};
    ///
    /// let findings =
    ///     lint("on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n").unwrap();
    /// let annotations =
    ///     Envelope::new(vec![SourceReport::linted("ci.yaml", findings.iter().collect())])
    ///         .annotations();
    ///
    /// assert!(annotations.starts_with("::notice "), "{annotations}");
    /// assert!(annotations.contains("file=ci.yaml"), "{annotations}");
    /// assert!(annotations.contains("title=missing-timeout"), "{annotations}");
    /// ```
    pub fn annotations(&self) -> String {
        let mut out = String::new();
        if let Some(refusal) = &self.refusal {
            out.push_str(&annotation(
                DEFAULT_ANNOTATION_LEVEL,
                None,
                None,
                Some(refusal.name),
                &refusal.message,
            ));
            out.push('\n');
        }
        for source in &self.sources {
            for line in source.annotation_lines() {
                out.push_str(&line);
                out.push('\n');
            }
        }
        out
    }

    /// The machine rendering.
    ///
    /// A refused run carries its refusal at the top level and `sources` stays empty; a run that
    /// started lists one block per source, each holding either `findings` or its own `refusal`.
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::lint;
    /// use muhtesip::report::{Envelope, SourceReport};
    ///
    /// let findings =
    ///     lint("on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n").unwrap();
    /// let json = Envelope::new(vec![SourceReport::linted("ci.yaml", findings.iter().collect())])
    ///     .json();
    ///
    /// // Parsed with a real parser, not compared against a string this same code wrote.
    /// let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    /// assert_eq!(value["envelope"], "muhtesip/1");
    /// assert_eq!(value["sources"][0]["findings"][0]["rule"], "missing-timeout");
    /// ```
    pub fn json(&self) -> String {
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"envelope\": {},\n", json_string(ENVELOPE)));

        match &self.refusal {
            None => out.push_str("  \"refusal\": null,\n"),
            Some(refusal) => out.push_str(&format!(
                "  \"refusal\": {{\"name\": {}, \"message\": {}, \"means\": {}}},\n",
                json_string(refusal.name),
                json_string(&refusal.message),
                json_string(REFUSAL_MEANS),
            )),
        }

        out.push_str("  \"sources\": [");
        if self.sources.is_empty() {
            out.push_str("]\n}\n");
            return out;
        }
        let blocks: Vec<String> = self.sources.iter().map(|s| s.json(2)).collect();
        out.push('\n');
        out.push_str(&blocks.join(",\n"));
        out.push_str("\n  ]\n}\n");
        out
    }

    /// The SARIF 2.1.0 rendering, for a code-scanning consumer.
    ///
    /// The rule table comes from the caller because the record does not own the registry. Only the
    /// rules the results actually cite are described, in sorted order, so two runs over the same
    /// input produce the same bytes.
    ///
    /// A refusal is not a result: it becomes an invocation notification and makes the invocation
    /// unsuccessful, so "the tool could not read this file" can never read as "this file is clean".
    ///
    /// # Examples
    ///
    /// ```
    /// use muhtesip::{lint, Registry};
    /// use muhtesip::report::{Envelope, SourceReport};
    ///
    /// let findings =
    ///     lint("on: push\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps: []\n").unwrap();
    /// let sarif = Envelope::new(vec![SourceReport::linted("ci.yaml", findings.iter().collect())])
    ///     .sarif(&Registry::all().metas());
    ///
    /// let value: serde_json::Value = serde_json::from_str(&sarif).unwrap();
    /// assert_eq!(value["version"], "2.1.0");
    /// assert_eq!(value["runs"][0]["tool"]["driver"]["name"], "muhtesip");
    /// assert_eq!(value["runs"][0]["results"][0]["ruleId"], "missing-timeout");
    /// ```
    pub fn sarif(&self, rules: &[&RuleMeta]) -> String {
        let used = self.used_rules();
        let descriptors: Vec<String> = used.iter().map(|id| sarif_rule(id, rules)).collect();
        let results: Vec<String> = self
            .sources
            .iter()
            .flat_map(|source| match &source.outcome {
                Outcome::Linted(findings) => findings
                    .iter()
                    .map(|finding| sarif_result(source.source, finding, &used))
                    .collect::<Vec<String>>(),
                Outcome::Refused(_) => Vec::new(),
            })
            .collect();
        let notifications: Vec<String> = self
            .refusals()
            .iter()
            .map(|refusal| sarif_notification(refusal))
            .collect();

        let mut invocation = vec![format!(
            "\"executionSuccessful\": {}",
            notifications.is_empty()
        )];
        if !notifications.is_empty() {
            invocation.push(format!(
                "\"toolExecutionNotifications\": {}",
                json_array(&notifications, 6)
            ));
        }

        format!(
            "{{\n  \"$schema\": {schema},\n  \"version\": {version},\n  \"runs\": [\n    {{\n      \"tool\": {{\n        \"driver\": {{\n          \"name\": {name},\n          \"informationUri\": {repo},\n          \"version\": {tool_version},\n          \"rules\": {rules}\n        }}\n      }},\n      \"results\": {results},\n      \"invocations\": [\n        {{\n          {invocation}\n        }}\n      ]\n    }}\n  ]\n}}\n",
            schema = json_string(SARIF_SCHEMA),
            version = json_string(SARIF_VERSION),
            name = json_string(env!("CARGO_PKG_NAME")),
            repo = json_string(env!("CARGO_PKG_REPOSITORY")),
            tool_version = json_string(env!("CARGO_PKG_VERSION")),
            rules = json_array(&descriptors, 6),
            results = json_array(&results, 4),
            invocation = invocation.join(",\n          "),
        )
    }

    /// The rule ids the results cite, de-duplicated and sorted.
    fn used_rules(&self) -> Vec<&str> {
        let mut used: Vec<&str> = self
            .sources
            .iter()
            .flat_map(|source| match &source.outcome {
                Outcome::Linted(findings) => findings.iter().map(|f| f.rule).collect::<Vec<&str>>(),
                Outcome::Refused(_) => Vec::new(),
            })
            .collect();
        used.sort_unstable();
        used.dedup();
        used
    }

    /// Every refusal this record carries, the run-level one first.
    fn refusals(&self) -> Vec<&Refusal> {
        let mut refusals: Vec<&Refusal> = self.refusal.iter().collect();
        refusals.extend(
            self.sources
                .iter()
                .filter_map(|source| match &source.outcome {
                    Outcome::Refused(refusal) => Some(refusal),
                    Outcome::Linted(_) => None,
                }),
        );
        refusals
    }
}

/// Whether a rendering carries terminal color.
///
/// The library never reads the environment: a caller decides, and passes the answer in. That keeps
/// the same envelope printable to a terminal and to a pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// No escapes: what a pipe, a file, or a machine format gets.
    Plain,
    /// Severities wrapped in SGR sequences, for a person reading a terminal.
    Ansi,
}

/// Wrap `text` in an SGR sequence when the style asks for color, and return it unchanged otherwise.
///
/// Coloring unconditionally at the call site is therefore safe: the plain style is what keeps a piped
/// run producing the bytes it always did.
pub fn paint(text: &str, sgr: &str, style: Style) -> String {
    match style {
        Style::Plain => text.to_string(),
        Style::Ansi => format!("\u{1b}[{sgr}m{text}\u{1b}[0m"),
    }
}

/// The terminal code for a severity: red for a defect, yellow for a warning, dim for advice.
fn severity_sgr(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "1;31",
        Severity::Warning => "33",
        Severity::Note => "2",
    }
}

/// The plain line for one finding: `source:line: severity: message [rule]`.
///
/// Only the severity carries color. The path, the line number and the message are what a person
/// selects and copies out of a terminal, and an escape sequence in the middle of that is noise.
fn finding_line(source: &str, finding: &Finding, style: Style) -> String {
    format!(
        "{source}:{}: {}: {} [{}]",
        finding.line,
        paint(
            &finding.severity.to_string(),
            severity_sgr(finding.severity),
            style
        ),
        finding.message,
        finding.rule
    )
}

/// A JSON array, one item per line at `level`, or `[]` when there is nothing to list.
fn json_array(items: &[String], level: usize) -> String {
    if items.is_empty() {
        return "[]".to_owned();
    }
    let inner = indent(level);
    let closing = indent(level.saturating_sub(1));
    format!(
        "[\n{inner}{}\n{closing}]",
        items.join(&format!(",\n{inner}"))
    )
}

/// One SARIF reporting descriptor — a rule the results cite — on one line.
///
/// A rule absent from the supplied table is still described, by id alone: the result's `ruleId` must
/// never point at a rule the document does not declare.
fn sarif_rule(id: &str, rules: &[&RuleMeta]) -> String {
    let mut fields = vec![format!("\"id\": {}", json_string(id))];
    if let Some(meta) = rules.iter().find(|meta| meta.id == id) {
        fields.push(format!(
            "\"shortDescription\": {{\"text\": {}}}",
            json_string(meta.description)
        ));
        fields.push(format!("\"helpUri\": {}", json_string(&meta.docs_url())));
        fields.push(format!(
            "\"defaultConfiguration\": {{\"level\": {}}}",
            json_string(sarif_level(meta.severity.name()).unwrap_or(DEFAULT_ANNOTATION_LEVEL))
        ));
    }
    format!("{{{}}}", fields.join(", "))
}

/// One SARIF result, on one line.
fn sarif_result(source: &str, finding: &Finding, used: &[&str]) -> String {
    let mut fields = vec![format!("\"ruleId\": {}", json_string(finding.rule))];
    if let Some(index) = used.iter().position(|id| *id == finding.rule) {
        fields.push(format!("\"ruleIndex\": {index}"));
    }
    fields.push(format!(
        "\"level\": {}",
        json_string(sarif_level(finding.severity.name()).unwrap_or(DEFAULT_ANNOTATION_LEVEL))
    ));
    fields.push(format!(
        "\"message\": {{\"text\": {}}}",
        json_string(&finding.message)
    ));
    fields.push(format!(
        "\"locations\": [{{\"physicalLocation\": {{\"artifactLocation\": {{\"uri\": {}}}, \"region\": {{\"startLine\": {}}}}}}}]",
        json_string(&uri(source)),
        finding.line
    ));
    format!("{{{}}}", fields.join(", "))
}

/// One invocation notification naming a refusal, on one line.
fn sarif_notification(refusal: &Refusal) -> String {
    format!(
        "{{\"level\": {}, \"descriptor\": {{\"id\": {}}}, \"message\": {{\"text\": {}}}}}",
        json_string(DEFAULT_ANNOTATION_LEVEL),
        json_string(refusal.name),
        json_string(&refusal.message)
    )
}

/// A path as the `uri` a SARIF location expects.
///
/// Only the characters that would break a URI are escaped; the path is otherwise passed through, so
/// a repository-relative path stays repository-relative. Turning it into an absolute `file:` URI
/// would make the report depend on the machine that produced it.
fn uri(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            '%' => out.push_str("%25"),
            ' ' => out.push_str("%20"),
            '\\' => out.push_str("%5C"),
            c if c.is_ascii_control() => out.push_str(&format!("%{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The envelope entry for one finding, one line of JSON.
fn finding_json(source: &str, finding: &Finding, level: usize) -> String {
    format!(
        "{}{{\"file\": {}, \"line\": {}, \"rule\": {}, \"severity\": {}, \"message\": {}, \"text\": {}}}",
        indent(level),
        json_string(source),
        finding.line,
        json_string(finding.rule),
        json_string(finding.severity.name()),
        json_string(&finding.message),
        // The JSON carries the plain line as `text`: a machine reading it gets the bytes, never an
        // escape sequence, whatever the terminal the same run happens to be printing to.
        json_string(&finding_line(source, finding, Style::Plain)),
    )
}

/// `level` levels of two-space indentation.
fn indent(level: usize) -> String {
    "  ".repeat(level)
}

/// A JSON string literal.
///
/// Hand-written rather than pulled from a serializer: the envelope is small and fixed, and the only
/// hazard is escaping. `tests/report.rs` parses every produced envelope with a real JSON parser (a
/// dev-dependency, so the shipped binary gains nothing) and drives the escaping cases directly.
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::{annotation, escape_data, escape_property, json_string};
    use crate::data::annotation_level;
    use crate::rule::Severity;

    /// Every string here would break a JSON document if it were copied in raw.
    const NASTY: &[&str] = &[
        "plain",
        "say \"hi\"",
        "back\\slash",
        "a\nb",
        "a\rb",
        "a\tb",
        "\u{8}\u{c}",
        "\u{0}\u{1}\u{1f}",
        "café — ünïcode ✓",
        "-",
    ];

    #[test]
    fn escapes_the_characters_that_would_break_a_document() {
        assert_eq!(json_string("plain"), "\"plain\"");
        assert_eq!(json_string("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(json_string("a\\b"), "\"a\\\\b\"");
        assert_eq!(json_string("a\nb"), "\"a\\nb\"");
        assert_eq!(json_string("a\tb"), "\"a\\tb\"");
        assert_eq!(json_string("\u{1}"), "\"\\u0001\"");
    }

    #[test]
    fn every_escaped_string_parses_back_to_its_input() {
        for raw in NASTY {
            let encoded = json_string(raw);
            let parsed: String =
                serde_json::from_str(&encoded).expect("every escaped string is valid JSON");
            assert_eq!(&parsed, raw, "round trip for {raw:?}");
        }
    }

    #[test]
    fn non_ascii_passes_through_unchanged() {
        assert_eq!(json_string("café — ünïcode ✓"), "\"café — ünïcode ✓\"");
    }

    #[test]
    fn an_annotation_carries_the_file_the_line_and_the_rule() {
        let line = annotation(
            "warning",
            Some("ci.yaml"),
            Some(10),
            Some("unpinned-action"),
            "action 'x@v4' is not pinned",
        );
        assert_eq!(
            line,
            "::warning file=ci.yaml,line=10,title=unpinned-action::action 'x@v4' is not pinned"
        );
    }

    #[test]
    fn a_message_may_hold_a_comma_and_a_colon_but_a_property_may_not() {
        // Inside the property list those characters are separators; in the message they are text.
        let line = annotation("error", Some("a,b:c.yaml"), None, None, "bash, pwsh: sh");
        assert_eq!(line, "::error file=a%2Cb%3Ac.yaml::bash, pwsh: sh");
    }

    #[test]
    fn data_escaping_covers_what_would_end_the_command() {
        assert_eq!(escape_data("100%"), "100%25");
        assert_eq!(escape_data("a\nb"), "a%0Ab");
        assert_eq!(escape_data("a\rb"), "a%0Db");
        assert_eq!(escape_data("café ✓"), "café ✓", "only the three characters");
    }

    #[test]
    fn property_escaping_adds_the_separators() {
        assert_eq!(escape_property("100%"), "100%25");
        assert_eq!(escape_property("a,b:c"), "a%2Cb%3Ac");
    }

    #[test]
    fn a_run_level_annotation_leaves_no_empty_property() {
        assert_eq!(
            annotation("error", None, None, None, "nope"),
            "::error::nope"
        );
    }

    #[test]
    fn every_severity_maps_to_an_annotation_level() {
        // A missing table entry would silently drop a finding's level, so the table must be total.
        for severity in [Severity::Error, Severity::Warning, Severity::Note] {
            assert!(
                annotation_level(severity.name()).is_some(),
                "no annotation level for {}",
                severity.name()
            );
        }
        assert_eq!(annotation_level(Severity::Note.name()), Some("notice"));
    }
}
