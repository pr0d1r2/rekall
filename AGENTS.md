# Working here

The fleet-standard entry point. It says where things live and what the loop
is; it does not restate the rules, because two copies of one rule is the
defect this tool exists to remove.

## Start here: the gate

```sh
direnv allow      # or: nix develop
hk check          # everything, the same definition CI runs
```

One definition in [`hk.pkl`](hk.pkl), three callers: `pre-commit` takes the
fast 18, `pre-push` and [`ci.yml`](.github/workflows/ci.yml) take all 24. The
split is by cost — a sixty-second step on every commit is a step someone
learns to bypass.

Every failing step names the fix rather than only the breach. That is itself a
gate step, so a message that only says "failed" is a bug, not a hint to
interpret.

## Where the rules actually are

| | |
|---|---|
| [`CLAUDE.md`](CLAUDE.md) | The working agreement — and this repo's extraction **corpus**. |
| [`SPEC.md`](SPEC.md) | The law: `§V` invariants, `§T` tasks, `§B` bugs, `§R` research. |
| [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md) | The loop, and what the gate refuses. |
| [`docs/INTEGRATION.md`](docs/INTEGRATION.md) | Putting `check` in a gate and `hook` in a harness. |

`CLAUDE.md` is doing two jobs on purpose. It is the working agreement an agent
reads, and it is the corpus `rekall` is pointed at as consumer #0 — the `M` and
`S` statements extracted from it live in `.rekall/rules/` and
`.claude/skills/`. Editing it changes what this repo gates itself with, so an
edit there is a change to the corpus, not only to prose.

That is why this file points rather than repeats. A rule copied here would be a
second always-on statement of something already stated once — the exact shape
`rekall scan` is built to find and `rekall apply` is built to remove.

## The loop

1. **Decide in the spec first.** `SPEC.md` is edited through `/spec`, never by
   hand. A judgment records what it rejected and what would reverse it.
2. **Build against it.** `/build` flips a `§T` status cell and touches nothing
   else in the spec.
3. **When a test fails, decide which kind it is before editing** — a code bug,
   or a spec gap. A gap goes to `§B` and produces its invariant *before* the
   fix lands. This is the one process rule worth being pedantic about, and
   `.claude/skills/when-a-test-fails-decide-first/` is the extracted form of
   it.
4. **One decision per commit**, with the reasoning in the message. Straight to
   `main`; there are no feature branches here and the gate is the review.

## Reproduce a verdict without hk

Each step is a plain command, so a red gate can be reproduced one piece at a
time:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
mth check SPEC.md            # the spec's own format
itok check                   # the .context-limits ceilings
rekall check                 # this repo's extractions
```

`-D warnings` goes after `--` and not in `RUSTFLAGS`, because `RUSTFLAGS`
reaches path dependencies and a sibling's stray warning would redden this gate
for code this crate does not own.

## What this crate is

A CPU-only, offline tool that turns always-on agent prose into tangibles:
mechanical statements become a rule with a runner, situational ones become a
skill with a trigger and an explicit do-not-fire clause. Everything it cannot
classify stays `U`, which is a legal answer.

There is no network path anywhere in it, at any flag. That is not a policy to
uphold — it is a property to preserve, and adding the first `reqwest` would
break four separate claims in `README.md` and `docs/SECURITY.md` at once.

## What does not belong here

- A second token counter. Counting is `itok`'s, reached as a pinned flake
  input.
- A second `SPEC.md` format implementation. The format is `microlith`'s and
  `mth` checks it.
- A dependency for something the standard library already does.
  [`docs/THIRD-PARTY-NOTICES.md`](docs/THIRD-PARTY-NOTICES.md) records the
  argument each existing one had to win.
- A threshold raised to make a build pass. Those numbers are reviewed
  decisions; raising one deliberately is allowed and takes a commit message
  saying why.

## Cross-project references

`itok` and `microlith` are siblings and are **see-also, never authority**.
`SPEC.md` may cite them as evidence in `§R`; nothing here may depend on their
internals. Both are public, so an outside reader can check a cited row — and a
row an outsider cannot check is marked `internal` and carries no name.
