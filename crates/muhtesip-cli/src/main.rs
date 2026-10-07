//! The `muhtesip` command line: lint workflow files and report findings.
//!
//! This file is deliberately thin. Every decision worth testing lives in the library; here we
//! only read a source, apply an optional configuration, call the linter, build one `Envelope`,
//! and render it. Both formats read that same envelope — nothing is derived twice, and the
//! exit code comes from the envelope rather than from a second pass.

use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use muhtesip::report::{Envelope, Outcome, Refusal, SourceReport, Style, paint};
use muhtesip::{Config, Finding, Registry, Severity};

/// Exit code when every input was clean.
const EXIT_CLEAN: u8 = 0;
/// Exit code when at least one finding was reported.
const EXIT_FINDINGS: u8 = 1;
/// Exit code for a usage, read, parse, or configuration failure.
const EXIT_ERROR: u8 = 2;

/// The refusal name for a source that could not be read.
const READ_REFUSAL: &str = "READ_ERROR";
/// The refusal name for a source that could not be parsed or linted.
const PARSE_REFUSAL: &str = "PARSE_ERROR";
/// The refusal name for a configuration, or a command line, that could not be used.
const CONFIG_REFUSAL: &str = "CONFIG_ERROR";

/// The label used for a source read from standard input.
const STDIN_LABEL: &str = "<stdin>";
/// The argument that selects standard input.
const STDIN_ARG: &str = "-";
/// The flag that names a configuration file.
const CONFIG_FLAG: &str = "--config";
/// The flag that selects the output format.
const FORMAT_FLAG: &str = "--format";
/// The plain, human-facing format.
const FORMAT_PLAIN: &str = "plain";
/// The machine-readable format.
const FORMAT_JSON: &str = "json";
/// The format whose lines the runner turns into annotations on the diff.
const FORMAT_ANNOTATIONS: &str = "annotations";
/// The code-scanning format.
const FORMAT_SARIF: &str = "sarif";
/// The flag naming the severity a finding must reach for the run to fail.
const FAIL_ON_FLAG: &str = "--fail-on";
/// The severity a run fails on when the caller does not say.
///
/// `Warning`, not `Note`: a note is advice, and a linter that fails a build on advice is a linter that
/// gets muted. Measured on the benchmark corpus, 539 of muhtesip's 770 findings on real workflow files
/// are notes — with `Note` as the default, almost every real repository would fail CI on advisory
/// output alone.
const DEFAULT_FAIL_ON: Severity = Severity::Warning;
/// The severity names, for an error message. A test asserts every severity appears here, so adding
/// one to the enum cannot leave a message that omits it.
const SEVERITY_NAMES: &str = "error, warning, or note";
/// The prefix that marks an option (as opposed to a source or stdin).
const OPTION_PREFIX: &str = "--";
/// The configuration file discovered by walking up from the working directory.
const CONFIG_FILE: &str = ".muhtesip.yaml";
/// The flag that writes a starter configuration and stops.
const INIT_FLAG: &str = "--init-config";
/// The flag that prints the version and stops.
const VERSION_FLAG: &str = "--version";
/// The flag that prints the usage and stops.
const HELP_FLAG: &str = "--help";
/// The usage text: what a run does, in the order the arguments are read.
const USAGE: &str = "\
muhtesip — lint CI workflow files

usage:
  muhtesip <file>...                  lint files, one finding per line
  muhtesip <dir>...                   a directory expands to its workflow files
  muhtesip < file.yaml                or no arguments / '-' to read standard input
  muhtesip --format json <file>...    plain (default), json, annotations, sarif
  muhtesip --fail-on error <file>...  exit 1 only at or above this severity (default: warning)
  muhtesip --config <file> <file>...  per-project ignores and rule settings
  muhtesip --init-config              write a starter .muhtesip.yaml and stop
  muhtesip --version                  print the version and stop
  muhtesip --help                     print this and stop

exit codes: 0 clean, 1 a finding at or above the threshold, 2 a read, parse, or usage failure";
/// The directory holding a project's workflow files.
const WORKFLOWS_DIR: &str = ".github/workflows";
/// The extensions a workflow file may carry.
const WORKFLOW_EXTENSIONS: &[&str] = &["yml", "yaml"];
/// The marker directory that stops the upward search.
const REPO_MARKER: &str = ".git";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let options = match Options::parse(&args) {
        Ok(options) => options,
        Err(message) => {
            // A command line we cannot read cannot produce a report: when we do not know what was
            // asked for, no envelope is trustworthy, so the message goes to stderr alone.
            error_line(&message);
            return ExitCode::from(EXIT_ERROR);
        }
    };

    // An action, not a lint run. `--version` and `--help` answer whatever else is on the command line,
    // because there is nothing to guess about the intent; `--init-config` writes a file, so `parse`
    // refuses the options that would otherwise change what it does.
    match options.action {
        Action::Version => {
            println!("muhtesip {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::from(EXIT_CLEAN);
        }
        Action::Help => {
            println!("{USAGE}");
            return ExitCode::from(EXIT_CLEAN);
        }
        Action::InitConfig => return init_config(),
        Action::Lint => {}
    }

    // Standard input is the default only when no argument was given at all. A directory that
    // matched no workflow files must NOT fall back to reading stdin, or an empty directory would
    // silently start waiting on a pipe.
    let requested = if options.sources.is_empty() {
        vec![STDIN_ARG.to_string()]
    } else {
        options.sources.clone()
    };
    let sources = expand(&requested);

    let config = match load_config(options.config.as_deref()) {
        Ok(config) => config,
        Err(error) => return refuse(options.format, CONFIG_REFUSAL, error.to_string()),
    };
    let registry = match build_registry(&config) {
        Ok(registry) => registry,
        Err(error) => return refuse(options.format, CONFIG_REFUSAL, error.to_string()),
    };

    // Read and lint every source. The findings are owned here and borrowed by the reports below;
    // each source also keeps the directives its own text declared.
    let mut outcomes: Vec<(usize, Result<LintedSource, Refusal>)> = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let outcome = match read_source(source) {
            Ok(text) => match muhtesip::lint_str(&text, &registry) {
                Ok(findings) => Ok(LintedSource {
                    directives: muhtesip::directives::Directives::parse(&text),
                    findings,
                }),
                Err(error) => Err(Refusal {
                    name: PARSE_REFUSAL,
                    message: error.to_string(),
                }),
            },
            Err(error) => Err(Refusal {
                name: READ_REFUSAL,
                message: error.to_string(),
            }),
        };
        outcomes.push((index, outcome));
    }

    // ONE DERIVATION: filter here, once, and let both encodings read the result.
    let labels: Vec<String> = sources.iter().map(|s| label_for(s).to_owned()).collect();
    let reports: Vec<SourceReport<'_>> = outcomes
        .iter()
        .map(|(index, outcome)| {
            let label = labels[*index].as_str();
            match outcome {
                Ok(linted) => {
                    // Suppression has exactly two sources: the path-scoped configuration, and the
                    // document's own directives. Both are predicates, applied once, right here.
                    let kept: Vec<&Finding> = linted
                        .findings
                        .iter()
                        .filter(|finding| {
                            !config.is_ignored(label, finding)
                                && !linted.directives.ignores(finding.line, finding.rule)
                        })
                        .collect();
                    SourceReport::linted(label, kept)
                }
                Err(refusal) => SourceReport::refused(label, refusal.clone()),
            }
        })
        .collect();
    let envelope = Envelope::new(reports);

    // The plain renderer has always put a refusal on stderr; keep it there.
    if options.format == Format::Plain {
        for source in &envelope.sources {
            if let Outcome::Refused(refusal) = &source.outcome {
                error_line(&refusal.message);
            }
        }
    }

    match options.format {
        Format::Plain => print!("{}", envelope.text_styled(output_style())),
        Format::Json => print!("{}", envelope.json()),
        Format::Annotations => print!("{}", envelope.annotations()),
        Format::Sarif => print!("{}", envelope.sarif(&registry.metas())),
    }

    // The exit code comes from the same record the report does: a refusal is an error (2), a finding
    // at or above the caller's threshold is a failure (1), anything else is clean (0).
    if envelope.has_refusal() {
        ExitCode::from(EXIT_ERROR)
    } else if envelope.fails(options.fail_on) {
        ExitCode::from(EXIT_FINDINGS)
    } else {
        ExitCode::from(EXIT_CLEAN)
    }
}

/// Report a run-level refusal: the message on stderr, and — in a machine format — a named refusal on
/// stdout, so a consumer reading only stdout never sees an empty document it cannot interpret.
fn refuse(format: Format, name: &'static str, message: String) -> ExitCode {
    error_line(&message);
    let refused = Envelope::refused(name, message);
    match format {
        Format::Plain => {}
        // A refused run has no results, so there are no rules to describe.
        Format::Json => print!("{}", refused.json()),
        Format::Annotations => print!("{}", refused.annotations()),
        Format::Sarif => print!("{}", refused.sarif(&[])),
    }
    ExitCode::from(EXIT_ERROR)
}

/// One source that was read and linted, before any suppression.
struct LintedSource {
    /// The inline directives the source's own text declares.
    directives: muhtesip::directives::Directives,
    /// The findings, unsuppressed.
    findings: Vec<Finding>,
}

/// How findings are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// One line per finding, for a person.
    Plain,
    /// One JSON envelope, for a program.
    Json,
    /// Log commands the runner turns into annotations on the diff.
    Annotations,
    /// A SARIF 2.1.0 document, for code scanning.
    Sarif,
}

impl Format {
    /// The format a name selects.
    fn from_name(name: &str) -> Result<Self, String> {
        match name {
            FORMAT_PLAIN => Ok(Format::Plain),
            FORMAT_JSON => Ok(Format::Json),
            FORMAT_ANNOTATIONS => Ok(Format::Annotations),
            FORMAT_SARIF => Ok(Format::Sarif),
            other => Err(format!(
                "unknown format '{other}'; use {FORMAT_PLAIN}, {FORMAT_JSON}, {FORMAT_ANNOTATIONS}, or {FORMAT_SARIF}"
            )),
        }
    }
}

/// The terminal code for a refusal: a defect in the invocation, in the same red as a defect in a file.
const ERROR_SGR: &str = "1;31";

/// Whether color belongs on a destination.
///
/// Color is for a person reading a terminal, so it is off when the destination is not one (a pipe, a
/// file, a step capturing output) and off on a `TERM=dumb` terminal. `CLICOLOR_FORCE` asks for color
/// anyway, which is the only way to colorize a log a person will read later — and the only way to
/// exercise this path without borrowing a terminal. `NO_COLOR` outranks even that: it is a person
/// saying no once for everything, and the person is the one who has to read the output.
fn color_enabled(is_terminal: bool, no_color: bool, force: bool, term: Option<&str>) -> bool {
    if no_color {
        return false;
    }
    if force {
        return true;
    }
    is_terminal && term != Some("dumb")
}

/// The style for a destination, from its own terminal check and the environment.
fn style_for(is_terminal: bool) -> Style {
    let set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
    let term = std::env::var("TERM").ok();
    let colored = color_enabled(
        is_terminal,
        set("NO_COLOR"),
        set("CLICOLOR_FORCE"),
        term.as_deref(),
    );
    match colored {
        true => Style::Ansi,
        false => Style::Plain,
    }
}

/// The style for standard output.
fn output_style() -> Style {
    style_for(std::io::stdout().is_terminal())
}

/// The style for standard error, decided on its own terminal: the two streams are redirected
/// separately, and a person can be reading one while the other goes to a file.
fn error_style() -> Style {
    style_for(std::io::stderr().is_terminal())
}

/// One refusal on standard error: `muhtesip: <message>`, with the name in red for a person reading it.
fn error_line(message: &str) {
    eprintln!("{}: {message}", paint("muhtesip", ERROR_SGR, error_style()));
}

/// What a run does instead of linting.
///
/// One choice rather than three booleans: `--init-config`, `--version` and `--help` all mean "do this
/// one thing and stop", so a run that asked for two of them is no longer a state the types allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Lint the sources.
    Lint,
    /// Write a starter configuration.
    InitConfig,
    /// Print the version.
    Version,
    /// Print the usage.
    Help,
}

/// Record the one action a run may take, refusing a second one and refusing a value.
fn set_action(
    action: &mut Action,
    wanted: Action,
    flag: &str,
    inline: Option<&str>,
) -> Result<(), String> {
    if inline.is_some() {
        return Err(format!("{flag} takes no value"));
    }
    if *action != Action::Lint {
        return Err(format!(
            "only one of {INIT_FLAG}, {VERSION_FLAG}, {HELP_FLAG} may be given"
        ));
    }
    *action = wanted;
    Ok(())
}

/// The value of a flag: from `--flag=value`, or from the argument after it.
fn value_of<'a>(
    flag: &str,
    what: &str,
    inline: Option<&'a str>,
    remaining: &mut impl Iterator<Item = &'a String>,
) -> Result<&'a str, String> {
    match inline {
        Some(value) if !value.is_empty() => Ok(value),
        Some(_) => Err(format!("{flag} needs {what}, not an empty value")),
        None => remaining
            .next()
            .map(String::as_str)
            .ok_or_else(|| format!("{flag} needs {what}")),
    }
}

/// The message for an option nobody defined, naming the flag they probably meant.
fn unknown_option(typed: &str) -> String {
    match nearest_flag(typed) {
        Some(known) => format!("unknown option '{typed}'; did you mean '{known}'?"),
        None => format!("unknown option '{typed}'"),
    }
}

/// The known flag closest to what was typed, when it is close enough to be a typo.
///
/// Two edits covers a transposition or a single slip. Beyond that the honest answer is to say nothing
/// rather than guess, so an option nobody defined is reported as itself. A flag that merely *starts*
/// with what was typed is a prefix rather than a typo — `--fail` for `--fail-on` — and is suggested
/// too, but only when there is something to have been typing.
fn nearest_flag(typed: &str) -> Option<&'static str> {
    const KNOWN: [&str; 6] = [
        CONFIG_FLAG,
        FORMAT_FLAG,
        FAIL_ON_FLAG,
        INIT_FLAG,
        VERSION_FLAG,
        HELP_FLAG,
    ];
    KNOWN
        .into_iter()
        .map(|flag| (edit_distance(typed, flag), flag))
        .filter(|(distance, _)| *distance <= 2)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, flag)| flag)
        .or_else(|| {
            KNOWN
                .into_iter()
                .find(|flag| typed.len() > OPTION_PREFIX.len() && flag.starts_with(typed))
        })
}

/// The Levenshtein distance between two strings, in characters rather than bytes.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, a_char) in a.chars().enumerate() {
        current[0] = i + 1;
        for (j, b_char) in b.iter().enumerate() {
            let substitute = previous[j] + usize::from(a_char != *b_char);
            current[j + 1] = substitute.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// The command line as this binary understands it.
struct Options {
    /// The configuration file named on the command line, if any.
    config: Option<String>,
    /// The output format.
    format: Format,
    /// What this run does instead of linting.
    action: Action,
    /// The severity a finding must reach for the run to fail.
    fail_on: Severity,
    /// The sources to lint, in order. Empty means standard input.
    sources: Vec<String>,
}

impl Options {
    /// Split the arguments, or explain what is wrong with them.
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut config = None;
        let mut format_given = false;
        let mut format = Format::Plain;
        let mut action = Action::Lint;
        let mut fail_on_given = false;
        let mut fail_on = DEFAULT_FAIL_ON;
        let mut sources = Vec::new();

        let mut remaining = args.iter();
        while let Some(arg) = remaining.next() {
            // `--flag=value` is taken as well as `--flag value`. Both are typed in practice, and the
            // `=` form cannot be split by anything between the command line and here.
            let (flag, inline) = match arg.split_once('=') {
                Some((flag, value)) => (flag, Some(value)),
                None => (arg.as_str(), None),
            };
            match flag {
                CONFIG_FLAG => {
                    if config.is_some() {
                        return Err(format!("{CONFIG_FLAG} given more than once"));
                    }
                    let path = value_of(CONFIG_FLAG, "a file path", inline, &mut remaining)?;
                    config = Some(path.to_string());
                }
                FORMAT_FLAG => {
                    if format_given {
                        return Err(format!("{FORMAT_FLAG} given more than once"));
                    }
                    let name = value_of(FORMAT_FLAG, "a format name", inline, &mut remaining)?;
                    format = Format::from_name(name)?;
                    format_given = true;
                }
                INIT_FLAG => set_action(&mut action, Action::InitConfig, INIT_FLAG, inline)?,
                VERSION_FLAG => set_action(&mut action, Action::Version, VERSION_FLAG, inline)?,
                HELP_FLAG => set_action(&mut action, Action::Help, HELP_FLAG, inline)?,
                FAIL_ON_FLAG => {
                    if fail_on_given {
                        return Err(format!("{FAIL_ON_FLAG} given more than once"));
                    }
                    let name = value_of(FAIL_ON_FLAG, "a severity name", inline, &mut remaining)?;
                    fail_on = Severity::from_name(name).ok_or_else(|| {
                        format!("unknown severity '{name}'; use {SEVERITY_NAMES}")
                    })?;
                    fail_on_given = true;
                }
                // `--` ends the options: what follows is a source, whatever it looks like. The
                // alternative is a file that cannot be named because its name begins with a dash.
                "--" => sources.extend(remaining.by_ref().cloned()),
                _ if flag.starts_with(OPTION_PREFIX) => return Err(unknown_option(flag)),
                _ => sources.push(arg.clone()),
            }
        }

        // `--init-config` writes a file and stops, so any combination with it is undefined rather
        // than something to guess at.
        if action == Action::InitConfig {
            if config.is_some() {
                return Err(format!("{INIT_FLAG} takes no {CONFIG_FLAG}"));
            }
            if format_given {
                return Err(format!(
                    "{INIT_FLAG} produces no report, so it takes no {FORMAT_FLAG}"
                ));
            }
            if fail_on_given {
                return Err(format!(
                    "{INIT_FLAG} produces nothing to fail on, so it takes no {FAIL_ON_FLAG}"
                ));
            }
            if !sources.is_empty() {
                return Err(format!("{INIT_FLAG} lints nothing, so it takes no sources"));
            }
        }

        Ok(Self {
            config,
            format,
            action,
            fail_on,
            sources,
        })
    }
}

/// Load the configuration: the named file, a discovered one, or the empty default.
fn load_config(config_path: Option<&str>) -> Result<Config, Box<dyn std::error::Error>> {
    let found = match config_path {
        Some(path) => Some(PathBuf::from(path)),
        None => discover_config()?,
    };
    let Some(path) = found else {
        return Ok(Config::default());
    };

    let text = std::fs::read_to_string(&path)?;
    Ok(Config::parse(&text)?)
}

/// Build the rule set, applying the configuration's per-rule adjustments.
fn build_registry(config: &Config) -> Result<Registry, Box<dyn std::error::Error>> {
    let registry = Registry::all();
    registry.validate(config.settings())?;
    Ok(registry.configure(config.settings().clone()))
}

/// Find a configuration file by walking up from the working directory.
///
/// The search stops at the first directory holding a repository marker, so a configuration file in
/// a parent project never silently governs this one.
fn discover_config() -> Result<Option<PathBuf>, std::io::Error> {
    let mut directory = std::env::current_dir()?;
    loop {
        let candidate = directory.join(CONFIG_FILE);
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
        if directory.join(REPO_MARKER).exists() || !directory.pop() {
            return Ok(None);
        }
    }
}

/// Expand each argument into the sources to lint: a file stays as it is, a directory becomes the
/// workflow files it holds.
///
/// A path that cannot be inspected is passed through unchanged, so reading it reports the failure
/// rather than the expansion swallowing it.
fn expand(requested: &[String]) -> Vec<String> {
    let mut sources = Vec::new();
    for request in requested {
        let is_directory = request != STDIN_ARG
            && std::fs::metadata(request)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false);
        if is_directory {
            sources.extend(workflow_files(Path::new(request)));
        } else {
            sources.push(request.clone());
        }
    }
    sources
}

/// The workflow files a directory holds, in path order.
///
/// A project directory expands to its `.github/workflows` directory, because that is the only place
/// the platform reads workflows from. A directory that does not have one expands to the YAML files
/// directly inside it. Neither search recurses: the platform reads only the top level of its
/// workflow directory, so going deeper would check files that cannot run.
fn workflow_files(directory: &Path) -> Vec<String> {
    let workflows = directory.join(WORKFLOWS_DIR);
    let target = if workflows.is_dir() {
        workflows
    } else {
        directory.to_path_buf()
    };

    let mut files: Vec<String> = std::fs::read_dir(&target)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && is_workflow_file(path))
                .map(|path| path.to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Whether a path carries one of the extensions a workflow file may have.
fn is_workflow_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| WORKFLOW_EXTENSIONS.contains(&extension))
}

/// Write a starter configuration, refusing to overwrite a file that is already there.
fn init_config() -> ExitCode {
    let path = Path::new(CONFIG_FILE);
    if path.exists() {
        error_line(&format!(
            "{CONFIG_FILE} already exists; refusing to overwrite it"
        ));
        return ExitCode::from(EXIT_ERROR);
    }

    let document = Config::template(&Registry::all());
    if let Err(error) = std::fs::write(path, document) {
        error_line(&format!("could not write {CONFIG_FILE}: {error}"));
        return ExitCode::from(EXIT_ERROR);
    }

    println!("wrote {CONFIG_FILE}");
    ExitCode::from(EXIT_CLEAN)
}

/// Read one source (`-` means standard input).
fn read_source(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    if source == STDIN_ARG {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        Ok(buffer)
    } else {
        Ok(std::fs::read_to_string(source)?)
    }
}

/// The human-facing name of a source, for output.
fn label_for(source: &str) -> &str {
    if source == STDIN_ARG {
        STDIN_LABEL
    } else {
        source
    }
}
