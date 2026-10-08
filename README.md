# muhtesip

[![ci](https://github.com/brsbyrk/muhtesip/actions/workflows/ci.yml/badge.svg)](https://github.com/brsbyrk/muhtesip/actions/workflows/ci.yml)

An **embeddable** linter for CI workflow files: a Rust library that reads a workflow document and
returns findings. It never prints, exits, or touches the filesystem. A thin `muhtesip` binary ships
on top for CI.

A small rule set in an embeddable form — not a replacement for actionlint or zizmor.

```rust
let findings = muhtesip::lint(workflow_text)?;
for finding in &findings {
    println!("line {}: {} — {}", finding.line, finding.severity, finding.message);
}
```

## Install

```toml
# Not on crates.io yet.
muhtesip = { git = "https://github.com/brsbyrk/muhtesip" }
```

```sh
cargo install --git https://github.com/brsbyrk/muhtesip muhtesip-cli   # the CLI
```

## Use

```sh
muhtesip .github/workflows/ci.yml   # a file
muhtesip .github/workflows          # a directory — the workflow files it holds
muhtesip < ci.yml                   # or standard input
```

One line per finding: `{path}:{line}: {severity}: {message} [{rule}]`.

Exit codes: `0` clean, `1` a finding at or above the threshold, `2` a read, parse or usage failure.
The threshold defaults to `warning`, so advice does not fail a build — `--fail-on error|warning|note`
moves it. `--format plain|json|annotations|sarif` picks the encoding.

## Rules

| Rule | Default | Catches |
|---|---|---|
| `unpinned-action` | warning | a `uses:` not pinned to an immutable commit |
| `action-ref` | error | a `uses:` the platform cannot resolve |
| `missing-timeout` | note | a job with no `timeout-minutes` |
| `runner-label` | error | a `runs-on` label GitHub does not host |
| `permissions` | error | a `permissions` entry naming an unknown scope or access |
| `job-needs` | error | a `needs` reference that is undefined, repeated or cyclic |
| `id` | error | a job or step id that is duplicated or malformed |
| `deprecated-commands` | warning | a retired workflow command in a `run` script |
| `if-cond` | warning | an `if` condition that is constant |
| `shell-name` | error | a `shell` the job's runner does not provide |
| `env-var` | error | an `env` variable name the platform will not export |
| `credentials` | error | a container password written literally |
| `matrix` | warning | a repeated matrix value, or an `exclude` with nothing to remove |
| `glob` | error | an invalid ref or path filter pattern |
| `events` | error | an unknown event, a filter it does not accept, or an unusable `schedule` |
| `directive` | warning | a suppression comment that cannot work |

Full reference: [`docs/rules.md`](docs/rules.md).

## Configuration

```yaml
# .muhtesip.yaml — discovered by walking up from the source
rules:
  unpinned-action:
    severity: error
  missing-timeout:
    enabled: false
paths:
  "**/legacy.yaml":
    ignore:
      - "not pinned to an immutable commit"
```

A one-off finding can be suppressed in the workflow itself. A directive covers its own line and
every following line more indented than it — the block it opens — so a comment on a step's key line
covers that step, and one on a job's key line covers the job:

```yaml
      - uses: example/toolbox@v4 # muhtesip: ignore[unpinned-action]
```

Unknown keys, unknown rule ids and invalid patterns are reported, not ignored.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check            # licences and advisories (deny.toml)
```

Conventions: [`AGENTS.md`](AGENTS.md).

## License

MIT — see [`LICENSE`](LICENSE).

*Named for the Ottoman market inspector, whose job was checking that goods met the standard.*
