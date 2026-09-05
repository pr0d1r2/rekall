# Third-party notices

`rekall` ships **five direct dependencies**, which pull in 26 crates in total.
This file says what they are, why each direct one is there rather than
hand-rolled, and what licence the set carries.

## Why there are any at all

The project's own constraint is *do not add a dependency for something the
standard library already does*. These five are the ones that survived it:

| Crate | Version | Why not hand-rolled |
|---|---|---|
| [`serde`](https://crates.io/crates/serde) | 1.0.229 | Derive for the config, ledger and plan types. Writing the deserializers by hand would be more code and no better. |
| [`toml`](https://crates.io/crates/toml) | 0.9.12 | TOML is a real grammar with a specification. Hand-rolling a parser for it is the reuse-before-building failure this project's own rules name, for no gain. |
| [`serde_json`](https://crates.io/crates/serde_json) | 1.0.151 | Every verb offers `--format json` with the same anatomy as its human form, and `hook` speaks JSON on both ends. That is contract surface, not convenience. |
| [`globset`](https://crates.io/crates/globset) | 0.4.20 | Corpus roots and trigger `path` keys are globs. Matching them correctly — including the `**` cases — is a solved problem with specified behaviour. |
| [`walkdir`](https://crates.io/crates/walkdir) | 2.5.0 | Symlink-loop detection specifically. A hand-rolled recursion over a memory directory containing a self-referential link would hang rather than fail. |

None of them is optional, none has a network feature enabled, and both parser
crates come from ecosystems that are specified and heavily exercised. The build
runs offline from the tracked `Cargo.lock`.

## The full set

Every crate reachable from a normal (non-dev, non-build) dependency edge:

| Crate | Version | Licence |
|---|---|---|
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| bstr | 1.13.1 | MIT OR Apache-2.0 |
| globset | 0.4.20 | Unlicense OR MIT |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| log | 0.4.33 | MIT OR Apache-2.0 |
| memchr | 2.8.3 | Unlicense OR MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| same-file | 1.0.6 | Unlicense OR MIT |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| serde_json | 1.0.151 | MIT OR Apache-2.0 |
| serde_spanned | 1.1.1 | MIT OR Apache-2.0 |
| syn | 3.0.3 | MIT OR Apache-2.0 |
| toml | 0.9.12 | MIT OR Apache-2.0 |
| toml_datetime | 0.7.5 | MIT OR Apache-2.0 |
| toml_parser | 1.1.3 | MIT OR Apache-2.0 |
| toml_writer | 1.1.2 | MIT OR Apache-2.0 |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| walkdir | 2.5.0 | Unlicense OR MIT |
| winnow | 0.7.15, 1.0.4 | MIT |
| zmij | 1.0.23 | MIT |

Every crate is available under MIT or a more permissive licence, so the whole
tree can be taken under MIT alongside this project. `unicode-ident` additionally
carries the Unicode licence for the character tables it embeds; that licence is
permissive and requires only that its notice travel with the data, which this
row is.

The versions above are what `Cargo.lock` resolves today. The lock file is
tracked and is the authority; this table is a readable copy of it and is
regenerated rather than edited.

## Not dependencies: the tools in the gate

Three sibling tools appear in the dev shell and in the gate, and are **not**
linked into this crate. A consumer building from crates.io never sees them.

- [`itok`](https://github.com/pr0d1r2/itok) — token counting. Reached as a
  pinned flake input and called as a binary. Token counts belong to `itok` and
  this project deliberately does not write a second counter.
- [`microlith`](https://github.com/pr0d1r2/microlith) (`mth`) — owns the
  `SPEC.md` format and checks it in the gate.
- [`hk`](https://github.com/jdx/hk) — the gate runner itself.

All three are MIT-licensed.

## Trademarks

Nominative use only; no affiliation or endorsement is implied.

- **Rust** and **Cargo** are trademarks of the Rust Foundation.
- **Linux** is a registered trademark of Linus Torvalds.
- **NixOS** and **Nix** are trademarks of the NixOS Foundation.
- **GitHub** is a trademark of GitHub, Inc.
- **GitLab** is a trademark of GitLab B.V.
- **Claude** and **Anthropic** are trademarks of Anthropic PBC.
- **Terraform** is a trademark of HashCorp, Inc. `plan` and `apply` borrow its
  split, and say so.
- **Total Recall** and **Rekall** are referenced as the source of this
  project's name; no connection to the rights holders is claimed.

## rekall itself

Everything else in this repository is licensed under the MIT License — see
[`LICENSE`](../LICENSE).
