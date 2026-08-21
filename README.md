# rekall

Mine agent memory and `CLAUDE.md`-class prose for statements that should have
been code, extract them, and gate that the extraction stayed honest.

CPU only. Offline. No model, no network, no API key.

## The problem

A rule a model must REMEMBER is already lost. Prose stated at turn 3 competes
with everything after it, dies at compaction, and costs window on every turn
it does not fire. A rule that arrives at its trigger point uninvited costs
nothing until it matters.

So `rekall` sorts always-on prose into two kinds of tangible:

- **Mechanical** statements become a rule with a runner that exits nonzero.
- **Situational** statements become a skill with a trigger, and an explicit
  clause for when it must NOT fire.

Anything it cannot classify stays `U` — unknown, and legal. A classifier that
never says "I do not know" is lying at a fixed rate.

## Status

**0.1.0 is the scaffold rung.** The crate builds reproducibly on every
supported platform and the gate runs. No verb does its job yet: each one is
recognized and reports that it is unimplemented, which is deliberately not the
same as being unknown.

The design is settled and written down. `SPEC.md` is the source of truth —
28 invariants, each carrying the reasoning it stands on, and a task list that
says what is decided and what is still open.

## Verbs

```
rekall init     detect corpus roots and write rekall.toml
rekall scan     inventory the corpus, one row per statement
rekall show     one statement or artifact in full
rekall plan     diff an extraction, optionally into a plan file
rekall apply    execute an extraction; confirms before it mutates
rekall check    the gate -- exit 1 on drift
rekall recall   which situational skills load here
rekall hook     harness hook JSON on stdin, decision JSON out
rekall log      read the ledger
rekall catch    mine a transcript for candidate statements
rekall revert   reverse one extraction, verbatim
```

`plan` and `apply` are terraform's split, borrowed with its obligations: a
plan is a reviewable artifact, a stale plan is refused rather than executed,
and `apply` confirms before it touches anything.

Every verb offers `--format human` and `--format json` with the same anatomy.
An agent should never have to parse prose.

## Safety

The corpus is private. Agent memory and `CLAUDE.md` hold user facts, so:

- **Nothing egresses.** Not to a LAN host, not behind an opt-in flag. There is
  no network path in this crate at all.
- **Extraction is reversible.** The ledger keeps the source path, the line
  span and the original text, so `revert` is mechanical replay rather than a
  rewrite.
- **Memory directories are read-only** unless `apply` named that file.

## Development

```sh
direnv allow      # or: nix develop
hk check          # the whole gate, the same definition CI runs
```

The dev shell pins the toolchain and puts `rekall` and `mth` on PATH, so the
crate can check its own spec and its own corpus without a global install.

## Platform support

Supported: `aarch64-darwin`, `x86_64-darwin`, `x86_64-linux`, `aarch64-linux`
— the fleet's `nixpkgs-lock` set.

**`x86_64-darwin` is supported but NOT built in CI.** GitHub offers no free
x86_64 macOS runner, so that target is exercised locally and not on every
push. Said plainly rather than left implicit: an untested platform claim is
worse than an absent one only when nobody says it is untested.

## License

MIT.
