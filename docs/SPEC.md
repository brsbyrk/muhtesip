# muhtesip — specification

**What it is.** A fast, embeddable linter for CI workflow files. It reads a workflow
document and returns **findings** — it never prints, exits, or touches the filesystem
itself. That is the library contract (see AGENTS.md). A thin `muhtesip` binary ships on top
of the library (see *Slice 2* below); it holds no logic the library does not already expose.

**Why it exists.** The incumbent is written in Go and is a binary, not a library, so it
cannot be embedded in a Rust program. muhtesip is the same job as an embeddable crate with a
rule set described as data.

## Vocabulary (fixed — used in code)

- **Document** — one workflow file, parsed.
- **Node** — a value in the document, carrying its **line** (1-based).
- **Job** — a named unit under the document's `jobs` mapping.
- **Step** — an entry in a job's `steps` sequence.
- **Rule** — a named check. `id()` is a stable string; `check()` appends findings.
- **Finding** — `{ rule, severity, message, line }`. Severity is `Error | Warning | Note`.

## Architecture (three layers, one direction)

```
text ──parse──▶ Document ──rules──▶ Vec<Finding>
```

- `crates/muhtesip/src/model.rs` — parse text to `Document`. Owns the YAML dependency for documents.
- `crates/muhtesip/src/config.rs` — parse a configuration document into settings. Owns the YAML dependency for
  configuration. No other module sees the YAML crate.
- `crates/muhtesip/src/rule.rs` — the `Rule` trait, `RuleMeta` (a rule's declared facts), `Severity`, `Finding`.
- `crates/muhtesip/src/rules/` — one module per rule (`unpinned_action.rs`, `missing_timeout.rs`, …). Each rule is a
  zero-sized struct with its declarative metadata and its private helpers; `mod.rs` re-exports them
  and holds the format vocabulary more than one rule needs.
- `crates/muhtesip/src/settings.rs` — per-rule adjustments (`RuleSettings`, `RuleOverride`) as **data**.
- `crates/muhtesip/src/lib.rs` — `lint_str(&str, &Registry) -> Result<Vec<Finding>>`, the one entry point.
- `crates/muhtesip/src/registry.rs` — the rule set and its adjustments, as **data** (a list + a lookup), not code.

A rule must not know about another rule. Findings are sorted by `(line, rule)` before
return so output is deterministic — a linter whose output order depends on a `HashMap`
cannot be diffed.

## Slice 1 (done) — deliberately small, end to end

1. `model.rs`: parse `jobs.*.steps[*]` with each step's line number, via `yaml-rust2`.
2. Two rules, both real:
   - `unpinned-action` — a `uses:` value not pinned to an immutable ref (a 40-hex commit)
     is a supply-chain risk: a tag can be moved under you. Severity: Warning.
   - `missing-timeout` — a job with no `timeout-minutes` can hang a runner for the
     platform default. Severity: Note.
3. `lint_str` wiring + a `Registry::default()`.
4. Tests: unit tests per rule, plus fixtures under `crates/muhtesip/tests/fixtures/` and one integration
   test asserting exact findings for a good and a bad document.

## Non-goals for slice 1

- No CLI, no config file, no severity overrides, no autofix. Those are later slices and
  each gets its own spec section.
- No global rule ordering logic. No `unsafe`. No new dependency beyond the YAML parser
  (and `thiserror` for the error type if it earns its place).

## Slice 2 (this unit of work) — the thin `muhtesip` binary

The library is the product; the CLI is a separate, deliberately thin concern. It adds **no
dependency** (the argument surface is a small hand-written scan, not a parser) and holds no rule
logic.

1. `crates/muhtesip-cli/src/main.rs` — read each argument (a file, `-` for standard input, or a directory, which expands
   to the workflow files it holds; no arguments means standard input), call `muhtesip::lint`, print one
   line per finding, choose an exit code.
2. Output line: `{path}:{line}: {severity}: {message} [{rule}]`, one per finding, in the order
   the library returns them (already sorted by line, then rule).
3. Exit codes, as named constants (protocol, not policy):
   - `0` — no finding reached the fail threshold.
   - `1` — at least one finding was at or above it. The threshold defaults to `warning`, so advisory
     `note` output does not fail a build.
   - `2` — a read, parse, or usage failure. Distinct from `1`: "your workflow has problems"
     and "I could not read your workflow" must not be the same signal to a CI script.
   Which severity fails a build is **the caller's policy**, not the library's: severity is modelled in
   the library, the threshold is chosen by `--fail-on error|warning|note` here.
4. Errors go to **stderr**, prefixed `muhtesip:`; findings go to **stdout**.
5. Color is **presentation, never content**: the same line, with the severity in a terminal color when
   the destination is a terminal — and not under `NO_COLOR`, and not on `TERM=dumb`.
   `CLICOLOR_FORCE` asks for it anyway. Piped and redirected output is byte-identical to before, and no
   machine format ever carries an escape: `json`'s `text` field is the plain line whatever terminal the
   same run happens to be printing to.
6. Argument forms: `--flag=value` is the same request as `--flag value`; `--` ends the options, so a
   source after it is a source whatever it looks like; an unknown option names the flag it resembles
   instead of leaving the caller to guess; a value on a flag that takes none is refused rather than
   ignored.

### Definition of done (slice 2)

- `crates/muhtesip/tests/cli.rs` spawns the built binary (`CARGO_BIN_EXE_muhtesip`) and asserts exit codes and
  output for: a clean file (0, silent), a file with findings (1, one line each), malformed
  input (2), and a missing file (2).
- The library is unchanged by this slice except for `Severity`'s `Display` impl.
- The three gates pass.

## Definition of done (slice 1)

- `cargo test --all-features` passes, including the two fixture tests.
- `cargo clippy --all-targets --all-features -- -D warnings` is clean.
- `cargo fmt --all -- --check` is clean.
- The library prints nothing and takes no `&mut self`.
