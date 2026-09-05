# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Version ladder

A minor version here is a level of **guarantee**, not a feature count. Each rung
answers one question: *what can you rely on at this tag?*

| version | what you can rely on | status |
|---------|----------------------|--------|
| `0.1` | it builds reproducibly on every supported platform, and the gate runs | reached |
| `0.2` | every verb but `catch` does its job end to end, and the tool runs against its own corpus | **current** |
| `0.3` | second intake: `catch` mines a transcript, so a violation seen at turn 200 outlives the session | planned |
| `0.4` | the diagnostic half — `init` names the first cut, `--dead` answers across a team rather than one checkout | planned |
| `1.0` | the contract frozen: the CLI surface, the JSON anatomy, the trigger format and the ledger schema | planned |

Pre-1.0 SemVer permits a minor to break, and here each rung *is* a behaviour
change, so that permission is used honestly rather than worked around.

This ladder is not yet `§V` law in [`SPEC.md`](SPEC.md); it records the rungs as
they were actually earned. Promoting it to an invariant is spec work and goes
through `/spec`.

Nothing has been published to crates.io yet.

## [Unreleased]

Documentation caught up with the code. The repository now carries the
`docs/` set the rest of the fleet has — code of conduct, contributing,
security policy, third-party notices, integration — plus a worked example in
the README that is real command output rather than an illustration.

### Fixed

- `typos --write-changes` renamed **HashiCorp** to "HashCorp" inside the
  third-party trademark notices and committed the result. The word list now
  carries the proper noun. The underlying shape — a gate step that rewrites
  rather than reports, so an unrecognised name is silently changed instead of
  adjudicated — is a spec matter and is recorded as such.

## [0.2.0] — 2026-09-05

**The working rung.** Every verb but `catch` does its job end to end, and the
loop closes on this repository's own `CLAUDE.md` as consumer #0.

### Added

- `init` — detects corpus roots and writes `rekall.toml`, refusing to clobber
  an existing one without `--force`. User roots are named in the output and
  deliberately left out of the tracked project file.
- `scan` — one row per statement: id, source span, tokens, class, sharpness,
  signals. Deterministic and report-only.
- `show` — one statement in full, with every signal that fired and its weight.
- `plan` — the extraction diff, with the net token result named *before* the
  move. `--out` makes it a reviewable artifact; `apply` refuses a stale one.
- `apply` — writes the artifact, deletes the source span, leaves a pointer.
  Confirms on a tty and demands `--auto-approve` off it.
- `check` — the gate. Exits 1 on a rule with no runner, a skill with no trigger
  or no do-not-fire clause, an orphan artifact, or a span that should have gone.
- `recall` and `hook` — one matcher behind two front doors, with the trigger
  format as fenced `rekall` TOML blocks.
- `log` and `log --dead` — the ledger, fire counts, and what has never fired.
- `revert` — reverses one extraction verbatim from the ledger.
- `itok` delegation for every token column. This project does not write a
  second counter.
- Classification by weighted signals with a deadband, including mood signals
  (imperative, absolute quantifier, `, not ` contrast) scoped to list items.
- `[triggers].runner_timeout_ms`, defaulting to 2000.

### Fixed

- Runners were bounded by wall-clock rather than CPU time, so under contention
  a rule costing milliseconds was killed. Two of three full test runs failed
  while the same tests passed alone in one to two seconds — meaning every
  green gate for that stretch rested on a suite failing two runs in three.
  (`§B5`)
- `apply` prefixed only the first line of a quoted statement, so every wrapped
  bullet put prose into a shell script as code. 509 tests were green; every
  fixture was one line, and the defect appeared on the first statement a human
  actually wrote. (`§B6`)
- `hook` built the situation text from `prompt` alone, so a `word` trigger
  could never match a tool call — the only event `hook` runs on. `recall` said
  the skill would load and `hook` said nothing loads, on the same payload.
  (`§B7`)
- `log` reported gross statement tokens as reclaimed while `apply` wrote a
  pointer back into the same file, so two real extractions each made the corpus
  bigger and the column said smaller. `log` now reports the net, and `plan`
  names it before the move. (`§B3`)
- `recall` and `hook` reported a generated mechanical artifact as "trigger
  could not be read". A missing block is gate-only, which is legal. (`§B4`)
- The token scratch directory was named by PID alone, so concurrent counts in
  one process deleted each other's files mid-count — visible only under load.
  (`§B1`)
- A gate message wrote its own advice in backticks inside a double-quoted shell
  string, so the shell executed it and printed `command not found` where the
  advice should have been. (`§B2`)

### Changed

- The pointer left behind shrank to `<!-- rekall <id> -->`, and `revert`
  locates by id rather than by artifact path.
- `cli.rs` split per verb, with a module-size limit and a runner enforcing it.

## [0.1.0] — 2026-08-21

**The scaffold rung.** The crate builds reproducibly on every supported
platform and the gate runs. No verb does its job yet — each is recognized and
reports that it is unimplemented, which is deliberately not the same as being
unknown.

### Added

- Root `flake.nix` covering all four supported systems, `Cargo.toml` with its
  own lints from commit one, vendored `hk.pkl` schema, MIT licence, the ASCII
  gate, and `mth` checking this crate's own spec.
- `.context-limits` with a ceiling per path and a runner enforcing it.
- A clippy deny list with no allow-list and no lint-debt file — affordable
  exactly once, at zero lines of code.

[0.2.0]: #020--2026-09-05
[0.1.0]: #010--2026-08-21
