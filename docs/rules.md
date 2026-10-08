# muhtesip — rule reference

Every rule muhtesip ships, with its id, default severity, and what it catches. This page is the target
of each rule's `docs_url()` (see `crates/muhtesip/src/rule.rs`); a test asserts every link resolves to an anchor on
this page, so the two cannot drift.

Severities: **error**, **warning**, **note**. Defaults live in each rule's `RuleMeta`. A project can
change them — or turn a rule off — with a per-rule override (`muhtesip::RuleSettings`, applied via
`Registry::configure`), keyed by rule id.

## unpinned-action

- **Severity:** warning
- **Catches:** a `uses:` reference that is not pinned to an immutable commit — on a step, and on a job
  that calls a reusable workflow, which is the one `uses` that does not sit on a step. That job-level
  reference used to reach no rule at all: the model carried it nowhere, so a workflow calling another
  workflow was invisible to every check.
- **Why:** a tag or branch can be moved after review, so a workflow that trusts one is trusting
  whatever the target points at on the day it runs. Pin to a 40-hex commit instead.
- **Not flagged:** a reference pinned to a 40-character hex commit; a local reference (`./path`)
  resolved from the same checkout, which a third party cannot move; and a `$/path` reference, which the
  docs define as resolving "to that repository at the running commit (the same SHA as the running
  workflow or action)" — immutable by construction. The `$/` case was a false positive until
  `action-ref` forced the forms to be read properly: 19 of the corpus's 187 findings were `$/`-style
  references that no third party can move.
- **Also flagged:** a `docker://{image}:{tag}` reference, because a tag can be repointed after review.
  Whether the platform accepts a digest reference here is unverified — see `action-ref` below.

## action-ref

- **Severity:** error
- **Catches:** a `uses:` reference the platform cannot resolve, in any position.
- **Why:** an unresolvable reference does not fail as a typo. The run fails with a message about the
  action it could not find, which points at the symptom rather than at the reference that was written.
- **Documented forms, all accepted:** `{owner}/{repo}@{ref}`, `{owner}/{repo}/{path}@{ref}`, `$/path`
  (the same repository, at the running commit), `./path` (the runner's checked-out workspace), and
  `docker://{image}:{tag}`. The `$/` form is on GitHub's workflow-syntax page; no actionlint rule id
  covers it.
- **Not flagged:** anything starting `./`, including `./` alone, which is the action at the repository
  root. Measured: the corpus contains 50 of those among 1,527 `uses:` values, every one in GitHub's own
  action repositories, and a stricter reading of this rule rejected exactly those. A bare `$/` is
  accepted for the same reason.
- **Not flagged, deliberately:** a reference whose *owner* segment is malformed — for example
  `@actions/checkout@v4`. GitHub's username grammar is documented, but the grammar of a `uses:`
  reference is not, and the platform answers that case with a lookup failure rather than a syntax error.
  The rule says nothing rather than guessing at a grammar it cannot cite.
- **Deferred:** whether a `docker://` reference may name a digest. The workflow-syntax page never
  mentions digests, so a digest form is neither accepted nor recommended here, and such a reference is
  judged by `unpinned-action` like any other movable tag. Guessing would trade a false positive for a
  false negative.

## missing-timeout

- **Severity:** note
- **Catches:** a job with no `timeout-minutes`.
- **Why:** without a bound, a hung step runs until the platform's default limit, which is long
  enough to strand a queue.

## runner-label

- **Severity:** error
- **Catches:** a `runs-on` label that is not one GitHub hosts.
- **Why:** a misspelt or invented label never schedules, so the workflow fails for a reason that is
  invisible until a run is attempted.
- **Not flagged:** a documented hosted label (the union of GitHub's docs and actionlint's list — see
  `crates/muhtesip/src/data/`), a self-hosted preset (`linux`, `macos`, `windows`,
  `x64`, `arm`, `arm64`), any list containing `self-hosted` (a self-hosted runner's own labels are
  unknowable without configuration), and any value containing `${{ … }}` (an expression has no
  value to check).
- **Not yet:** conflicting labels in one list (e.g. an Ubuntu label beside a Windows one) — a
  later slice, `B1b`.

## permissions

- **Severity:** error
- **Catches:** a `permissions` entry naming a scope or an access the platform does not offer.
- **Why:** the platform silently ignores an unknown scope or access, so the job runs with less
  authority than the author intended — or the author believes a permission was granted that never
  was.
- **Checked:** the workflow-level block and each job's block.
- **Accepted scopes** are the union of GitHub's docs table and actionlint's list — neither is a
  superset. Accepted access is `read`, `write`, `none`; a whole-block value is `read-all`,
  `write-all`, or `{}`.
- **Not flagged:** expressions, and per-scope access narrowing (actionlint flags `id-token: read`;
  a per-scope claim goes stale and false-positives).

## job-needs

- **Severity:** error
- **Catches:** a `needs` reference that is undefined, repeated, or part of a dependency cycle.
- **Why:** `needs` decides execution order, so a broken graph either never schedules the job or
  schedules it before what it depends on.
- **Cycle reporting:** every job **in** a cycle is reported. A job that merely *depends* on a cycle
  is not — a leftover-node heuristic (Kahn's algorithm) would wrongly accuse it. actionlint reports
  only the first cycle.
- **Not flagged:** expressions, and case differences (`Build` and `BUILD` are the same job id).

## id

- **Severity:** error
- **Catches:** a job id or step id that is duplicated, or that does not match the platform's id
  grammar.
- **Why:** an id is how a workflow refers to a job or a step, so a duplicate makes the reference
  ambiguous and a malformed id makes it unrunnable.
- **Grammar:** `^[a-zA-Z_][a-zA-Z0-9_-]*$` — a letter or underscore, then letters, digits, hyphens,
  or underscores. ASCII, so `café` is rejected. Applied to job ids, step ids, and `needs`
  references (a `needs` entry is a job id, so it answers to the same grammar).
- **Uniqueness:** step ids are unique within their job; job ids are unique across the workflow.
  Both compare case-insensitively, as the platform does.
- **Not flagged:** expressions.

## deprecated-commands

- **Severity:** warning
- **Catches:** a retired workflow command used in a `run:` script — `::set-output`, `::save-state`,
  `::set-env`, `::add-path`.
- **Why:** the retired commands either do nothing or are refused, so the workflow silently loses
  the value it thought it was setting. The message names the form that replaces each one.
- **Precision:** a finding points at the command's own line inside a multi-line script, not at the
  script's first line. And the *form* is required: the platform reads a command only as
  `::name parameter=value::data`, so the closing `::` is what makes the text a command. Without it the
  line is ordinary output that merely mentions the name — `echo "::set-output test"` is not reported,
  and neither is a migration note in a comment. Found by the labelled set, which had exactly that line
  as a control.
- **Tier:** warning, not error — `set-output` and `save-state` still work with a deprecation
  notice; `set-env` and `add-path` no longer do. The tier is the honest middle of the two.
- **Not flagged:** a longer word that merely starts with a retired token (`::set-outputs`), a name
  without a closing `::`, and any script without a retired command.

## if-cond

- **Severity:** warning
- **Catches:** an `if:` condition on a job or a step that is a **constant**, or an expression that
  is not the whole value.
- **Why:** a constant condition either runs something the author meant to guard, or silently skips
  something the author meant to run.
- **The footgun:** text outside the `${{ }}` braces leaves the value non-empty, so the condition is
  **always true** — `${{ false }} || ${{ true }}` never skips anything. A `${{ }}` must be the
  entire condition.
- **Literals:** `true` / `false`, with or without the wrapper, are reported.
- **Not yet:** constant *expressions* that are not bare literals (`1 == 1`, `!true`). Proving those
  needs an expression parser and a constant-folding pass — that is the separate expression epic
  (`B9`), and muhtesip deliberately does not guess in the meantime.
- **Not flagged:** any condition that actually depends on something (`github.event_name == 'push'`).

## shell-name

- **Severity:** error
- **Catches:** a step's `shell:` naming a shell the job's runner does not provide.
- **Why:** the step fails before its script runs, and it fails at run time rather than at review.
- **Platform-aware:** the job's `runs-on` labels decide which shells exist. `sh` is macOS/Linux
  only; `cmd` and `powershell` are Windows only; `bash`, `pwsh`, and `python` are everywhere. So
  `shell: powershell` on `ubuntu-latest` is reported as invalid *on macOS or Linux*, while a
  misspelt `bsh` gets no platform qualifier — it is not a shell anywhere.
- **Not flagged:** a custom shell (one containing `{0}`, the script-path placeholder), an
  expression, a shell on a step that runs no script, and any runner whose platform the labels do
  not decide.

## env-var

- **Severity:** error
- **Catches:** an `env` variable name containing `&`, `=`, a space, or a tab.
- **Why:** the platform silently will not export such a variable, so the step runs without the
  value the author thought it had.
- **Checked:** the workflow-level, job-level, and step-level `env` blocks.
- **Not flagged:** a name built from an expression (it has no value to judge).
- **Gap:** actionlint also checks a `container`'s and each `service`'s `env` names; the model only
  reaches workflow/job/step `env`.

## credentials

- **Severity:** error
- **Catches:** a `container`'s or a `service`'s `credentials.password` written as a literal.
- **Why:** the password is committed to the repository in plain text, and anyone who can read the
  workflow can read it.
- **Not flagged:** a password whose value contains an expression (`${{ secrets.DB_PASSWORD }}`,
  `${{ env.X }}`, `${{ inputs.x }}`), and a container that declares no credentials.

## matrix

- **Severity:** warning
- **Catches:** a matrix row listing the same value twice, and an `exclude` that has nothing to
  remove (no rows, and no `include` that could add one).
- **Why:** a repeated value silently runs the same job twice; an `exclude` with nothing to remove
  is dead configuration that reads as if it filters something.
- **Not flagged:** the same value in two *different* rows (that is legitimate), an `exclude` that
  has rows to filter, an `exclude` alongside an `include`, and any value built from an expression.
- **Not yet:** checking that an `exclude` value actually matches a matrix value — actionlint does
  this with subset matching over objects and arrays, a false-positive-prone analysis of its own.

## glob

- **Severity:** error
- **Catches:** an invalid filter pattern in `branches`, `branches-ignore`, `tags`, `tags-ignore`,
  `paths`, or `paths-ignore`.
- **Why:** the platform rejects the workflow when it is read, so the run never starts — and the
  rejection names the pattern, not the mistake in it.
- **Ref patterns** are checked as git ref names: no spaces, `~`, `^`, `:`, `[` or `*` in a name; no
  leading `/`; no trailing `/` or `.`; `?` and `+` must follow an ordinary character.
- **Path patterns** are checked as file paths: a path may hold a space or a backslash that a ref
  name may not, but leading/trailing spaces and a leading `.` or `..` are rejected.
- **Character classes** are checked: a missing `]`, an empty class, a class of one character, a
  backwards range, and a range with no end are all reported.
- **Not flagged:** a pattern built from an expression, and any filter that holds no patterns
  (`types`, `workflows`, …).
- **Ported from** actionlint's `globValidator` (`crates/muhtesip/src/glob.rs`), with two deviations:
  no column tracking, and no whitespace-skipping when peeking.

## events

- **Severity:** error
- **Catches:** a trigger event that does not exist; an activity type an event does not have; a
  `types` list on an event that accepts none; a ref or path filter used on an event that does not
  filter on it; `branches` with `branches-ignore` (likewise `tags`/`tags-ignore`,
  `paths`/`paths-ignore`) on one event; `workflows` outside a `workflow_run` event; a `workflow_run`
  event that names no workflow; and a `schedule` the platform cannot run — a malformed cron
  expression, an entry with no `cron` at all, a cron that fires more often than the platform's
  minimum interval, or a timezone name the tz database does not carry.
- **Why:** the platform refuses the workflow when it is read, before any job is scheduled, and the
  refusal does not say which name or filter was wrong. A schedule is the same class: a cron that
  fires every minute is silently stretched to the five-minute floor, so the author gets fewer runs
  than they wrote and nothing tells them.
- **Data:** trigger names and their activity types are a table in `crates/muhtesip/src/data/`, from
  GitHub's "Events that trigger workflows" and actionlint's table. Three names were decided by hand:
  `actor` is a documentation *section* about scheduled
  workflows, and `pull_request_comment` is listed by the docs itself as "(use `issue_comment`)" —
  both are rejected, because neither is an event; `repository_dispatch` is accepted with unbounded
  `types`, because its activity types are caller-defined `event_type` values.
- **Schedules:** the field bounds are the usual crontab ones (minute 0-59, hour 0-23, day-of-month
  1-31, month 1-12, day-of-week 0-6, Sunday 0), and `@daily`-style descriptors are rejected because
  the docs state that outright. The docs name the operators but not the bounds and not whether names
  (`JAN`, `MON`) are allowed; names are **accepted**, as actionlint accepts them, and refusing a cron
  the platform runs is the false positive this rule exists to avoid. The five-minute
  floor and the timezone key are both documented: "The shortest interval you can run scheduled
  workflows is once every 5 minutes", and "By default, scheduled workflows run in UTC. You can
  optionally specify a timezone using an IANA timezone string". The interval is computed, not
  pattern-matched: `59,0 * * * *` fires one minute apart across the hour boundary, and only arithmetic
  over the firings sees that. The timezone names are a table generated from the tz database
  (`crates/muhtesip/src/data/timezones.rs`), links included, so a legacy-but-valid `Asia/Calcutta` is not refused.
- **Inputs:** the two events that declare inputs are checked against their schema. A `workflow_call`
  input must declare a `type` (the docs say so, and `choice` is not among the three values it takes); a
  `workflow_dispatch` input must be one of its five types; a default must agree with the declared type
  (`number` parseable as one, `boolean` `true`/`false`, and a default built from `${{ }}` is left alone
  because the platform resolves it); `type: choice` needs `options`; `options` belong only to a choice;
  an option may not repeat; a choice's default must be one of its options; and a manual block may
  declare at most 25 inputs. One asymmetry is deliberate, tested, and easy to get wrong: a **required
  `workflow_call` input that also carries a default** is reported, because every caller must pass it so
  the default can never apply — while the same pair on a **manual** input is correct, since the browser
  pre-fills it, which is why the platform's own documented example uses exactly that shape. The type
  lists come from the docs; the consistency checks come from actionlint's `rule_events.go`, which
  cites GitHub's changelog for manual-workflow input types; the judgement lives in
  `crates/muhtesip/src/inputs.rs` and the schema in `crates/muhtesip/src/data/mod.rs`.
- **Not yet:** a *caller's* `with:` against the callee's declared inputs. That needs the called
  workflow's document, which the library cannot fetch — it never touches the filesystem — so supplying
  it is an API decision of its own, recorded as **B8b-2** rather than left as an apparent omission.

## directive

- **Severity:** warning
- **Catches:** a suppression comment that names a rule this build does not have (`# muhtesip:
  ignore[unpinned-actionn]`), and a comment that reads as a suppression but is not a usable directive
  (`# muhtesip: ignore` with no brackets, `# muhtesip: ignore[]`, an unclosed bracket, or a space before the
  bracket).
- **Why:** a directive that suppresses nothing while *reading* as a suppression is the worst shape a
  suppression can take — the author believes the finding is silenced, so their red build looks like a
  bug rather than a typo. zizmor stays silent here, so this is a deliberate departure.
- **Precision:** prose that merely mentions the marker (`# muhtesip: ignore this for now`) is **not**
  reported, and neither is the wrong-case marker. Only the shapes that could only be an attempt at a
  directive are; a linter that flags prose gets switched off.
- **How:** it is the one rule that reads the source text rather than the parsed document, because the
  YAML model carries structure and a comment is not structure. The trait's `check_source` hook hands it
  the text and the ids this build has — deciding whether a *name* exists is a registry question, not a
  text question, so `crates/muhtesip/src/directives.rs` only reports what the text says.
- **It is a rule like any other:** it can be disabled or re-levelled in config, it appears in
  `--init-config`, it gets a SARIF descriptor, and `# muhtesip: ignore[directive]` is the explicit way to
  accept the comment as written.
