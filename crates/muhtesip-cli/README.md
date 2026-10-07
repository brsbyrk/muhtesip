# muhtesip-cli

The `muhtesip` command line. It reads a CI workflow file, calls the
[`muhtesip`](https://crates.io/crates/muhtesip) library, prints the findings, and exits with a code that
says whether the run should fail.

```
cargo install muhtesip-cli     # installs the command `muhtesip`
```

The library is the product; this crate is a shell around it. It holds no rule and no decision the library
does not already make: it reads a source, applies an optional configuration, builds one envelope, prints
one rendering, and turns that envelope into an exit code. That is also why the entry point lives in its
own crate rather than as a `[[bin]]` of the library — the boundary keeps the claim true in the manifest,
and a dependency this side wants never lands on library consumers.

Rules, configuration, the benchmark and the design notes: <https://github.com/brsbyrk/muhtesip>.
