# Integration

Two halves, and they are independent — you can adopt either without the other.

- **`rekall check` in your gate.** Refuses a half-done extraction at commit
  time. This is the half that keeps the tool honest.
- **`rekall hook` in your harness.** Delivers a rule at the moment it applies
  instead of holding it in the window all session.

## Putting `rekall check` in a gate

`check` is CPU-only and offline. It needs no key, no network and no daemon,
which is what makes it legal in a pre-commit hook at all.

It exits `0` when every extraction is complete, `1` on drift, `2` on a usage
error. Drift means one of: a mechanical rule whose runner is still the
generated placeholder, a situational skill with an empty or absent trigger, a
skill with no do-not-fire clause, an orphan artifact, or a source span that was
supposed to be deleted and is still there.

Two wirings are given below and no others, because these are the two that have
actually been run. A config snippet nobody executed is a claim, and this is the
wrong project to publish untested claims in.

### hk

What this repository uses, and what the rest of the Rust side of the fleet
uses:

```pkl
["rekall-check"] {
  glob = List("**/CLAUDE.md", "**/AGENTS.md", ".rekall/**", ".claude/skills/**")
  check = """
    rekall check || { echo 'An extraction is unfinished. Each line above names the fix.' >&2; exit 1; }
    """
}
```

### Plain git

The fallback that needs no runner at all:

```sh
#!/bin/sh
# .git/hooks/pre-commit
exec rekall check
```

Verified in both directions: a complete extraction commits, and an extraction
with an unfilled trigger exits 1, prints the two problems and leaves the tree
uncommitted.

### Anything else

`rekall check` is an ordinary command that exits nonzero, so any hook runner
that can call a command can call this one. Writing out configs for runners
nobody here has tried would be guessing at their glob syntax and their
pass-the-filenames semantics, and getting either wrong costs a reader more than
the missing snippet does.

### CI

Nothing special. `rekall check` in a job; a nonzero exit fails it. Use
`--format json` if something downstream consumes the result:

```sh
rekall check --format json > rekall-check.json || true
```

Note the `|| true` there is for *capturing* the report; drop it if the job
should fail, which it usually should.

## Putting `rekall hook` in a harness

`hook` reads a harness hook payload as JSON on stdin and writes a decision as
JSON on stdout. It is an adapter, not a daemon and not an interceptor.

For Claude Code, in `.claude/settings.json` (project) or
`~/.claude/settings.json` (user):

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          { "type": "command", "command": "rekall hook", "timeout": 5 }
        ]
      }
    ]
  }
}
```

What it does on each call:

- Matches the situation against every extracted artifact's trigger.
- Injects the payload of each situational skill that matched.
- Runs the runner of each mechanical rule whose trigger matched, and passes its
  output through as advice.
- Counts the fire. **That counter is the only thing it writes.**

What it does not do:

- **It never blocks a tool call.** A mechanical rule that fails advises; the
  decision stays with the harness and the human. A tool that sits in the
  request path of every call and can veto is a tool one bad rule turns into an
  outage.
- **It never reaches the network**, because nothing in this crate does.
- **It never fails loudly on a payload it cannot read.** An unparsable payload
  yields an empty decision, not a crash. The blast radius of panicking there is
  every tool use, not one report.

### Codex

Same adapter, one flag, because the two harnesses read different keys:

```
rekall hook --agent codex
```

Codex `PreToolUse` accepts `systemMessage` and ignores `additionalContext`,
so without the flag rekall emits the Claude Code envelope and Codex loads
nothing — exit 0, silently. That is the failure this flag exists to remove.

The name is **named, never sniffed**. Codex sends `turn_id` and
`permission_mode` where Claude Code sends neither, so detection would rest on
a field's absence, and absence is what the next release changes. Omitting
`--agent` means `claude`, which keeps every hook line already in the field
working. An unknown name is exit 2, never a fall back.

Only `PreToolUse` is rendered for Codex, because it is the only pair anyone
here has run. At any other event rekall prints a valid empty decision, exits
0, and says on stderr which skills it did not deliver and why. It will not
guess at an envelope: a shape guessed right looks exactly like one guessed
wrong, until the day it silently drops a skill.

`rekall catch` needs no flag. It reads every known transcript root, and a
file under `~/.codex/sessions` is a Codex rollout *by location* — a fact
about where it sits rather than a guess about what it holds.

### Other harnesses

rekall reads the same payload fields from both, so only the REPLY differs.

The payload fields `hook` reads are `hook_event_name`, `tool_name`,
`tool_input.file_path`, `tool_input.notebook_path`, `tool_input.command`,
`cwd` and `prompt`. Everything else is ignored rather than rejected, so a
harness that grows a field does not break the adapter. If your harness spells
these differently, the adapter is the place to translate — `rekall hook` is
deliberately the only part that knows a harness exists.

## Checking the wiring without a harness

`recall` takes the situation as arguments and prints what `hook` would decide:

```console
$ rekall recall "ALTER TABLE orders ADD COLUMN region text" \
    --tool Edit --path db/migrate/003_orders.sql
skip    c931906  M1  .rekall/rules/never-commit-a-env-file.sh  (trigger did not match)
load    8bd8468  S2  .rekall/skills/when-a-migration-touches-a-table/SKILL.md  (trigger matched)
```

One matcher sits behind both verbs, so what `recall` prints is what `hook`
decides. That is an invariant rather than a coincidence, and it is one the
project has already broken once: `recall` said `load` and `hook` said nothing
loads, on the same input, because the two were being handed different
*situations* rather than running different matchers. It is `§B7`, and it is why
`recall` exists as a debugging surface at all.

## Runner timeout

Mechanical runners are bounded by **wall-clock** time, configurable in
`rekall.toml`:

```toml
[triggers]
runner_timeout_ms = 2000
```

Wall-clock is a blunt instrument and the number exists because of it. Under
contention a rule costing milliseconds of CPU can exceed the bound and be
killed, so a loaded machine can manufacture a timeout that never really
happened (`runner:B5`). The bound is configurable precisely so you can answer
that, and a limit nobody can raise is one that lies about what happened.

So raise it when your machine is loaded as readily as when a runner genuinely
does more work — the two are indistinguishable from here. A CI box running
the suite in parallel is the common case, and 2000 is chosen for a quiet
laptop rather than that.

An earlier version of this page claimed the bound was CPU time and that
wall-clock "was the original bound". It was not true: the fix for `B5` made
the bound configurable and never changed what it measures. It is recorded as
`runner:B16` rather than quietly corrected, because a document that overstates
a repair is how the repair stops being made.

## Pointing it at a scratch corpus

`REKALL_HOME` replaces `HOME` when the corpus resolves a `~` root. Your
config is still found under the real `HOME`; only the corpus moves:

```sh
REKALL_HOME=/tmp/scratch rekall scan
```

This exists because this tool's corpus can be your agent memory, and running
a destructive experiment against the data you are studying is the one way to
lose it. Use it whenever you are trying something out against a memory root.

An empty value is treated as unset, the same trap `XDG_CONFIG_HOME=""` sets
one module over — an empty override would resolve `~/x` to `/x`.

**It does not make your real corpus safe.** Forget the variable and you get
the ordinary behaviour. The guard that matters is still `rekall plan`, which
names every file before `apply` touches one.


## What `rekall` will not do to your repository

- It does not write to a memory directory unless `apply` named that file.
- It does not put a home path into a tracked config. `init` finds your user
  roots, names them in its output, and leaves them out of the project file,
  because that file follows the repo into every checkout.
- It does not mutate anything from a report-only verb. `scan`, `show`, `check`,
  `recall` and `log` do not write; `plan` writes only its own plan file.
- It does not execute anything except the runners under the rules directory
  that the ledger names. Those are your scripts, run with your permissions —
  see [`SECURITY.md`](SECURITY.md), which states that as design rather than as
  an oversight.

## This repository's own gate

`rekall` is consumer #0: it runs against its own `CLAUDE.md`, and
`rekall check` is a step in its own gate. That is not a demo. A tool that gates
other people's extractions and not its own is a tool nobody has run in anger.

One definition, three callers: [`hk.pkl`](https://github.com/pr0d1r2/rekall/blob/main/hk.pkl) holds the steps,
`pre-commit` takes the fast 21, `pre-push` and
[`ci.yml`](https://github.com/pr0d1r2/rekall/blob/main/.github/workflows/ci.yml) take all 27. The split is by cost — a
sixty-second step on every commit is a step someone learns to bypass, and a
bypassed hook is worse than none.

## Known gaps

Named rather than left implicit:

- **`x86_64-darwin` is supported and not built in CI.** There is no free
  x86_64 macOS runner; it is exercised locally.
- **The fire counter is per-checkout.** `.rekall/fires` is untracked by design,
  so `--dead` answers for your working copy and not for your team. Aggregating
  it is not solved.
- **Extracting a rule does not wire it into your gate for you.** `plan` names
  the wiring step and `apply` writes the artifact; adding the script to your
  own gate definition is still yours to do.
