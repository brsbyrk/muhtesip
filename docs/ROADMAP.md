# muhtesip — feature roadmap, positioning, and benchmark plan

> **Status: DRAFT (plan) — Phase A complete (A1–A3, A3b, A3c shipped); Phase B complete (B1–B6, B7a–B7f
> shipped); Phase C complete (C1–C5); E1–E4 done; B8a/B8b/B8c-format done. Deferred with reasons:
> B8b-2 (caller-side `with:`), B8c-metadata (action.yml), B9 (expression type-check).**
>
> **Note on paths (2026-10-07):** the repository became a workspace. Entries below were written when the
> library and the binary were one crate, so their `src/`, `tests/` and `examples/` paths are the
> **library crate's** — now `crates/muhtesip/…` — and `src/main.rs` is now
> `crates/muhtesip-cli/src/main.rs`. The entries keep their original wording as the record of what was
> done; the current layout is in [`../AGENTS.md`](../AGENTS.md).
> Research inputs (kept in the development working copy — `out/` is not published, so these are named
> rather than linked): `out/research/actionlint-inventory.md`, `out/research/zizmor-inventory.md`,
> `out/research/landscape-and-benchmarks.md`.
>
> **DEFECTS (2026-10-04).**
>
> 1. **UNFIXED, and NOT a bug — a spec disagreement.** A multi-line flow sequence whose closing `]`
>    is at the same indentation as its parent key is rejected by `yaml-rust2`/`saphyr-parser`, so
>    **every finding for such a file is lost** behind a parse refusal. `python/cpython`'s own
>    `mypy.yml` is written that way and GitHub runs it.
>    The maintainers' position is that the document is invalid **YAML 1.2** — the spec says *"flow
>    nodes must be indented by at least one more space than the parent block collection"* — and they
>    closed the earlier report as `invalid-yaml`. Checked, not assumed: moving that `]` two spaces
>    deeper makes their parser accept the file, so the trigger is unambiguous and this is not a
>    scanner mistake. libyaml (PyYAML) accepts both forms.
>    Tracked upstream at **saphyr-rs/saphyr#55** (`help wanted`, `needs-discussion`; `yaml-rust2#69`
>    redirects there). Our evidence was added on 2026-10-04 rather than filed as a duplicate: the live
>    `python/cpython` file, and that this shape is the only one rejected out of 258 corpus files —
>    relevant because a GitHub-workflow tool must parse the ecosystem as it actually is.
>    **Practical consequence: muhtesip refuses a file GitHub accepts, and that is documented here rather
>    than worked around** — a lenient pre-parse would mean writing a parser.
> 2. **FIXED (2026-10-04) — YAML aliases were silently dropped.** The event builder ignored
>    `Event::Alias`, so a step written `uses: *action` was **absent from the document** — no error,
>    no refusal, just a missing finding. Demonstrated with a three-step job: steps 1 and 3 flagged,
>    step 2 gone, while libyaml reported all three. Fixed **in our own code** by keeping the
>    dependency's anchor ids and resolving aliases, so no new dependency and no `unsafe` was needed.
>    The test that pinned the wrong behaviour is inverted: it now asserts one finding per usage site,
>    at the line each is written on.
>
> **RESOLVED (2026-10-04): a merge key (`<<:`) used to be refused; it is now merged.** `<<: *base`
> inserts the anchored mapping's entries into the mapping, with YAML's two precedence rules: a key the
> mapping writes itself always wins, and in a list of mappings the earlier one wins. Both hold because
> the merge is applied when the mapping **ends** rather than when `<<` is read, so a written key wins
> wherever it sits. A merge whose value contains no mapping is still refused **by name**, rather than
> silently losing the entries. No file in the 258-file corpus uses a merge key, so this closed a
> correctness gap rather than a measured false positive — the honest reading of the change.
> Competitor facts were re-verified 2026-10-03 (see *Provenance*). Slices marked **(provisional)**
> rest on material that was inventoried but not yet read closely by the planner.

## 1. Where muhtesip stands

A Rust **library** that parses a CI workflow document and returns findings. Slices 1–2 are done:
`lint_str`/`lint` + two rules (`unpinned-action`, `missing-timeout`) and a thin binary. The library
prints nothing and holds no state; the CLI is a separate shell.

**The bet, stated plainly:** the differentiator is *embeddability and rules-as-data*, not rule
count. The two serious incumbents cannot be linked into a Rust program.

## 2. Competitive positioning (verified 2026-10-03)

| Tool | Lang | Distributed as | Embeddable **in Rust**? | Popularity | 
|---|---|---|---|---|
| **muhtesip** | Rust | library + bin | **yes — the point** | new (name free on crates.io) |
| actionlint | Go | bin + Go module | no — Go-only, no C ABI | 4,292★ · v1.7.12 |
| zizmor | Rust | **bin only** (`has_lib=false`) | **no** | 6,631★ · 250,873 dl · v1.30.1 |
| ghalint | Go | bin | no | 261★ · v1.5.6 |
| action-validator | Rust | **lib + bin** | yes (schema only) | 33,843 dl |
| gh-actions-lint / gha-lint / ravelact | Rust | lib | yes | 70 / 302 / 142 dl |

**Read:** the Rust mind-share is zizmor's, but it ships a binary. The only embeddable Rust crates
are negligible (≤302 dl) or schema-only (`action-validator`). **No embeddable, actions-aware,
rule-engine Rust library exists.** That is muhtesip's opening — and it is a *shape* gap, not a
features gap: on features, muhtesip trails both incumbents badly.

## 3. Coverage matrix (what exists, who has it)

Canonical feature families; `—` = absent. Derived from the two inventories.

| Family | muhtesip now | actionlint | zizmor |
|---|---|---|---|
| Action pinning | `unpinned-action` | action format, outdated popular actions | `unpinned-uses`, `unpinned-images`, ref audits (online) |
| Job timeout | `missing-timeout` | — | — |
| **Expression parse + type-check** | — | **9 checks (its big edge)** | `template-injection` (subset) |
| Shell/Python static analysis | — | shellcheck + pyflakes (**external tools**) | — |
| Permissions | — | `permissions` | `excessive-permissions`, `undocumented-permissions` |
| `needs` DAG | — | `job-needs` | — |
| Matrix | — | `matrix` | — |
| Events / cron / glob | — | `events`, `glob` | `dangerous-triggers` |
| Runner labels | — | `runner-label` | `self-hosted-runner` |
| ids (uniqueness/naming) | — | `id` | — |
| Credentials / secrets | — | `credentials` | container-cred, overprovisioned, inherit, outside-env, unredacted |
| Env-var names | — | `env-var` | — |
| Shell name | — | `shell-name` | — |
| Action/composite metadata | — | `action`, `action.yml` schema | `superfluous-actions` |
| Deprecated commands | — | `deprecated-commands` | — |
| `if:` soundness | — | `if-cond` | `unsound-condition/-contains/-ternary` |
| Config + ignores | — | `.github/actionlint.yaml`, `-ignore` regex | `zizmor.yml`, inline `# zizmor: ignore[…]`, personas |
| Output formats | plain only | `-format` Go template → JSON/SARIF/markdown/annotations | `plain/json/sarif/github` |
| LSP | — | editor plugins | `--lsp` (experimental) |
| Embeddable library | **yes** | Go only | **no** |

Totals: actionlint = **38 checks / 18 rule kinds** (2 need an external tool); zizmor = **41 audits**
(5 need the network). Neither number is muhtesip's target to *match* — see the roadmap ordering.

## 4. Roadmap

Ordering principle: **deepen the reusable pattern first, then close coverage, then add surfaces.**
Adding 15 rules before the rule-metadata/policy layer exists means rewriting all 15. Each slice is
bounded and independently shippable.

### Phase A — Turn the rule engine into a declared system (the reusable pattern)

- **A1 — Rule metadata as data.** **DONE (2026-10-03).** Each rule declares a `RuleMeta` (`id`,
  default `Severity`, one-line description); `Rule::id()` is derived from it, so they cannot
  disagree; the registry publishes the table via `metas()`/`meta(id)`; `docs_url()` is derived from
  the crate's repository URL. See `src/rule.rs`, `docs/rules.md`, `tests/metadata.rs`,
  `examples/registry.rs`. *Why first:* `--format`, SARIF/LSP, the config layer, and the severity map
  all read this; without it each later surface hardcodes rule knowledge.
  *DoD:* ✅ registry exposes metadata for every shipped rule; ✅ tests assert metadata is
  complete, ids unique, findings name declared rules, and emitted severity equals the default.
- **A2 — Severity override map + enable/disable, as data.** **DONE (2026-10-03).**
  `RuleSettings`/`RuleOverride` (`src/settings.rs`) are pure data; `Registry::configure(settings)`
  applies them as a **lookup on rule id** in `find()` — a disabled rule is skipped, an adjusted
  severity is written over what the rule emitted, no rule named in code. `RuleSettings` is the exact
  shape A3's config file will parse into. See `tests/settings.rs`, `examples/registry.rs`.
  *DoD:* ✅ tests prove escalating/downgrading one rule leaves the other at its default, disabling
  one rule removes only its findings, and empty/unknown/explicit-enable settings are inert.
- **A3 — Config document.** **DONE (2026-10-03).** `src/config.rs` parses `rules.<id>.{severity,
  enabled}` into the `RuleSettings` that A2 already applies; the CLI takes `--config <path>`.
  Strict on purpose — a config file is user input, so unknown top-level keys, unknown settings,
  misspelt severities, and (via `Registry::validate`) unknown rule ids are **reported**, while the
  in-memory API stays permissive. See `tests/config.rs`, `tests/fixtures/muhtesip.yaml`.
  *DoD:* ✅ a fixture config changes the findings of a fixed document; ✅ parsing and validation are
  unit-tested; ✅ the binary applies a config and rejects a bad one with exit `2`.
- **A3b — Path-scoped ignores.** **DONE (2026-10-03).** `Config::parse` reads `paths.<glob>.ignore`
  (a glob matched against the path, a regex matched against the finding's message) and exposes
  `Config::is_ignored(path, finding)`; the CLI filters with it, so an ignored finding neither prints
  nor affects the exit code. Invalid globs and regexes are **rejected at parse time**, not silently
  dropped. Adds the crate's first real dependencies beyond the parser (`globset`, `regex`). Design
  note: the filter is a pure function of `(path, finding)` living on `Config`, so `lint_str` stays
  path-free and the library still returns values. See `tests/config.rs`, `tests/fixtures/ignores.yaml`.
- **A3c — Config discovery.** **DONE (2026-10-03).** The CLI walks up from the working directory for
  `.muhtesip.yaml`; an explicit `--config` wins, and the search stops at the first directory holding a
  repository marker, so a parent project's file never silently governs this one.

### Phase B — Coverage toward actionlint parity (offline, no external tool)

Every slice is a rule + a fixture pair. Rule ids track actionlint's so the two can be diffed (the
benchmark's `rules.map.toml` depends on this).

- **B1 — `runner-label`.** **DONE (2026-10-03).** Reports a `runs-on` label GitHub does not host.
  The label table is **data** (`src/data.rs`), the union of GitHub's docs tables and actionlint's
  curated list — verified 2026-10-03 that neither is a superset (the docs list `ubuntu-26.04`,
  `xcode-27`, `windows-11-vs2026-arm`; actionlint lists the `*-cores` larger runners and
  `ubuntu-slim`). Skips what it cannot decide (expressions; any list containing `self-hosted`),
  because a linter must not false-positive on a real label. `src/model.rs` gained `Job::runs_on`.
  See `tests/runner_label.rs`.
  *DoD:* ✅ one finding per unknown label, pointing at the `runs-on` value's line; ✅ documented
  labels across every family are accepted; ✅ self-hosted lists and expressions are not flagged.
- **B1b — Conflicting runner labels.** **DONE (2026-10-03).** One `runs-on` list naming two operating
  systems never schedules; `data.rs` groups hosted labels by the OS they provide (`os_family`).
  Deliberately narrower than actionlint, which also flags two *versions* of one system — that claim
  false-positives on real files, and a false report costs more than a missing one.
- **B2 — `permissions`.** **DONE (2026-10-03).** Checks the workflow-level and every job-level
  `permissions` block: unknown scope, unknown access, unknown whole-block token. Scope table is the
  union of the docs and actionlint (again neither is a superset). Access is validated against the
  general `read`/`write`/`none`, deliberately without actionlint's per-scope narrowing. Skipped:
  expressions. See `tests/permissions.rs`.
- **B3 — `job-needs`.** **DONE (2026-10-03).** Three checks over the dependency graph: undefined
  reference, repeated entry, and dependency cycles. Cycle detection is a depth-first walk that marks
  the jobs on the current path, so it reports exactly the **members** of each cycle — not jobs that
  merely depend on one, which a Kahn leftover-set heuristic would wrongly accuse. Diverge from
  actionlint deliberately: it reports only the first cycle, and it also flags duplicate *job ids*
  (that belongs to `B4`). Ids compare case-insensitively, as the platform does. See
  `tests/job_needs.rs`.
- **B4 — `id`.** **DONE (2026-10-03).** Job and step id uniqueness (case-insensitive, as the
  platform compares them) plus the id grammar `^[a-zA-Z_][a-zA-Z0-9_-]*$`, applied to job ids, step
  ids, **and `needs` references**. Step ids are unique within a job; job ids across the workflow —
  this is the home for duplicate job ids, which actionlint reports under `job-needs` instead.
  Model work: `Step` gained `id`/`id_line`, and `Node::Mapping` now carries each entry's **key
  line**, so a job-id finding points at the id itself rather than the mapping one line below it.
  The grammar is compiled once (`LazyLock<Regex>`), and matches ASCII only, like the platform.
  See `tests/id.rs`.
- **B5 — `deprecated-commands`.** **DONE (2026-10-03).** Detects the four retired workflow commands
  in a `run:` script (`set-output`, `save-state`, `set-env`, `add-path`), each with the replacement
  from a data table sourced to the two deprecation announcements. Findings point at the command's
  **own line** inside a multi-line script — the model records the byte offset and the rule counts
  line breaks before it. Tier is **warning**, not error: `set-output`/`save-state` still work with
  a deprecation notice while `set-env`/`add-path` no longer do, so "this will break" is the honest
  claim. See `tests/deprecated_commands.rs`.
- **B6 — `if-cond`.** **DONE (2026-10-03).** Two checks on job and step conditions: the wrapper
  footgun (text outside `${{ }}` leaves the value non-empty, so the condition is always true —
  `${{ false }} || ${{ true }}` never skips), and the bare literals `true`/`false`. **Scope call:**
  actionlint finds these via its full expression parser; muhtesip does *not* guess at non-literal
  constants (`1 == 1`, `!true`) and says so in `docs/rules.md` — that belongs to the `B9` expression
  epic. Detecting nothing is better than detecting wrongly. See `tests/if_cond.rs`.
- **B7 — the small-rule batch**, one commit per rule.
  - **B7a — `shell-name`.** **DONE (2026-10-03).** Validates a step's `shell:` against the platform
    the job's `runs-on` labels select, reusing `os_family` from the runner-label work: `sh` is
    macOS/Linux only, `cmd`/`powershell` are Windows only. The message says *why* a shell is
    unavailable ("invalid on Windows") when it is a real shell, and drops the qualifier when the
    name is simply wrong. Custom shells (`{0}`) and expressions are skipped. See
    `tests/shell_name.rs`.
  - **B7b — `env-var`.** **DONE (2026-10-03).** Rejects `&`, `=`, spaces, and tabs in an `env`
    variable name, at workflow, job, and step level — the platform silently does not export such a
    variable, so the step runs without a value it expects. Names built from expressions are
    skipped. Uses the model's mapping key lines, so a finding points at the name itself. See
    `tests/env_var.rs`.
  - **B7c — `credentials`.** **DONE (2026-10-03).** A `container`'s or a service's
    `credentials.password` written as a literal is an error: it is committed in plain text. A value
    containing an expression passes. Model: `Job::container_passwords` (`ContainerPassword`), which
    walks `container` and each `services.<name>` down to `credentials.password`. See
    `tests/credentials.rs`.
    **Gap noted for B7b:** actionlint also checks `container`/`services` `env` names; muhtesip's
    model reaches only workflow/job/step `env`. Now that container traversal exists, closing it is
    small — a follow-up, not a silent omission.
  - **B7d — `matrix`.** **DONE (2026-10-03).** Two checks: a row listing the same value twice, and
    an `exclude` with nothing to remove (no rows and no `include`). **Scope call:** actionlint also
    verifies that each `exclude` value *matches* a matrix value, with subset matching over objects
    and arrays — a false-positive-prone analysis deferred to its own slice, and not guessed at
    meanwhile. The same value in two different rows is not a repeat. Model: `Job::matrix`
    (`Matrix`/`MatrixRow`/`MatrixValue`), which records `include`/`exclude` as counts rather than
    parsing their combination semantics. See `tests/matrix.rs`.
  - **B7e — `glob`.** **DONE (2026-10-03).** Validates the filter patterns in `branches`,
    `branches-ignore`, `tags`, `tags-ignore` (as git ref names) and `paths`, `paths-ignore` (as
    file paths), plus character-class syntax. **Sizing correction:** this was listed as a small
    rule, but it needed an `on:` model (`Document::triggers`) *and* a port of actionlint's
    262-line `globValidator` state machine (`src/glob.rs`, 9 unit tests), so it went in as two
    commits and `B7f` will reuse the `on:` model. See `tests/glob.rs`.
  - **B7f — `events`.** **DONE (2026-10-04).** Unknown trigger names, activity types
    (`types`) an event does not accept, ref/path filters used on an event that does not support
    them, `branches`+`branches-ignore`-style pairs, and `workflows` outside/absent from
    `workflow_run`. Reused the `on:` model from B7e as planned, and the trigger-name/activity-type
    table is data in `src/data.rs`. **Phase B is complete.** Cron syntax, the five-minute interval,
    the `schedule` timezone, and the `workflow_dispatch`/`workflow_call` input schemas were
    **deferred** (they need a cron engine, an IANA timezone database, and an inputs model) — see
    `B8` below.
  - **B8 — `schedule` and workflow-call inputs.** The **schedule half is DONE (2026-10-04)**, as
    **B8a**: cron field syntax, the platform's five-minute minimum interval (computed, not
    pattern-matched), and the IANA timezone name, all inside the `events` rule where actionlint puts
    them too. The descriptor ban and the floor and the timezone key are each quoted from the docs;
    the bounds and the decision to accept `JAN`/`MON` names are recorded with their reasons in
    `docs/rules.md`. The **input half is DONE (2026-10-04)** too, as **B8b**: the declaration schema of
    both events, which caught the corpus's two real defects. The **caller's** side — a `with:` against
    the callee's inputs — is deferred as **B8b-2** because it needs the called workflow's document, which
    the library cannot fetch by design.
  - **Maintenance (mid-B7): `src/rules.rs` split into `src/rules/`, one module per rule.** It had
    reached 772 lines across 10 rules and contradicted this document's own layout. Done as a
    verbatim script split, not a retype: **107 tests pass and the binary's output on all 13 fixtures
    is unchanged**. Largest source file is now 139 lines.
- **B8c — `action`: format + local/composite `action.yml` metadata validation.** The **format half is
  DONE (2026-10-07)** as the `action-ref` rule: a `uses:` reference in no documented form is an error, on
  a step *and* on a job calling a reusable workflow. Two things came out of building it. (1) The
  job-level `uses` reached no rule at all — the model carried it nowhere, so a workflow calling another
  workflow was invisible to every check, which is the silent-loss class; `actions/checkout@v7` at job
  level is now reported as the movable reference it is. (2) The documented forms are a **union**:
  GitHub's syntax page lists `$/path` (the same repository at the running commit), which no actionlint
  rule id knows, and the docs state that a `$/` reference "must not include an `@{ref}` suffix" — a
  checkable defect that reading only the incumbent would have missed. Building the forms also exposed a
  false positive in `unpinned-action`: `$/` references were being reported as unpinned, and **29 of the
  corpus's findings were exactly that**. Total went 772 → **753** (−29 removed, +10 added, measured by
  diffing the pre-change binary over the same files, not reasoned from a balance).
  The **metadata half stays deferred** — it validates an `action.yml`, a different document with its own
  parse path. *(provisional — the metadata schema was inventoried, not yet spec'd. Not a publish
  blocker; see "Before publishing".)*
  One question is left open on purpose: whether a `docker://` reference may name a digest. The
  workflow-syntax page never mentions digests, so the rule neither accepts nor recommends one, and a
  `docker://` reference is judged by `unpinned-action` like any other movable tag. Guessing would trade
  a false positive for a false negative.
- **B9 — Expression parse + type-check** (`expression` kind; actionlint's 9 checks). **Its own
  epic**: a lexer, parser, context model (`github`, `needs`, `matrix`, `steps`), and a type
  checker. Biggest slice by far; the honest way to close the single largest coverage gap.
  **(provisional — needs its own spec before any code.)**
- **B10 — External-tool rules (shellcheck/pyflakes): decide, don't drift.** A library must not
  silently spawn processes. If taken, it is an **opt-in, feature-gated** rule that shells out and
  reports the tool's absence — never core. *Open decision D2.*

### Phase C — Surfaces (make it usable to humans and CI)

- **C1 — Output formats:** `--format` for `json`, `sarif` (v2.1.0), and GitHub annotations. Reads
  A1's metadata. (See the `machine-readable-output` skill: one derivation, text renderer beside it.)
  - **`json` — DONE (2026-10-04).** `src/report.rs` holds the envelope: `Envelope` → `SourceReport`
    → `Outcome::{Linted, Refused}`, with `text()` and `json()` reading the same derivation. The
    plain bytes are unchanged (the pre-existing CLI tests are the no-op proof). Findings carry the
    plain line as `"text"`, and `tests/report.rs` reassembles the text output from the envelope, so
    the correspondence is asserted rather than hoped for. Refusals are named at the site
    (`READ_ERROR`, `PARSE_ERROR`, `CONFIG_ERROR`); an empty result and a refusal do not look alike.
    No new runtime dependency — the JSON writer is ~40 lines, and `serde_json` is a **dev**-dependency
    so that tests parse the output with a real parser.
  - **`annotations` — DONE (2026-10-04).** A third reading of the same envelope: one runner log
    command per finding (`::warning file=…,line=…,title=<rule>::<message>`), and one per refusal with
    its name in the `title`. The escaping is taken from the runner's own implementation
    (`actions/toolkit`, `packages/core/src/command.ts`): `%`, CR and LF everywhere, plus `:` and `,`
    in a property value — a message holding a newline would otherwise end the command. The severity →
    level table is data (`src/data.rs`), with a test asserting it covers every severity, and `note`
    maps to `notice`. No new dependency.
  - **`sarif` — DONE (2026-10-04).** A SARIF 2.1.0 document (`Envelope::sarif(rules)`, the rule table
    supplied by the caller because the record does not own the registry). Only the rules the results
    cite are described, sorted, so the bytes are stable; a refusal is an `executionSuccessful: false`
    invocation with a named notification rather than a result, and a path becomes a `uri` with only
    the URI-breaking characters escaped. **Verified against the official 2.1.0 schema**
    (`json.schemastore.org/sarif-2.1.0.json`, 112 KB, fetched and run through `jsonschema`) over four
    shapes — findings, a parse refusal, a config refusal, and three sources at once. All valid. That
    check needs the schema file, so the suite asserts the invariants instead and this line records the
    run. C1 is now complete.
- **C2 — Inline ignore comments:** `# muhtesip: ignore[<id>]`.
  - **DONE (2026-10-04).** `src/directives.rs`: `Directives::parse(text)` scans the raw text (the YAML
    dependency drops comments) and `ignores(line, rule)` answers. Coverage is **by indentation** — a
    directive covers its own line and every deeper following line, the block it introduces — which
    handles zizmor's documented `run: | # zizmor: ignore[...]` case without needing spans. The syntax
    matches zizmor's (`# <tool>: ignore[a, b]`, names trimmed, trailing explanation allowed), so one
    habit works in both tools. Suppression is applied in **one place**, beside the config ignores and
    before either encoding, so `--format json` reflects it exactly as plain output does.
  - **Was open decision D5** below: a directive that names an unknown rule — or is malformed — was
    silent. **Resolved 2026-10-04**: both are now reported by the `directive` rule.
- **C3 — Directory / project discovery:** lint a whole `.github/workflows/` tree.
  - **DONE (2026-10-04).** A directory argument expands to its workflow files: a project directory to
    its `.github/workflows`, a directory without one to the YAML files directly inside it. Neither
    search recurses — the platform reads only the top level, so a deeper file cannot run. Expansion
    is sorted, so two runs diff cleanly, and it happens **before** the standard-input default: a
    directory that matches nothing must not silently start reading a pipe. An empty match is exit 0
    with `"sources": []`, which says "I checked nothing" — the truth, rather than a fabricated pass.
- **C4 — `--init-config`.**
  - **DONE (2026-10-04).** `Config::template(&Registry)` builds the document *from the registry*, so
    it cannot describe a rule the build lacks or omit one it has; a test parses it back with the
    strict reader, because a template the tool cannot read is worse than none. The write lives in the
    CLI (the library still never touches the filesystem) and **refuses to overwrite** an existing
    file. Undefined combinations (`--format`, sources, `--config`) are refused rather than guessed at.

- **C5 — `--fail-on <error|warning|note>`: the exit code follows severity, not the bare presence of
  findings.**
  - **DONE (2026-10-04), driven by Experiment B's numbers rather than by taste.** The corpus showed
    **539 of muhtesip's 770 findings are `note`**, and any finding at all failed the run — so on a real
    repository muhtesip failed CI on advisory output alone. That is the fastest way to get a linter muted
    rather than adopted. `Severity` is now declared **least serious first**, so the derived ordering
    *is* the severity order (`Note < Warning < Error`) and a threshold comparison cannot quietly pick
    the wrong end of the scale; the library gained `Envelope::fails(threshold)`; the **caller** picks
    the threshold, because severity is the library's business and failing a build is policy. Default
    `warning` (notes print, do not fail); `--fail-on note` restores the previous behaviour;
    `--fail-on error` fails on defects only. `--init-config` refuses it, as it refuses `--format`.
    The contract in `docs/SPEC.md` and `README.md` was updated with the code, and the test that had
    pinned "one finding remains → exit 1" now pins **both** thresholds on the same input.
  - **Verified on the corpus, not argued:** under the new default, **75** files fail where **219**
    failed before — **144 real repositories were being failed by advice alone**. Predicted from the
    saved findings and then confirmed by running the built binary over all 258 files: 182 exit 0
    (38 clean + 144 notes-only), 75 exit 1, 1 exit 2 (the known parser refusal).

### Phase D — Security-audit family (zizmor adjacency) — DECIDE LATER

Overlaps zizmor (`template-injection`, `excessive-permissions`, `unpinned-images`,
`overprovisioned-secrets`, `use-trusted-publishing`). **Do not enter by default** — this is where a
me-too product dies. Revisit once B is credible. Flagged as **decision D3**.

### Phase E — Prove it: benchmark, release, dogfood

- **E1 — `bench/` harness.** Experiment A (speed) + Experiment B (coverage), exactly as designed in
  `out/research/landscape-and-benchmarks.md` §2. Deliverable: a re-runnable harness producing
  `speed.{json,csv}` and `coverage.{json,csv}` plus a rendered table.
  > **Note (2026-10-07):** the harness was removed from the repository. This entry is the record of
  > what it did and what it measured; the paths below name files that no longer ship.
  - **E1a — Experiment A (speed). DONE (2026-10-04).** `bench/fetch-corpus.sh` (20 repositories
    pinned to SHAs, 258 files, 1.43 MB, `corpus.lock.json` + `corpus.sha256` committed, the bytes
    themselves not), `examples/bench.rs` (in-process), `bench/measure.py` (whole CLI runs, median of
    11, spread and peak RSS reported) and `bench/run.sh` (orchestration; a comparator is measured
    only if present). Start-up is never subtracted. Results, machine and caveats: `bench/README.md`.
    Headline: **88 MB/s in process, 29 ms per 258-file CLI run, 5.5 MB RSS, 1.97 MB binary**, against
    actionlint's 57.6 ms / 27.3 MB / 5.83 MB and zizmor's 383 ms / 67.2 MB / 22.9 MB. **Both
    competitors have no in-process number at all** — neither ships a library target.
  - **E1b — Experiment B (coverage). The comparison is DONE (2026-10-04); labelled precision/recall
    is NOT built.** `bench/run-coverage.sh` + `bench/coverage.py` compare all three tools on the same
    258 files **by concept**, through `bench/rules.map.json` — each tool names the same idea
    differently, and where a tool has no equivalent rule the map says `null` rather than mapping onto
    a different check.
    Totals: **muhtesip 770, actionlint 170, zizmor 841** (`--offline`). On the concepts they share:
    `runner-label` **40 vs 72 with agreement on all 40** (the 32 extra are all self-hosted `1ES.Pool=…`
    labels in microsoft/vscode that actionlint calls unknown); `permissions` 4 vs 4, all agreed;
    `unpinned-action` 187 against zizmor's `unpinned-uses` 156, matched only coarsely because zizmor
    reports an enclosing job **span**, not a line.
    Everything else that fired has no counterpart: muhtesip's `missing-timeout` (539, advisory, and its
    largest rule); actionlint's `workflow-call`/`action`/`expression`/`syntax-check`/…; zizmor's
    `artipacked`/`template-injection`/`excessive-permissions`/`cache-poisoning`/… — **zizmor is a
    security auditor and muhtesip a correctness linter**, which is the Phase D decision, and this table
    is what that decision costs.
    **E1b-2 — precision and recall on labelled fixtures. DONE (2026-10-04).** `bench/labels/` holds
    seven workflows with **17 declared findings**, each carrying the reason for it, labelled by reading
    the files against the platform's documented behaviour and **never** by running a tool — a label
    taken from a tool's output only confirms what the tool already believes. `bench/run-quality.sh` +
    `quality.py` measure **detection, location and false positives separately**: muhtesip recall **1.00**,
    precision **0.94**, 16/17 on the declared line. It also produced the two defects now queued as
    **E1b-3** and **E1b-4**, and **disproved two of its own labels** (a dash in an env name; an
    expression-valued permission) — recorded in `bench/README.md`, because a label that bends to the
    tool is not a label. Agreement is still not precision and only this set speaks to precision.
  - **E1c — the YAML dependency defect. MEASURED (2026-10-04); decision below.**
    `bench/parser-trial` runs the reproducer plus all 258 corpus files through **six** candidates,
    each driven through its event API *with markers* — the shape `src/model.rs` needs, because every
    finding's line comes from the parser's marker.

    | candidate | reproducer | corpus | marks | safe to call |
    |---|---|---|---|---|
    | yaml-rust2 0.13 *(current)* | REJECTS | 257/258 | yes | yes |
    | yaml-rust2 0.12 | REJECTS | 257/258 | yes | yes |
    | saphyr-parser 0.1 *(the "successor")* | REJECTS | 257/258 | yes | yes |
    | serde_yaml 0.9 | parses | 258/258 | **no** | yes |
    | unsafe-libyaml 0.2 | parses | 258/258 | yes | **no — `unsafe` API** |
    | libyaml-safer 0.3 | parses | **PANICS** | yes | yes |

    Three facts, each of which only measurement could produce:
    - **The obvious swap fixes nothing.** `saphyr-parser`, the successor, rejects the same documents
      — and 0.12 does too, so it is not a regression either. Pinning is not a fix.
    - **The only crate that is both libyaml-correct and safe to call PANICS** on a common shape: a
      block scalar at end of input with **no final newline**. Minimal reproducer `run: |\n  echo hi`
      with no trailing newline — which is exactly how `actions/cache`'s `licensed.yml` ends. It
      aborts instead of erroring, in a tool that must never crash on a user's file.
    - **No crate satisfies safe-to-call + correct + marks.** So there is no swap that keeps the
      invariant and fixes the bug.

    **DECISION: keep `yaml-rust2`; `unsafe_code = "deny"` stays untouched.** The earlier
    recommendation to adopt `unsafe-libyaml` is **withdrawn** — it traded a stated invariant for one
    file in 258, and proposed that trade before the safe alternative had been measured. The owner
    rejected it, correctly.

    Follow-ups, in order:
    1. **Defect 2 (aliases) — DONE (2026-10-04).** Fixed in `Builder` with **no new dependency and no
       `unsafe`**: the dependency's event stream carries anchor **ids** (`Alias(usize)`, plus an
       anchor id on every node event), so the builder keeps an anchor table and resolves the alias to
       a copy of the named node. An aliased node is re-anchored to the **alias's** line, because that
       is where it is used — resolving to the anchor's line produced two identical findings on one
       line, which reads as a duplicate rather than two usages. A dangling alias stays refused (the
       dependency reports it first). Tests: one finding per usage site, lines 6/7/8.
    2. **Defect 1 — reported upstream, 2026-10-04.** Both were already known, so the findings were
       added to the canonical threads instead of filed as duplicates:
       [`saphyr-rs/saphyr#55`](https://github.com/saphyr-rs/saphyr/issues/55#issuecomment-5979508840)
       (the tracked issue, `needs-discussion`) and
       [`simonask/libyaml-safer#6`](https://github.com/simonask/libyaml-safer/issues/6#issuecomment-5979509061)
       (an open panic, now with a minimal reproducer). `yaml-rust2#69` already redirects to the
       former. The flow-sequence case stays documented and tripwired here until upstream decides.
    3. **E1d — merge keys (`<<:`). DONE (2026-10-04).** Was refused **by name**; now merged. Both of
       YAML's precedence rules are applied when the mapping *ends* — so a written key wins whether it
       sits above or below the merge — and a merge whose value holds no mapping is still refused by
       name. The `tests/lint.rs` case flipped from "refused" to "merged" and grew a control; the
       refusal cases moved to their own test. An alias where a *key* belongs is now its own refusal
       with its own message, because it was never a merge.
- **E2 — Publish to crates.io.** The name is **`muhtesip`**, probed free on crates.io, npm and pypi on
  2026-10-07. README states the positioning (embeddable library) and links the benchmark. Git tag +
  release.
- **E3 — Dogfood. DONE (2026-10-04).** The decision: **pin**. Every `uses:` in both workflows is pinned
  to a commit SHA with the version named in a comment — including `release.yml`, which holds
  `contents: write`, where a moved tag would hand out that permission — and every job declares a
  `timeout-minutes`, since muhtesip's own advice is a poor advertisement otherwise. **muhtesip now reports
  nothing on its own workflows** (`muhtesip .github/workflows` → exit 0), and CI runs exactly that as a
  step, so the state is enforced rather than remembered: unpinning an action fails the build.
- **E4 — Distribution: a CLI people can use without a toolchain. DONE (2026-10-04).**
  `.github/workflows/release.yml`, driven by a `v*` tag: five native platforms (linux x86_64/arm64,
  macOS arm64/Intel, windows x86_64), each built and archived by `scripts/package-release.sh` with a
  `.sha256` beside it, attached to the release by `gh`. No cross-compilation and no guessed action
  inputs. Two guards: the tag must equal `Cargo.toml`'s version, because a release whose name lies about
  its contents cannot be withdrawn; and the packaging lives in a script rather than inside the workflow,
  so it can be — and was — run before a tag ever needed it. The CLI gained `--version` and `--help`,
  which a binary people download is expected to answer, and the three action flags became one `Action`
  choice instead of three booleans that could contradict each other.
- **CLI ergonomics, and clap declined (2026-10-04).** Measured before deciding: `clap` with `derive` is
  **24 crates by itself**, against muhtesip's whole 40-crate tree — and Cargo dependencies are per crate,
  not per target, so it would land on every *library* consumer to serve the *binary*. The escapes are
  worse than the cost: feature-gate it and `cargo install muhtesip` stops working, put it in the default
  features and library users inherit it anyway, and a sibling `muhtesip-cli` crate means the name people
  install is no longer `muhtesip`. The hand-rolled parser stays, and the effort went where a person
  actually looks: **severity color on a terminal** (off when piped or redirected, under `NO_COLOR`, or
  on `TERM=dumb`; `CLICOLOR_FORCE` to ask for it), `--flag=value`, `--` to end the options, and an
  unknown flag answered with the one they meant. Color is presentation, never content: the JSON's
  `text` field is the plain line even when the same run is printing in color.

### Before publishing — the remaining order (set 2026-10-04)

Work everything left *before* the crates.io publish, in this order; each unit ships on its own, gated,
with its docs updated in the same commit.

1. ~~**E1d — merge keys (`<<:`).**~~ **DONE (2026-10-04).** Was refused by name; now merged, with both
   precedence rules, and only an unusable merge still refused. `src/model.rs` plus tests.
2. ~~**E1b-2 — labelled fixtures (precision/recall).**~~ **DONE (2026-10-04).** `bench/labels/` (seven
   files, 17 declared findings, every label carrying its reason) + `bench/quality.py` +
   `bench/run-quality.sh`, measuring **detection, location and false positives separately**. muhtesip:
   recall **1.00**, precision **0.94** at first measurement and **1.00** once E1b-3 landed, 16/17 on the
   declared line (the seventeenth is E1b-4). Results and adjudications: `bench/README.md`.
3. ~~**E1b-3 — `deprecated-commands` matches a name MENTION, not a command.**~~ **DONE (2026-10-04).**
   The pattern now requires the platform's command form — the closing `::` of `::name params::data` —
   so `echo "::set-output test"` and a migration note in a comment are silent, while all three real
   command forms are still reported. `bench/labels/clean.yaml` keeps that line as the regression
   control, and the set's precision went from 0.94 to **1.00** when this landed.
4. ~~**E1b-4 — a job-level finding points at the job's BODY, not its key.**~~ **DONE (2026-10-04).**
   `Job.line` is now the job's key line and the redundant `id_line` field is gone, so a finding about a
   job points at the job's name — where a reader would add the missing key. The class was audited: the
   seven places that read a job's line all take the same value, step lines are unaffected (a step's
   first line *is* its own line), the corpus still reports exactly 770 findings, and the labelled set's
   location column went from 16/17 to **17/17**.
5. ~~**B8a — `schedule`: cron syntax, the five-minute floor, the IANA timezone name.**~~ **DONE
   (2026-10-04).** A five-field validator with the field bounds as data, the floor computed from the
   shortest gap between firings (`59,0 * * * *` fires a minute apart across the hour, which no pattern
   match sees), and a timezone table generated from the tz database, links included. In the `events`
   rule, where actionlint groups it. Verified two ways: the label set gained a schedule case and muhtesip
   detects all three of its defects on the exact declared lines — **actionlint detects them too**, which
   is independent evidence they are real — and over the corpus's 48 scheduled workflows it reports
   nothing, with a control proving that means "all valid" rather than "not parsed" (a real file with its
   cron tampered with reports at the right line).
6. ~~**B8b — `workflow_dispatch` / `workflow_call` input schemas.**~~ **The declaration half is DONE
   (2026-10-04); the caller half is deferred, with its reason, as B8b-2.** A `workflow_call` input must
   declare a `type` (docs) and `choice` is not one of its values; a manual input must be one of its five;
   a default must agree with the declared type, with `${{ }}` exempt; `choice` needs `options`, `options`
   belong only to a choice, an option may not repeat, a choice's default must be among its options, and
   a manual block is capped at 25 inputs. Plus the asymmetry the corpus proved: required-and-default is
   reported for a reusable input (the caller's pass makes the default dead) and **not** for a manual one
   (the browser pre-fills it) — which is why the platform's own example uses that shape. **It found the
   two real defects the coverage run had named**, on the same files and lines actionlint reports, taking
   the corpus from 770 to 772. `src/inputs.rs` (the judgement) + `src/data/mod.rs` (the schema).
7. **B8b-2 — a caller's `with:` against the callee's declared inputs.** Needs the *called* workflow's
   document, which the library cannot fetch: it never touches the filesystem, by design. So this is an
   API decision — the embedder supplies the callee's document, or the CLI resolves a local path — and it
   is recorded rather than half-built. **Not a publish blocker.**
8. **E2 — publish. The preparation is DONE (2026-10-04); the publish itself is the maintainer's.**
   `Cargo.toml` carries keywords, categories and an explicit `readme`, and `cargo package` verifies what
   publish would send (112 files, no corpus bytes, no build output). Two gaps were found and closed on
   the way, both of the "the claim was never checked" kind: **`rust-version = "1.85"` was false** — the
   events rule used a let-chain, stable in 1.88 — so those two sites were rewritten and
   `cargo +1.85.0 check --all-targets` now passes; and **nothing in the gate could have told**, so CI
   gained an `msrv` job that checks the declared floor, a `doc` step (`RUSTDOCFLAGS=-D warnings`, since a
   broken intra-doc link is a broken link on docs.rs), and a `package` step. Left: `cargo publish`
   (the maintainer's to trigger), then the README's "not on crates.io yet" note comes out.

**Deliberately NOT before publish, with reasons:**

- **B9 — expression parser and type checker** (`github`/`needs`/`matrix`/`steps` contexts; 9 of
  actionlint's checks). The largest slice by far and the biggest remaining coverage gap, but it is an
  epic with its own spec, not a publish blocker: 15 rules on a sound static core is coherent, and a
  half-built expression checker that misses type errors is worse than none. *(Recorded as needing its
  own spec before any code.)*
- **B8c — `action.yml` metadata validation** for `action`-type references. Same reasoning: its own spec.
  (This is the second thing this document numbered B8; renamed here so the ids are unique.)
- **B10 / decision D2 — external tools (`shellcheck`, `pyflakes`).** Recommendation stands: **not in the
  library**, because a library that silently spawns processes is a different product. A thin CLI extra
  would be the route if it is ever wanted.
- **F1/F2 — an example consumer, then an LSP.** F1 is the strongest artifact for the *differentiator*
  and belongs right after the publish rather than before it: linking a published crate is what makes it
  believable.

### Phase F — Embedding proof (the differentiator, made visible)

- **F1 — An example consumer.** A small `examples/` or downstream crate that links `muhtesip` and uses
  findings in-process. This is the one artifact that *demonstrates* the differentiator rather than
  asserting it. Strong candidate for the GitHub-profile purpose.
- **F2 — LSP** (later), reusing C1's formats.

## 5. Benchmark plan (summary)

Full design: `out/research/landscape-and-benchmarks.md` §2. The rules that matter:

- **Two experiments, never conflated:** A = speed, B = coverage.
- **Corpus:** ~40–60 popular public repos pinned at a commit SHA; only `.github/workflows/*.y{a,}ml`;
  stored on disk with a SHA-256 manifest so the timed run is **offline** (`bench/corpus.lock.json`,
  `bench/corpus.sha256`, `fetch-corpus.sh`).
- **Metrics (precise):** one-shot CLI wall time (median of ≥10), amortized in-process throughput,
  per-file p50/p95/p99, cold-start, peak RSS, binary size, findings-per-KLOC (the **honest** metric).
- **Fairness rule:** muhtesip is a library, the others are binaries — report **both** the one-shot CLI
  number and the in-process number; if startup is subtracted it must be shown as a stated
  assumption, **never** silently deducted.
- **Coverage (Exp. B):** run all three on the same bytes, normalize rule ids via `rules.map.toml`,
  emit three diff classes (muhtesip-only / incumbent-only / agreed), and compute precision/recall on
  the fixtures that carry expected labels — a tool that "finds more" by false-positiving loses.
- **Pitfalls:** warm page cache (state it), zizmor online audits need a token (disable for timing),
  version-pin everything, compare like-for-like strip settings.

## 6. Open decisions

- **D1 — Next slice.** Recommendation: **A1→A3** (the declaration layer), *then* Phase B. Rationale:
  it is the reusable pattern the project exists to leave behind, and it makes every B rule cheaper.
  Alternative: start at B1 and backfill A later. **Default taken unless overridden: A1.**
- **D2 — External tools (shellcheck/pyflakes).** Ship as an opt-in gated rule, or not at all?
  (Recommendation: not at all in the library; possibly a thin CLI extra.)
- **D3 — Security-audit family (Phase D).** Enter, or stay a generic rule engine? (Recommendation:
  stay generic; revisit after B.)
- **D4 — Rule-id convention.** Mirror actionlint's ids exactly, or use ours with a documented map?
  (Recommendation: our ids, plus `bench/rules.map.json`; exact mirroring imports their drift. Taken;
  the map is JSON, not the TOML this document first sketched, because the machine's python3 is 3.9 and
  has no stdlib TOML reader — the map is data either way.)
- **D5 — A directive that names nothing useful is silent. DECIDED and DONE (2026-10-04): option (b),
  reported as a finding.** `# muhtesip: ignore[typo]` suppressed nothing and said nothing; so did
  `# muhtesip: ignore` with no brackets. Option (a) (refuse the run) was rejected because one typo in a
  comment would discard *every* finding in *every* file, and (c) (stay silent, matching zizmor) keeps
  the worst shape a suppression can take: it reads as a suppression, suppresses nothing, and the
  author's red build then looks like a bug rather than a typo. Implemented as **a real rule**
  (`directive`, warning), because that is what makes it disableable in config, present in
  `--init-config`, and described in SARIF — and it required the API decision option (b) named: the
  `Rule` trait gained a **defaulted** `check_source(source, ids, findings)` hook, since a comment is
  not part of the parsed model and only the registry knows which rule names exist. Precision was the
  constraint: prose that mentions the marker is **not** reported, and neither is a wrong-case marker —
  only shapes that can only be an attempt at a directive. `src/directives.rs` still only reports what
  the text says (`named()`, `malformed()`); the rule judges it against the ids.

## 7. Provenance & confidence

- **Verified by the planner directly (2026-10-03):** actionlint `docs/checks.md` → **38** check
  sections; zizmor `registry.rs` → **41** `register_audit!`; crates.io `muhtesip` → **404 (free)**;
  actionlint 4,292★/v1.7.12, zizmor 6,631★/250,873 dl/v1.30.1, ghalint 261★/v1.5.6.
- **From cited inventories (primary sources, per-file citations in `out/research/`):** the full rule
  and audit lists, config schemas, CLI surfaces, embeddability findings.
- **Provisional / needs its own spec:** B8 (action metadata), B9 (expression type-checker), Phase D.
- **Not measured yet:** every number in §5 is a *plan*, not a result. No benchmark has been run.
