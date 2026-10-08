# Third-party notices

muhtesip is original work by Baris, released under the MIT licence ([`LICENSE`](LICENSE)). It
contains one piece of code from another project, and it compiles facts published by two sources.
Both are named here, and both are permissive.

## Code: one file, ported from actionlint

`crates/muhtesip/src/glob.rs` — the filter-pattern validator — is ported from `glob.go` in
[actionlint](https://github.com/rhysd/actionlint), on 2026-10-04. actionlint is MIT licensed
(Copyright (c) 2021 rhysd), and that file carries the licence text and the two deliberate
deviations in its module doc, so the notice travels with the code it covers.

Nothing else here is ported or translated from another project. The sixteen rules, the model, the
registry, the report encodings and the CLI were written for this project.

## Facts: data tables compiled from published sources

The tables under `crates/muhtesip/src/data/` list facts about the platform: hosted runner labels,
permission scopes, retired workflow commands, trigger events and their activity types, and the
`workflow_dispatch` input cap. Each table's doc comment names its sources — GitHub's own
documentation and, for the runner labels and permission scopes, actionlint's published lists (MIT).
A list of facts is not copyrightable expression, and each table is cited so a reader can check it.

## Development material

`out/` is gitignored and is not part of the repository. It holds measurement results and, for
verifying the port, a copy of actionlint's `glob.go`.
