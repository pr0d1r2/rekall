# rekall

<!-- BEGIN badges -->
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![edition 2024](https://img.shields.io/badge/edition-2024-000000?logo=rust&logoColor=white)](Cargo.toml)
[![MSRV 1.95](https://img.shields.io/badge/MSRV-1.95-000000?logo=rust&logoColor=white)](Cargo.toml)
[![direct dependencies 5](https://img.shields.io/badge/direct_dependencies-5-brightgreen)](docs/THIRD-PARTY-NOTICES.md)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-brightgreen)](Cargo.toml)
[![network none](https://img.shields.io/badge/network-none-brightgreen)](docs/SECURITY.md)

[![gate hk](https://img.shields.io/badge/gate-hk-6E4AFF)](hk.pkl)
[![gate steps 21 commit / 27 push](https://img.shields.io/badge/gate_steps-21_commit_%2F_27_push-6E4AFF)](hk.pkl)
[![coverage floor 99.26%](https://img.shields.io/badge/coverage_floor-%E2%89%A599.26%25-brightgreen)](.coverage)
[![invariants 59](https://img.shields.io/badge/invariants-59-6E4AFF)](SPEC.md)
[![bugs logged 20](https://img.shields.io/badge/bugs_logged-20-6E4AFF)](SPEC.md)
[![federated nodes 23](https://img.shields.io/badge/federated_nodes-23-6E4AFF)](docs/FEDERATION.md)

[![nix flake](https://img.shields.io/badge/nix-flake-5277C3?logo=nixos&logoColor=white)](flake.nix)
[![intel linux](https://img.shields.io/badge/linux-5277C3?logo=intel&logoColor=white)](flake.nix)
[![amd linux](https://img.shields.io/badge/linux-5277C3?logo=amd&logoColor=white)](flake.nix)
[![arm linux](https://img.shields.io/badge/linux-5277C3?logo=arm&logoColor=white)](flake.nix)
[![arm macos](https://img.shields.io/badge/macos-5277C3?logo=arm&logoColor=white)](flake.nix)

[![built with Claude Code](https://img.shields.io/badge/built_with-Claude_Code-D97757)](https://claude.com/claude-code)
[![built with Opus 5](https://img.shields.io/badge/built_with-Opus_5-D97757)](https://www.anthropic.com/claude)
[![built with SDD](https://img.shields.io/badge/built_with-spec--driven_development-D97757)](SPEC.md)
<!-- END badges -->

Read [LLM-DISCLAIMER](docs/LLM-DISCLAIMER.md) first.

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

## Install

```sh
git clone https://github.com/pr0d1r2/rekall && cd rekall
direnv allow          # or: nix develop
cargo build --release
```

Or `nix build .#default`, the pinned package CI builds. Not on crates.io.

The dev shell pins the toolchain and puts `rekall`, `mth` and `itok` on PATH,
so the crate checks its own spec and its own corpus with no global install.

## Use

A worked example, start to finish. Every block below is real output, with only
the working directory shortened.

### The corpus

A project with four conventions written in prose, the way they usually
accumulate:

```markdown
# Working on acme

## Conventions

- Rust source is ASCII only. Unicode belongs in test fixtures, not in
  identifiers.
- Never commit a `.env` file.
- When a migration touches a table with more than a million rows, take the
  backup first and say in the PR how long the restore took.
- Prefer `anyhow` at the binary edge and `thiserror` in libraries.
```

Every one of those is always-on. All four cost window on every turn, including
the turns where none of them apply.

### `init` — find the corpus

```console
$ rekall init
wrote  ~/acme/rekall.toml
root   CLAUDE.md
absent ~/.claude/CLAUDE.md
absent ~/.claude/projects
absent AGENTS.md
absent .claude/skills

Those are USER roots. They are NOT written here -- this file is tracked, and a home path in it
would follow the repo to every checkout. Put them in `~/.config/rekall/rekall.toml`;
roots UNION across scopes, so both get scanned.
```

It found your personal memory directory and deliberately refused to write it
into a tracked file. Roots union across scopes, so nothing is lost.

### `scan` — one row per statement

```console
$ rekall scan
9a4f57e  CLAUDE.md:5-6  19  M2  contrast,only
c931906  CLAUDE.md:7-7  9  M1  never,exact-handle
8bd8468  CLAUDE.md:8-9  31  S2  when,migration
7500995  CLAUDE.md:10-10  18  S1  prefer,exact-handle
```

Four statements, four ids, four classes. The class is `M` (mechanical) or `S`
(situational); the digit is **sharpness** — how sharp a runner or trigger the
statement admits, not how confident the classifier feels. `M1` is a runner
that needs no judgement; `S1` is a trigger that is an exact tool or path.

The id is a hash of the normalized text, scoped by path. Editing a *different*
line in the file does not move it. Editing the statement itself does, and that
is correct: a reworded rule is a new claim and must be reclassified rather than
inherit a verdict passed on different words.

### `show` — argue with a class

```console
$ rekall show 8bd8468
id       8bd8468
src      CLAUDE.md:8-9
class    S2
score    -1
signals
  -1  when
  +0  migration
text
  - When a migration touches a table with more than a million rows, take the
    backup first and say in the PR how long the restore took.
```

Every signal that fired and its weight. `scan` truncates to a row, so `show` is
where a classification is disputed — a class is a claim, and a claim must be
inspectable.

### `plan` — the diff, before anything moves

```console
$ rekall plan c931906 8bd8468
c931906  M1
  delete  CLAUDE.md:7-7
  write   .rekall/rules/never-commit-a-env-file.sh
  wire    add the script to the gate so it EXITS NONZERO on a violation (V2, V22)
  net     +2 tokens
8bd8468  S2
  delete  CLAUDE.md:8-9
  write   .rekall/skills/when-a-migration-touches-a-table/SKILL.md
  wire    give the skill a trigger AND an explicit do-not-fire clause (V3, V4), AND wire `rekall hook` -- the head disables the host's own loading, so nothing loads this skill until something delivers it (V57)
  net     +22 tokens
```

`net` is the honest number: statement tokens minus the pointer left behind.
`+2` for the one-liner is a real result and a small one — the sharpest
statements are the shortest, and a short statement barely outweighs its own
pointer. When an extraction would cost more than it saves, the line says so
in those words, before the move rather than after.

`--out FILE` makes the plan an artifact: reviewable, diffable, committable
before a byte of corpus moves.

### `apply` — execute

```console
$ rekall apply c931906 8bd8468 --auto-approve
write   .rekall/rules/never-commit-a-env-file.sh
write   .rekall/skills/when-a-migration-touches-a-table/SKILL.md
edit    CLAUDE.md
done    wrote .rekall/rules/never-commit-a-env-file.sh
done    wrote .rekall/skills/when-a-migration-touches-a-table/SKILL.md
done    edited CLAUDE.md
done    recorded 2 extraction(s)
```

The corpus now reads:

```markdown
- Rust source is ASCII only. Unicode belongs in test fixtures, not in
  identifiers.
<!-- rekall c931906 -->
<!-- rekall 8bd8468 -->
- Prefer `anyhow` at the binary edge and `thiserror` in libraries.
```

Two always-on paragraphs became two comments. On a tty `apply` confirms first;
`--auto-approve` is required off-tty, so a script cannot mutate your memory by
being run without a terminal.

### `check` — the gate refuses a half-done extraction

```console
$ rekall check
no-runner           .rekall/rules/never-commit-a-env-file.sh is still the generated placeholder, so the rule extracted from CLAUDE.md gates nothing (V2). Write the check in that script and remove the line that says it is unimplemented
no-trigger          .rekall/skills/when-a-migration-touches-a-table/SKILL.md has an empty or absent `## Fires when` block, so nothing can match it -- and an unfilled trigger leaves the statement as always-on prose, which is what it was extracted FROM (V3, V4). Fill the ```rekall block under it: `tool` for exact names, `path` for globs, `word` for literals
no-refusal-clause   .rekall/skills/when-a-migration-touches-a-table/SKILL.md has an empty or absent `## Does NOT fire when` block, so nothing can match it -- and an unfilled trigger leaves the statement as always-on prose, which is what it was extracted FROM (V3, V4). Fill the ```rekall block under it: `tool` for exact names, `path` for globs, `word` for literals
undelivered         .rekall/skills/when-a-migration-touches-a-table/SKILL.md carries `disable-model-invocation: true`, so the host will not load it, and this project wires no `rekall hook` to deliver it instead -- nothing can reach the skill (V56). Wire the hook (docs/INTEGRATION.md). Only this project's settings were read, so a hook wired in your user settings is not counted here
rekall: 4 extraction problem(s). Each line above names the fix.

$ echo $?
1
```

**This is the point of the tool.** Deleting a rule from prose and writing a
placeholder beside it is strictly worse than leaving it alone: the rule is now
gone from the window *and* enforced by nothing. The gate exits 1 until the
extraction is finished, and every line names the fix rather than only the
breach.

Artifacts arrive with empty triggers on purpose. An empty trigger matches
nothing, so an unfilled skill loads never rather than always — always-on prose
is what it was extracted from.

### Finishing the extraction

The skill gets a trigger and a refusal clause:

````markdown
## Fires when

```rekall
tool = ["Edit", "Write"]
path = ["db/migrate/**"]
```

## Does NOT fire when

```rekall
path = ["db/migrate/**_test.sql"]
```
````

The rule gets a body that actually checks something:

```sh
found=$(git diff --cached --name-only | grep -E '(^|/)\.env($|\.)' | grep -v '\.env\.example')
[ -z "$found" ] && exit 0
echo "rekall: refusing to commit a .env file: $found" >&2
echo "rekall: rename it to .env.example, or add it to .gitignore" >&2
exit 1
```

```console
$ rekall check
$ echo $?
0
```

Silent and zero. The gate says nothing when there is nothing to say.

### `recall` — ask what would load, and what would not

```console
$ rekall recall "ALTER TABLE orders ADD COLUMN region text" \
    --tool Edit --path db/migrate/003_orders.sql
skip    c931906  M1  .rekall/rules/never-commit-a-env-file.sh  (trigger did not match)
load    8bd8468  S2  .rekall/skills/when-a-migration-touches-a-table/SKILL.md  (trigger matched)
```

Now the same edit against a test fixture:

```console
$ rekall recall "ALTER TABLE orders ADD COLUMN region text" \
    --tool Edit --path db/migrate/003_orders_test.sql
skip    8bd8468  S2  ...  (refused by the do-not-fire block, which WINS over a match)
```

The refusal clause is not decoration. A list of what fires says nothing about
what does not, and a matcher has to decide both.

### `hook` — the same matcher, in the harness

```console
$ echo '{"hook_event_name":"PreToolUse","tool_name":"Edit",
         "tool_input":{"file_path":"db/migrate/003_orders.sql"}}' | rekall hook
{"hookSpecificOutput":{"additionalContext":"- When a migration touches a table with more than a million rows, take the\n  backup first and say in the PR how long the restore took.","hookEventName":"PreToolUse"}}
```

The rule arrives at the moment it applies, having cost nothing until then.
An unrelated call gets nothing:

```console
$ echo '{"hook_event_name":"PreToolUse","tool_name":"Read",
         "tool_input":{"file_path":"README.md"}}' | rekall hook
{}
```

Mechanical rules fire here too, and advise rather than block:

```console
$ echo '{"hook_event_name":"PreToolUse","tool_name":"Bash",
         "tool_input":{"command":"git commit -m wip"}}' | rekall hook
{"hookSpecificOutput":{"additionalContext":"rekall: refusing to commit a .env file: .env\nrekall: rename it to .env.example, or add it to .gitignore","hookEventName":"PreToolUse"}}
```

That is the `.env` rule catching a real staged file, at the tool call, before
the commit — instead of in a paragraph the model read two hundred turns ago.

`recall` and `hook` are one matcher behind two front doors. What `recall`
prints is what `hook` decides; when that stopped being true it was a logged
bug, not a footnote.

### `log` — what fired, and what never did

```console
$ rekall log
c931906  CLAUDE.md:7-7  M1  fires=2  net=+2  .rekall/rules/never-commit-a-env-file.sh
  - Never commit a `.env` file.
8bd8468  CLAUDE.md:8-9  S2  fires=1  net=+22  .rekall/skills/when-a-migration-touches-a-table/SKILL.md
  - When a migration touches a table with more than a million rows, take the
    backup first and say in the PR how long the restore took.
```

`rekall log --dead` lists what has never fired. This is the answer to the file
that only grows: rationale decays, so nobody dares delete a rule, and the
document accretes forever. A rule that never fired is one you can drop and
*prove* you could.

It says what it measured. `fires` counts delivery by `rekall hook`; a rule
your gate runs by path is enforced without touching it. So `--dead` prints
its scope, and names nothing when nothing has been counting -- an empty
counter and an unused rule look identical in that column.

### `revert` — put it back, verbatim

```console
$ rekall revert 8bd8468 --auto-approve
restore CLAUDE.md
remove  .rekall/skills/when-a-migration-touches-a-table/SKILL.md
done    restored CLAUDE.md
done    removed .rekall/skills/when-a-migration-touches-a-table/SKILL.md
done    dropped the ledger row for 8bd8468
```

The ledger keeps the source path, the line span and the original text, so
`revert` is mechanical replay rather than a rewrite. The statement returns
exactly as it was.

### `issue` -- hand it over without a gap

```console
$ rekall issue 8bd8468 --to ../set-and-setting/skills --auto-approve
issue   ../set-and-setting/skills/when-a-migration-touches-a-table/SKILL.md
done    issued 8bd8468 to ../set-and-setting/skills/when-a-migration-touches-a-table/SKILL.md
note    the copy keeps `disable-model-invocation: true` -- right where `rekall hook` delivers the skill, wrong where nothing does. The registry decides; this end cannot see which it is.
```

The local artifact **stays**. Removing it here would leave the rule enforced
by nothing until the registry materialized it back -- the exact gap the
extraction existed to close, reopened by the step meant to finish it. So both
copies stand, the ledger row records where it went, and `check` reports the
open handover on every run as a `note`, which fails nothing.

`rekall issue <id> --retire` ends it, and it is yours to trigger: this crate
cannot see a materialization it did not perform. It drops the local copy and
keeps the row, so the record of what went where stays readable.

`rekall issue --all` repairs rows that already exist -- a missing
`disable-model-invocation` back in every head, host links a fresh checkout
never had. It never regenerates an artifact: the triggers you filled in are
the only part of that file rekall did not write.

## Verbs

```
rekall init     detect corpus roots and write rekall.toml
rekall scan     inventory the corpus, one row per statement
rekall show     one statement or artifact in full
rekall plan     diff an extraction, optionally into a plan file
rekall apply    execute an extraction; confirms before it mutates
rekall check    the gate -- exit 1 on drift
rekall recall   which situational skills load here
rekall hook     harness hook JSON on stdin, decision JSON out (--agent)
rekall log      read the ledger
rekall catch    mine a transcript for candidate statements
rekall revert   reverse one extraction, verbatim
rekall issue    hand a proven skill to the loop that tends it
```

`plan` and `apply` are terraform's split, borrowed with its obligations: a
plan is a reviewable artifact, a stale plan is refused rather than executed,
and `apply` confirms before it touches anything.

Every REPORTING verb offers `--format human` and `--format json` with the same
anatomy. `hook` is the exception and takes no flags at all -- it speaks the
harness's JSON on both ends. An agent should never have to parse prose. JSON carries `class`, `sharpness`
and `label` as separate fields, so nothing has to string-surgery `M2`.

## The lifecycle

A verb list is not a process. Extraction is the first step of four, and a
skill earns each one:

| stage | where it lives | what proves it |
|---|---|---|
| **extracted** | `.rekall/`, tracked | nothing yet -- it is a hypothesis |
| **in trial** | same | `hook` fires it; the counter climbs |
| **issued** | a registry you name, AND still here | fires, and a human agreeing |
| **retired** | the registry alone | its materializer brought it back |

It is **tracked from the moment it exists**, not hidden until it proves
itself. The audit trail is the point: this repository's history says what
was extracted and when, and the registry's says what was adopted and when.
A trial nobody can see has no history to show, and it would be hidden from
`rekall log --dead` -- the very measurement meant to end it.

Issuing is a **move**, and a staged one. Prose leaves the corpus and a
pointer stays; the artifact leaves the repository in its own time and the
ledger row stays. In between both copies stand, and the row saying where the
other went is what makes that a transition rather than the duplication this
tool exists to remove.

Nothing here reaches the network. `issue` writes to a path you name and
git does the travelling -- the corpus is private, and no verb in this
crate has a network path at all.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | ok |
| `1` | drift, or a violation the gate refuses |
| `2` | usage error |

There is no network exit code, because there is no network path.

## Safety

The corpus is private. Agent memory and `CLAUDE.md` hold user facts, so:

- **Nothing egresses.** Not to a LAN host, not behind an opt-in flag. There is
  no network path in this crate at all.
- **Extraction is reversible.** The ledger keeps the source path, the line
  span and the original text, so `revert` is mechanical replay.
- **Memory directories are read-only** unless `apply` named that file.
- **`init` never writes a home path into a tracked file.** A user root is
  named in the output and left out of the project config.

See [`SECURITY.md`](docs/SECURITY.md) for what the attack surface actually is
and how to report something.

## Guarantees

- **Deterministic.** Same corpus in, same rows out. No model decides a class.
- **Offline at every flag.** Not an opt-out; there is no code path to opt out
  of.
- **Report-only verbs never write.** `scan`, `show`, `plan`, `check`, `recall`
  and `log` do not touch the corpus. `plan` writes only its own plan file.
- **`hook` writes exactly one thing:** the fire counter. It is in the request
  path of every tool call, so it may not do more.
- **A failing gate names the fix**, not only the breach.
- **An unfilled trigger fires never**, not always.

## Status

**0.4.0-rc.1 is a candidate, not a rung.** 0.1.0 guaranteed a reproducible
build and a running gate; 0.2.0, that every verb does its job; 0.3.0, the
second way in, `catch` reading a transcript.

0.4 is the **handover**, and the rc exists to test it before it is claimed.
`rekall issue` moves a proven extraction to the registry that tends it with no
window where the rule is enforced by nothing, and `check` refuses a skill
nothing can load. An rc publishes nothing to crates.io.

The spec is FEDERATED -- 23 nodes, one per module, each owning the rules for
its own directory. [`FEDERATION.md`](docs/FEDERATION.md) explains the shape,
and why the root is a route rather than a reading.

The design is settled and written down. [`SPEC.md`](SPEC.md) is the source of
truth — 59 invariants, each carrying the reasoning it stands on; 20 recorded
bugs, each naming the invariant that now catches it; and a task list that says
what is decided and what is still open, at 23 landed and 10 remaining.

## Development

```sh
direnv allow      # or: nix develop
hk check          # the whole gate, the same definition CI runs
```

21 steps on commit, 27 on push. [`CONTRIBUTING.md`](docs/CONTRIBUTING.md) is
the loop; [`INTEGRATION.md`](docs/INTEGRATION.md) is how to put `rekall` in
someone else's gate.

## Platform support

Supported: `aarch64-darwin`, `x86_64-darwin`, `x86_64-linux`, `aarch64-linux`
— the fleet's `nixpkgs-lock` set.

**`x86_64-darwin` is supported but NOT built in CI.** GitHub offers no free
x86_64 macOS runner, so that target is exercised locally and not on every
push. Said plainly rather than left implicit: an untested platform claim is
worse than an absent one only when nobody says it is untested.

## Changelog

[`CHANGELOG.md`](CHANGELOG.md). A minor is a level of guarantee here, not a
feature count.

## Contributing

[`CONTRIBUTING.md`](docs/CONTRIBUTING.md), and
[`CODE_OF_CONDUCT.md`](docs/CODE_OF_CONDUCT.md). The short version: claims
carry evidence, and that applies to the maintainers too.

## Security

[`SECURITY.md`](docs/SECURITY.md). Report privately, not in a public issue.

## License

MIT. Third-party notices: [`THIRD-PARTY-NOTICES.md`](docs/THIRD-PARTY-NOTICES.md).
