# Contributing

## The loop

```sh
direnv allow      # or: nix develop
hk check          # the whole gate, the same definition CI runs
```

Entering the dev shell installs `pre-commit` and `pre-push` and puts `rekall`,
`mth`, `itok` and `hk` on PATH. There is no second install path and no list of
linters to set up: [`hk.pkl`](../hk.pkl) is the one definition, 20 steps on
commit and 26 on push, and [`ci.yml`](../.github/workflows/ci.yml) calls the
same one. A laptop and a runner cannot disagree about what the gate is.

If a step is red, read it. Every failing step names the fix, not only the
breach — that is itself a gate step, so a message that only says "failed" is a
bug you can report.

## Decide in the spec before building

[`SPEC.md`](../SPEC.md) is the law rather than a description written afterwards.
Work goes into it before it goes into `src/`.

- `§V` invariants are what must stay true, each carrying the reasoning it
  stands on. A rule without its reasoning cannot be knowingly reversed later,
  only archaeologically.
- `§T` tasks are what is decided and what is open.
- `§B` bugs are every defect found so far, paired with the invariant that now
  catches it.
- `§R` research holds the measurements the constraints rest on.

The spec is edited through the `/spec` command and **never by hand**, and its
format is enforced by `mth` in the gate. A judgment records what it rejected and
what would reverse it.

## When a test fails, decide which kind first

A failing test is either a **code bug** or a **spec gap**, and the two have
different fixes.

- A code bug: fix the code. The invariant that caught it already exists.
- A spec gap: the invariant that would have caught it does not exist yet. It
  goes into `§B` with its cause, and produces or cites a `§V`, **before** the
  code fix lands.

Reaching for the code first on a spec gap produces a repository where the same
class of defect recurs with a different surface each time and nothing accretes.
This is the one process rule worth being pedantic about.

## Commits

One decision per commit, and the reasoning goes in the message rather than in a
comment nobody will find. The message says what was rejected and why, not only
what changed — `git log` is the audit trail for decisions that have no other
home.

Work goes straight to `main`. There are no feature branches in this repository;
the gate is the review.

## What the gate will not let you do

Stated plainly so they are not discovered one at a time:

- **You may not raise a threshold to make a build pass.** The numbers in
  [`clippy.toml`](../clippy.toml), the coverage floor in
  [`.coverage`](../.coverage) and the ceilings in
  [`.context-limits`](../.context-limits) are reviewed decisions. Raising one
  reflexively trains the reflex those numbers exist to catch. Raising one
  deliberately is allowed and requires a commit message that says why.
- **The coverage floor only rises.** The `fix` half refuses to record a drop.
  If coverage fell, cover the gap.
- **`allow` must name what it exempts and why.** An exemption is not a
  suppression.
- **Rust source is ASCII only.** The spec's symbols are format, not source.
- **Token counting belongs to `itok`.** Do not write a second counter.
- **Do not add a dependency for something the standard library already does.**
  [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md) records the argument each
  existing one had to win.

## Reporting a bug

Open an issue with what you ran, what happened, and what you expected.

**Do not paste a real corpus.** This tool reads agent memory and
`CLAUDE.md`-class files. Reduce the input to the smallest statement that
reproduces the behaviour; if you cannot share it at all, describe its shape —
how many statements, where the lines wrap, whether a pointer sits nearby — and
a fixture gets built from that. See [`SECURITY.md`](SECURITY.md) for anything
that should not be public in the first place.

Two report shapes are especially useful:

1. **A statement classified wrongly.** Send the statement, the class `rekall`
   gave it, and the class you would give it. `rekall show <id>` prints every
   signal and its weight, which is the actual argument.
2. **A `check` that passed when it should not have.** A green gate on a
   half-done extraction is the failure this project exists to prevent, and is
   treated as seriously as data loss.

## Reviewing

A claim carries evidence. "This is slower" invites a measurement; "this class
is wrong" invites the statement and the class you would give it instead. That
applies to maintainers too — see [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
