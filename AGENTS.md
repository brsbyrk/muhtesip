# AGENTS.md — muhtesip conventions

> **Resuming work?** Start at [`README.md`](README.md). This file describes the conventions.

## What this is

A fast, embeddable linter for CI workflow files: a library that returns findings, and a thin binary.

## Rust conventions

### Always
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — the gate. Plain
  `cargo clippy` does not lint test targets, so rot accumulates there invisibly.
- `cargo fmt --all -- --check`.
- `cargo test --workspace --all-features`.
- `anyhow::Result` for application errors; enums over string matching.
- Full names for types (`DependencyNode`, not `DepNode`).
- A doc comment on every item. **Enforced**: `[lints.rust] missing_docs = "warn"` and
  `clippy::missing_docs_in_private_items`, so an undocumented method, field, constant or variant
  fails the gate.
- Every main public method carries a `# Examples` section, and the examples are **doctests** —
  `cargo test` runs them, so a stale example fails the gate rather than misleading a reader.

### Never
- `unwrap()` in production — use `?`, `.context()`, or `.expect("why")`. **Enforced**:
  `[lints.clippy] unwrap_used = "deny"`. Unit tests inside the crate are exempt at the crate root
  (`#![cfg_attr(test, allow(clippy::unwrap_used, clippy::panic))]`) — but **integration test files
  are their own crates**, so that exemption does not reach `crates/*/tests/*.rs`. Use `assert!` and
  `.expect(…)` there; a `panic!` or `unwrap()` in an integration test is a gate failure.
- `todo!()` / `unimplemented!()` / `dbg!()` in committed code. **Enforced**.
- `unsafe` without an explicit `#[allow(unsafe_code)]` and a `// SAFETY:` comment.
  **Enforced**: `[lints.rust] unsafe_code = "deny"` + `undocumented_unsafe_blocks = "deny"`.
- Vendor names in code (file/type/field/env/table names). Vendor facts are DATA.
- Magic numbers — name it: a protocol CONSTANT vs a tunable POLICY knob. Per-entity values
  are a LOOKUP, not a branch.

### Library shape
A library returns values; it never writes to stdout or to a store by itself. Tracing is fine.

## Verification gate

Before reporting a coding task done:
1. `cargo test --workspace --all-features` — all pass (this includes the doctests)
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings` — 0 warnings
3. `cargo fmt --all -- --check` — clean

Read **exit codes, never a grep**: `cargo build | grep -c warning` reports a *failed* build.

CI runs three more steps beyond this local gate: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
--no-deps --all-features` (a broken intra-doc link is a broken link on docs.rs), `cargo package -p
muhtesip` (what `cargo publish` would send), and `cargo deny check` against `deny.toml` (licences and
advisories; `cargo install cargo-deny` — it is not in the default toolchain here). CI also runs
`muhtesip .github/workflows` on this repository, so its own advice is enforced on it.

## Layout

```
README.md               the project front page (and the crates' `readme`)
AGENTS.md               this file
LICENSE                 the MIT text; each crate symlinks to it so the packaged crate carries it
scripts/                repo scripts (package-release.sh)
docs/
  rules.md              rule reference — the target of every rule's docs_url()
crates/
  muhtesip/             the library: parse a workflow document, return findings, print nothing
    src/
      lib.rs        public API: lint_str / lint, the Error type
      cron.rs       cron field syntax and the shortest gap between firings (a pure function of the text)
      data/         vendor fact tables, always DATA and never branches:
        mod.rs        runner labels and their OS, permission scopes, id grammar, shells, retired
                      commands, trigger events and their activity types, annotation and SARIF levels,
                      the cron field bounds, the 5-minute floor, the workflow input types
        timezones.rs  the IANA names a schedule may name (598, links included)
      inputs.rs     the inputs the two events declare, judged against the schema (a pure function, no registry)
      model.rs      text -> Document (owns the YAML dep for documents); mappings carry key lines; aliases and merge keys are resolved here
      rule.rs       the Rule trait, RuleMeta (id/severity/description), Severity, Finding
      registry.rs   which rules run, declared as data (Registry::all); metas()/meta(id) lookups
      rules/        one module per rule (unpinned_action.rs, action_ref.rs, missing_timeout.rs, …); mod.rs re-exports them and holds the shared format consts
      settings.rs   per-rule adjustments as data (RuleSettings, RuleOverride)
      config.rs     parse a config document into settings (owns the YAML dep for config); Config::template generates a starter one
      report.rs     the run's result and every encoding of it: Envelope/SourceReport/Outcome, text() / json() / annotations() / sarif()
      directives.rs inline suppressions (# muhtesip: ignore[rule]): Directives::parse(text) -> ignores(line, rule)
    tests/          the library's tests, over tests/fixtures/
      lint.rs       end-to-end over tests/fixtures/{good,bad}.yaml
      metadata.rs   the rule-metadata contract: complete metadata, unique ids, declared findings
      settings.rs   overrides: escalate/downgrade/disable one rule, leave others alone
      runner_label.rs  known labels pass; unknown ones and OS conflicts are reported
      permissions.rs   unknown scope/access reported; expressions skipped
      job_needs.rs     undefined / repeated / cyclic dependencies; cycle members only
      id.rs            job/step id uniqueness and the id grammar
      deprecated_commands.rs  retired commands; findings point at the command's own line
      if_cond.rs       constant conditions, and an expression that is not the whole value
      shell_name.rs    shells checked against the runner's platform
      env_var.rs       env variable names the platform will not export
      credentials.rs   container/service passwords written literally
      matrix.rs        repeated matrix values; an exclude with nothing to remove
      glob.rs          ref/path filter patterns (validator ported in src/glob.rs)
      events.rs        unknown triggers, filters an event does not accept, a schedule the platform cannot run, and declared inputs that contradict their schema
      missing_timeout.rs  a job with no timeout-minutes bound; its own fixture, negative and control
      unpinned_action.rs  every immutable `uses:` form beside the reported ones; its own fixture
      report.rs        one derivation, every encoding: the envelope reassembles the text output; every SARIF ruleIndex resolves to its ruleId
      directives.rs    a directive suppresses exactly its own finding; one that cannot work is reported, not ignored
      fixtures/        the documents both crates' tests lint; a second copy would drift
    examples/
      registry.rs   prints the rule table — the embedding story in miniature
      bench.rs      times the library in process over a corpus directory
  muhtesip-cli/         the binary, and only the binary: read -> lint -> print -> exit code
    src/main.rs     thin: it holds no rule and no decision the library does not already make, and it is
                    the only place that reads the environment (terminal color, and nothing else)
    tests/
      cli.rs        spawns the binary: exit codes (--fail-on thresholds), output shape, --format
                    plain|json|annotations|sarif, directory expansion, --init-config, --version/--help,
                    colored output and the argument forms (= value, --, suggestions)
      config.rs     config discovery, ignores, and the generated template, parsed back by the strict
                    reader; it reaches across to the library crate's fixtures on purpose
out/                    development working notes (gitignored): the research inputs and the audit
                        reports that cite them
```

Update this section as modules land. A stale layout description is the first thing a delegate
reads and the first thing to be wrong.
