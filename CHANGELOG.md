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
| `0.2` | every verb but `catch` does its job end to end, and the tool runs against its own corpus | reached |
| `0.3` | second intake: `catch` mines a transcript, so a rule stated once outlives the session | reached |
| `0.4` | the HANDOVER: an extraction leaves this repo for a registry with no window where the rule is enforced by nothing, and the gate proves a skill can actually be loaded | **rc** |
| `0.5` | the diagnostic half — `init` names the first cut, `--dead` answers across a team rather than one checkout | planned |
| `1.0` | the contract frozen: the CLI surface, the JSON anatomy, the trigger format and the ledger schema | planned |

Pre-1.0 SemVer permits a minor to break, and here each rung *is* a behaviour
change, so that permission is used honestly rather than worked around.

This ladder is not yet `§V` law in [`SPEC.md`](SPEC.md); it records the rungs as
they were actually earned. Promoting it to an invariant is spec work and goes
through `/spec`.

Nothing has been published to crates.io yet.

## [Unreleased]

## [0.4.0-rc.1] — 2026-09-05

**A release candidate, for testing before the rung is claimed.** Nothing is
published to crates.io from an rc: `Cargo.toml`'s `repository` still points at
a repository that does not exist yet, and crates.io versions can be yanked but
never deleted. This is a tag to install from and run against real corpora.

Numbered `0.4.0-rc.1` and not `0.3.0-rc.1`, because `0.3.0` is already an
earned rung above and SemVer sorts a pre-release BEFORE its own version — an
rc named for `0.3.0` would ship as older than the thing it was testing.

What to exercise, in the order the defects were found: point it at a corpus
that already has extractions and run `rekall log --dead`, then `rekall check`
on a checkout where `rekall hook` is not wired, then `rekall issue --to` a
scratch directory and see whether the local copy stays live.

### Added

- `rekall issue <id>... --to <dir>` — hands a proven extraction to the loop
  that tends it. The portable `SKILL.md` goes out and **the local artifact
  stays**: removing it here would leave the rule enforced by nothing until
  the registry materialized it back, which is the gap extraction exists to
  close. The ledger row records where it went, and that record is what makes
  two copies a transition rather than a duplication.
- `rekall issue <id> --retire` — ends the handover. Drops the local artifact
  and its host link, keeps the row and its destination. Refused on a row that
  was never issued. It is human-triggered because this crate cannot observe a
  materialization it did not perform.
- `rekall issue --all` — repairs rows that already exist: a missing
  `disable-model-invocation` back in every head, and host links a fresh
  checkout never had. It never regenerates an artifact, so triggers survive.
- `rekall hook --agent codex` — the Codex dialect. Codex `PreToolUse` accepts
  `systemMessage` and ignores `additionalContext`, so before this rekall
  emitted the Claude Code envelope at a Codex hook and delivered nothing,
  silently, at exit 0. The agent is **named, never sniffed**; omitting the
  flag means `claude`; an unknown name is exit 2. Only measured (agent, event)
  pairs render — any other refuses on stderr, naming the skills it did not
  deliver, and still prints a valid empty decision at exit 0 so it cannot
  break every tool call.
- `check` reports a **dangling published link** — a `.claude/skills/<slug>`
  symlink pointing at nothing, which the host still indexes and cannot read.
  Only links into `.rekall/skills`, and silent where a ledger row already
  explains the missing file.
- `check` reports an **open handover** as a `note` rather than drift, in both
  formats. Notes travel beside the drift the exit code counts, never inside
  it. The JSON envelope gains a `notes` key.
- `check` reports an `S` artifact whose head does not disable the host's own
  loading (`no-guard`). Without that line the host loads the skill on its own
  judgement and the do-not-fire clause is bypassed.
- Agent-agnostic intake. `catch` iterates every known transcript root, so a
  Codex rollout under `~/.codex/sessions` is read by location rather than by
  a flag, and its deeper `payload` shape and `developer` role are understood.
- The spec is FEDERATED, 23 nodes, one per module.
  [`FEDERATION.md`](docs/FEDERATION.md) explains the shape.
- `docs/LLM-DISCLAIMER.md`, `AGENTS.md`, `deny.toml`, `release.toml`, and gate
  steps for links, advisories, licences and the published file set.

### Changed

- `apply` writes `S` artifacts under `.rekall/skills/` and **links** them into
  a host's own skills directory where one already exists, rather than writing
  into the host's tree. One file, two paths, no drift — and the link is only
  ever made where the directory is already there.
- The generated skill head carries `disable-model-invocation: true`.
- A ledger row carries `issued_to`. It defaults, and serialises only when set,
  so old ledgers load and unissued rows write no column.

### Fixed

- Every `S` artifact this crate had ever written was model-invocable, which
  bypassed its own do-not-fire clause on the host's path. `check` now catches
  it and `rekall issue` repairs it in place.
- `check`'s `no-head` advice used to say to revert and re-apply. That
  regenerates the artifact and discards the triggers a human filled in, which
  the ledger does not keep.

## [0.3.0] — 2026-09-05

**The second-intake rung.** Every verb the spec names now does its job; there
is no longer one that reports itself unimplemented.

### Added

- `rekall catch [<session>]` — reads a JSONL transcript and proposes what a
  human stated as a rule mid-session. A violation is a **user** turn the
  classifier does not call `U`; the assistant's own text is not evidence,
  because a model restating the rule it just broke would mint a candidate out
  of its own apology. Candidates go to the ledger, never to the corpus, and
  promotion still runs through `plan` then `apply`.
- Unreadable transcript lines are **skipped and counted**, and the count is
  printed even when it is zero. "Few candidates" and "few violations" are the
  same output otherwise.
- `cargo-deny` in the gate: `bans`, `licenses` and `sources` on commit, and
  `advisories` — the one step in the whole gate that touches the network — on
  push only.
- A relative-link check (`lychee --offline`) across every tracked file, not
  only markdown.
- `.crate-files`, recording what the published tarball ships, with a gate step
  that fails on any change to it.
- `release.toml` and `AGENTS.md`.
- `tests/cli.rs` — twelve tests over the built binary, pinning the exit codes
  and the JSON anatomy that a unit test cannot observe.

### Fixed

- The `.crate` shipped a generated skill from `.claude/skills/` while
  excluding the `.rekall/` ledger that records it — an orphan artifact, the
  exact state `check` exits 1 on, inside the tarball of the tool that refuses
  it. `cargo package` was green throughout, because a tarball with the wrong
  files in it still builds.
- `typos --write-changes` renamed **HashiCorp** to "HashCorp" inside the
  third-party trademark notices and committed the result. The word list now
  carries the proper noun; the underlying shape — a gate step that rewrites
  rather than reports — is recorded as a spec matter.
- `V44` shipped wrong for one commit: it scoped a transcript violation to the
  mood signals, which are half a directive by construction, so `catch` missed
  both "never X" and "always X" — the two forms a correction actually takes.
  Measured, recorded as `§B8`, and corrected to the class. (`§B8`)

### Changed

- Documentation caught up with the code: the `docs/` set the rest of the fleet
  carries — code of conduct, contributing, security policy, third-party
  notices, integration, LLM disclaimer — plus a README worked example that is
  real command output rather than an illustration.
- `resolve` moved out of `src/cli.rs` into `src/cli/resolve.rs` when the
  module-size gate fired at 502 lines. Dispatch and resolution are different
  jobs and the seam was already there.
- Two gate wirings were removed from `INTEGRATION.md` because nobody had run
  them. What remains is `hk` and a plain git hook, both exercised.

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

[0.4.0-rc.1]: #040-rc1--2026-09-05
[0.3.0]: #030--2026-09-05
[0.2.0]: #020--2026-09-05
[0.1.0]: #010--2026-08-21
