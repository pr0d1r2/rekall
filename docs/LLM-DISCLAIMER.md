# Built by an LLM, deliberately and in the open

This repository — code, spec, tests and prose — was written by
[Claude Code](https://claude.com/claude-code) running Anthropic's **Claude
Opus 5**. Every commit in this repository carries a
`Co-Authored-By: Claude Opus 5` trailer -- not most of them, all of them,
which `git log --format=%B | grep -c Co-Authored-By` will confirm against
`git rev-list --count HEAD`. A human owns every decision, reviews every
diff, and is accountable for what ships.

That is the disclaimer. The rest of this file is why it is a design note
rather than an apology, and what you can check for yourself.

## Why say it at all

Two failure modes make this worth stating in the open.

The first is the obvious one: an LLM writes plausible code, and plausible
is not correct. A reader who does not know how a repository was produced
cannot calibrate how hard to look at it.

The second is subtler and is the one this project is actually about. A
model that writes a rule into a prose file has not made the rule hold. It
has made a *claim* that the rule holds, in a document that competes for
attention with everything else in the window. The gap between "written
down" and "enforced" is where this project lives, so a repository built by
a model that could not close that gap in its own working file would be
arguing against itself.

So the standard here is: **a claim in prose is a defect until something
executes it.**

## The method is spec-driven development

[`SPEC.md`](../SPEC.md) is the law rather than a description written
afterwards. It carries:

- **71 `§V` invariants** — what must stay true, each with the reasoning it
  stands on rather than the rule alone.
- **`§T` tasks** — 32 landed, 10 open. What is decided and what is not is
  visible without reading the commit log.
- **9 `§B` bugs** — every defect found so far, paired with the invariant
  that now catches it. A bug that produced no invariant is a bug that will
  return.
- **`§R` research** — the measurements the constraints rest on, each row
  citable by an outside reader or explicitly marked `internal` when it
  cannot be.

A rule and its checker land in the same commit, because a rule with no
runner gates nothing.

The spec is edited through a single command and never by hand, and the
format is checked mechanically by [`microlith`](https://github.com/pr0d1r2/microlith)
in the gate — so "the spec says so" is a statement about a file that was
parsed, not about one that was skimmed.

## The guardrails are git hooks that also run on CI

Entering the dev shell (`nix develop`, or `direnv allow`) installs
`pre-commit` and `pre-push`, which run [hk](https://github.com/jdx/hk)
against one definition of the gate in [`hk.pkl`](https://github.com/pr0d1r2/rekall/blob/main/hk.pkl): **27 steps on
commit, 33 on push**, the slow half adding doctests, rustdoc, the
no-default-features build, the packaged tarball and coverage.
[`ci.yml`](https://github.com/pr0d1r2/rekall/blob/main/.github/workflows/ci.yml) calls that same definition on three
platforms, so a laptop and a runner cannot disagree.

Some numbers, current as of the commit that carries this file:

| | |
|---|---|
| Tests | 567, plus 12 integration and 1 doctest |
| Coverage | 99.15%, against a floor of 99.26% that may only rise |
| Direct dependencies | 5 |
| `unsafe` | `forbid`, so it cannot be reintroduced locally |
| Network calls | none, at any flag |

The coverage floor ratchets and the gate refuses to record a drop. That
mechanism was itself broken once and is described below.

## The record is deliberately unflattering

Four entries from `§B`, chosen because each one is a case of the gate being
green while something was wrong. They are the reason to trust the process
somewhat and the numbers above rather less.

**`B6` — 509 green tests, and the defect appeared on the first statement a
human actually wrote.** `apply` prefixed only the first line of a quoted
statement, so every wrapped bullet put prose into a shell script as code.
Every test used a one-line fixture. The suite was not weak; it was
uniformly wrong about what real input looks like.

**`B5` — every gate-green result for a stretch of work rested on a suite
that failed two runs in three.** A rule's runner was bounded by wall-clock
rather than CPU, so under contention a rule costing milliseconds was
killed. The same tests passed alone in one to two seconds. A timeout that
depends on how busy the machine is does not measure the thing it claims to.

**`B3` — two real extractions each made the corpus bigger, and the column
said smaller.** `log` reported gross statement tokens as reclaimed while
`apply` wrote a pointer back into the file eight lines away in the same
module. The tool whose entire purpose is context economy was reporting a
saving it was not delivering.

**`B7` — `recall` said the skill would load; `hook` said nothing loads, on
the same input.** Two views of one matcher, each correct alone. The
divergence only shows where a skill fails to load in production while the
report swears it would.

None of these were found by review of the diff. Each was found by running
the thing, which is the argument for the gate rather than against it — and
also the argument for reading `§B` before trusting `§V`.

## What a reader should actually check

If you are evaluating this repository, these are the load-bearing
questions, in the order they matter:

1. **Does the gate run for you?** `nix develop` then `hk check`. If a claim
   here is false, that is where it shows.
2. **Does `§B` look like a real bug log or a curated one?** Four of its
   seven rows are above, unedited. Judge the other three.
3. **Do the `§V` invariants have runners?** A `§V` row that nothing
   executes is exactly the defect this project exists to remove, and it
   would be embarrassing here specifically.
4. **Is `§R` checkable?** Rows carry public URLs where an outside reader
   can verify them and are marked `internal` where they cannot. A row that
   claims a measurement with no way to reach it is worth less than no row.

## Accountability

The human named in [`LICENSE`](../LICENSE) is responsible for this code,
including the parts a model wrote and the parts nobody caught. "The LLM
wrote it" is an explanation of provenance, never a transfer of
responsibility.

Bug reports are welcome and unflattering ones are more useful — see
[`SECURITY.md`](SECURITY.md) for the ones that should not be public, and
[`CONTRIBUTING.md`](CONTRIBUTING.md) for everything else.
