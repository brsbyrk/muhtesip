# muhtesip

[![ci](https://github.com/brsbyrk/muhtesip/actions/workflows/ci.yml/badge.svg)](https://github.com/brsbyrk/muhtesip/actions/workflows/ci.yml)

A fast, **embeddable** linter for CI workflow files — a Rust library that parses a workflow
document and returns findings. It never prints, exits, or touches the filesystem itself.

*Named for the Ottoman market inspector, whose whole job was checking that weights, measures and goods
met the standard — and naming the ones that did not.*

```rust
let findings = muhtesip::lint(workflow_text)?;
for finding in &findings {
    println!("line {}: {} {}", finding.line, finding.severity, finding.message);
}
```

## Install

```toml
[dependencies]
muhtesip = "0.1"
```

**Not on crates.io yet** — until it is, depend on the repository:

```toml
[dependencies]
muhtesip = { git = "https://github.com/brsbyrk/muhtesip" }
```

The library needs no runtime, no network, and no other process: it takes a `&str` and returns
findings. The `muhtesip` binary is a thin shell around it for a CI step.

### The CLI

The library and the binary are **two crates in one workspace**: `muhtesip` holds every rule and every
decision, and `muhtesip-cli` is the shell around it. The boundary is the point — it keeps "the library
prints nothing, exits nothing and touches no filesystem" true in the *manifest* rather than only in
prose, and it keeps any dependency the CLI might want off library consumers. Three ways to get the
binary, easiest first:

- **A prebuilt binary.** Download the archive for your platform from
  [Releases](https://github.com/brsbyrk/muhtesip/releases), check it against the `.sha256` published
  beside it, and unpack: it holds the binary and nothing else. macOS (arm64/Intel), Linux
  (x86_64/arm64) and Windows (x86_64).
- **Cargo:** `cargo install muhtesip-cli` once it is on crates.io, or right now
  `cargo install --git https://github.com/brsbyrk/muhtesip muhtesip-cli`. The crate is `muhtesip-cli`;
  the command it installs is `muhtesip`.
- **From a checkout:** `cargo build --release -p muhtesip-cli`, then `target/release/muhtesip --help`.

`muhtesip --help` prints the usage block above; `muhtesip --version` prints the version. Exit codes are `0`
clean, `1` a finding at or above the threshold (default `warning`), `2` a read, parse, or usage
failure.

## Why it exists

The established workflow linters are **binaries**. actionlint is Go (no C ABI, so a Rust program
cannot link it); zizmor — the dominant Rust entry — ships **no library target** at all
(`has_lib = false` on crates.io). So neither can be embedded in a Rust program in-process.

muhtesip is the same job as **an embeddable crate** with its rule set described as data: link it, hand
it bytes, get values back. A thin `muhtesip` binary ships on top, holding no logic the library does not
already expose.

> **Scope today:** phases A, B and C are complete — 16 rules, four output formats
> (`plain`/`json`/`annotations`/`sarif`), inline directives, directory discovery and `--init-config`,
> 325 tests. See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the honest state and what is next.
>
> **Measured, not asserted** (2026-10-04, Apple M1, 258 real workflow files pinned to SHAs):
> **88 MB/s in process**, **29 ms** for a whole 258-file CLI run, 5.5 MB peak RSS from a 1.97 MB
> binary — against actionlint's 57.6 ms / 27.3 MB / 5.83 MB and zizmor's 383 ms / 67.2 MB / 22.9 MB.
> Neither incumbent has an in-process number at all, because neither ships a library target.
> **Coverage is not yet measured**, so this is *not* a claim of "faster and as good": muhtesip ships 16
> rules to actionlint's 38 checks and zizmor's 41 audits, and a tool that checks less is faster.
> The harness that produced these numbers is a development working copy and is **not part of this
> repository**, so they are a record rather than something you can re-run from a checkout here.

## Rules

| Rule | Default | Catches |
|---|---|---|
| `unpinned-action` | warning | a `uses:` reference not pinned to an immutable commit |
| `action-ref` | error | a `uses:` reference the platform cannot resolve |
| `missing-timeout` | note | a job with no `timeout-minutes` |
| `runner-label` | error | a `runs-on` label that GitHub does not host |
| `permissions` | error | a `permissions` entry naming an unknown scope or access |
| `job-needs` | error | a `needs` reference that is undefined, repeated, or cyclic |
| `id` | error | a job or step id that is duplicated or malformed |
| `deprecated-commands` | warning | a `run` script using a retired workflow command |
| `if-cond` | warning | an `if` condition that is constant or badly wrapped |
| `shell-name` | error | a `shell` the job's runner does not provide |
| `env-var` | error | an `env` variable name that the platform will not export |
| `credentials` | error | a container password written literally instead of from a secret |
| `matrix` | warning | a repeated matrix value, or an `exclude` with nothing to remove |
| `glob` | error | an invalid ref or path filter pattern |
| `events` | error | an unknown `on` event, a filter it does not accept, a `schedule` the platform cannot run, or a declared input that contradicts its schema |
| `directive` | warning | a suppression comment that names an unknown rule, or is not usable |

Full reference: [`docs/rules.md`](docs/rules.md). Every rule publishes a `RuleMeta` (id, default
severity, description); `Registry::metas()` returns the table.

## Configuration

A project can escalate, downgrade, or disable any rule by id — as data, never a code branch:

```yaml
# .muhtesip.yaml
rules:
  unpinned-action:
    severity: error
  missing-timeout:
    enabled: false
paths:
  "**/legacy.yaml":            # scoped by path: suppress by matching the message
    ignore:
      - "not pinned to an immutable commit"
```

```sh
muhtesip .github/workflows/ci.yml            # discovers .muhtesip.yaml by walking up
muhtesip --config other.yaml ci.yaml         # or name one explicitly
```

The same policy is available in-process: `Registry::all().configure(settings)` for rule adjustments,
and `Config::is_ignored(path, finding)` for path scoped ignores. Unknown keys, unknown rule ids,
misspelt severities, invalid globs, and invalid regexes are all **reported**, not ignored.

A one-off finding can also be suppressed in the workflow itself, with the comment convention zizmor
uses — so the habit works in both tools:

```yaml
      - run: | # muhtesip: ignore[deprecated-commands]
          echo '::set-output name=x::1'
      - uses: example/toolbox@v4 # muhtesip: ignore[unpinned-action]
```

A directive covers its own line and every following line more indented than it — the block it opens.
So a trailing comment on `run: |` covers the script, and one on a job's key line covers the whole
job. Several rules share one directive (`# muhtesip: ignore[one, two]`), and a trailing explanation is
allowed. As in zizmor, a comment written *inside* a literal block is string content to YAML and
suppresses nothing — put it on the line that opens the block.

## Command line

```sh
muhtesip <file>...        # lint files; one line per finding
muhtesip <dir>...         # a directory expands to its workflow files
muhtesip < file.yaml      # or no arguments / '-' to read standard input
muhtesip --format json <file>...
muhtesip --fail-on error <file>...   # exit 1 only for defects; notes and warnings still print
muhtesip --init-config    # write a starter .muhtesip.yaml and stop
muhtesip --version        # print the version and stop
muhtesip --help           # print the usage and stop
```

`--flag=value` works as well as `--flag value`, `--` ends the options, and a mistyped flag is answered
with the one you probably meant. On a terminal, findings are colored by severity — presentation only:
piped or redirected output is byte-identical, and `NO_COLOR` (or `TERM=dumb`) turns color off.

A directory argument expands to its workflow files: a project directory to its `.github/workflows`,
and a directory without one to the YAML files directly inside it. Neither search recurses, because
the platform reads only the top level of its workflow directory — going deeper would check files
that cannot run. Files are checked in path order, so two runs can be diffed. A directory that holds
no workflow files is **not** an error: it matches nothing, reports nothing, and exits `0`, and the
envelope's `"sources": []` says exactly that.

`--init-config` writes a starter `.muhtesip.yaml` listing every rule this build ships — generated from
the registry, so it cannot describe a rule the build does not have — then stops. It lints nothing,
so it takes no sources and no `--format`, and it **refuses to overwrite** a file that is already
there.

Output: `{path}:{line}: {severity}: {message} [{rule}]`. Exit codes — `0` clean, `1` a finding at or
above the fail threshold, `2` a read, parse, or configuration failure (distinct from `1`, so CI can
tell the two apart).

The threshold defaults to `warning`, so **advice does not fail a build**: a run whose worst finding is
a `note` still prints it, and exits `0`. `--fail-on <error|warning|note>` moves the threshold —
`--fail-on note` makes any finding fail, `--fail-on error` only defects. The default is not a detail:
on the benchmark corpus **539 of muhtesip's 770 findings are notes**, so a `note` default would fail
almost every real repository on advisory output alone.

`--format json` emits one versioned envelope for the whole run. Both encodings are built from the
same result object, and every finding carries the exact plain line it would have printed, so the two
are provably the same answer — `tests/report.rs` reassembles the text output from the envelope:

```sh
$ muhtesip --format json ci.yml
{
  "envelope": "muhtesip/1",
  "refusal": null,
  "sources": [
    {
      "source": "ci.yml",
      "findings": [
        {"file": "ci.yml", "line": 10, "rule": "unpinned-action", "severity": "warning", "message": "action 'example/toolbox@v4' is not pinned to an immutable commit", "text": "ci.yml:10: warning: action 'example/toolbox@v4' is not pinned to an immutable commit [unpinned-action]"}
      ]
    }
  ]
}
```

A source that could not be linted carries a named `refusal` instead of findings (`READ_ERROR`,
`PARSE_ERROR`), and a run that could not start puts the refusal at the top level (`CONFIG_ERROR`).
A clean source is `"findings": []` with no `refusal`, so an empty result and a refusal never look
alike.

`--format annotations` renders that same envelope as runner log commands, so findings appear inline
on the pull-request diff:

```sh
$ muhtesip --format annotations ci.yml
::warning file=ci.yml,line=10,title=unpinned-action::action 'example/toolbox@v4' is not pinned to an immutable commit
```

A refusal keeps its name in the `title`, so a consumer reading annotations can still tell a refusal
from a finding. Message bodies and property values are escaped with the rules from the runner's own
implementation (`actions/toolkit`), because a message holding a newline would otherwise end the
command and leak the rest of it as a new log line. `note` severity is reported as `notice`.

`--format sarif` emits a SARIF 2.1.0 document for code scanning. Only the rules a run actually cites
are described, in sorted order, so the same input produces the same bytes. A file that could not be
read makes the invocation `"executionSuccessful": false` and carries a named notification, so "the
tool could not read this" can never be mistaken for "this file is clean". One result line, verbatim:

```json
{"ruleId": "missing-timeout", "ruleIndex": 0, "level": "note", "message": {"text": "job 'build' has no timeout-minutes bound"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "tests/fixtures/bad.yaml"}, "region": {"startLine": 7}}}]}
```

The output validates against the official SARIF 2.1.0 schema. `tests/report.rs` asserts the
structural invariants that validation would — including that every `ruleIndex` points at the rule
its result names — and `docs/ROADMAP.md` records the schema run, which needs a 112 KB download and so
is deliberately not in the suite.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings   # --all-targets is the point
cargo test --workspace --all-features
cargo deny check                                           # licence + advisory gate (deny.toml)
```

CI (`.github/workflows/ci.yml`) runs all four on every push. `cargo deny` is not part of this
machine's default toolchain — install it with `cargo install cargo-deny` before running that line.

Conventions: [`AGENTS.md`](AGENTS.md). Reference: [`docs/rules.md`](docs/rules.md) (every rule),
[`docs/SPEC.md`](docs/SPEC.md) (the library contract), [`docs/ROADMAP.md`](docs/ROADMAP.md) (what is
next).

## License

MIT — see [`LICENSE`](LICENSE).
