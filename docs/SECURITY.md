# Security policy

## Reporting a vulnerability

Report privately, not in a public issue.

- Email **pr0d1r2@gmail.com** with `rekall security` in the subject.
- Or open a **confidential issue** on the project, if you have access to it.

Please include what you ran, what happened, and the input that triggered it.

**Do not attach a real corpus.** This tool reads agent memory and
`CLAUDE.md`-class files, which are private by nature. Reduce the input to the
smallest statement that reproduces the behaviour. If you cannot share the
input at all, describe its shape — how many statements, how the lines wrap,
where the pointer sits — and we will build a fixture from that. A report we
have to reconstruct is better than a disclosure you cannot take back.

Expect an acknowledgement within a week. If a report is valid, the fix and the
advisory go out together, and you are credited unless you ask otherwise.

## Supported versions

Pre-1.0, only the latest published version is supported. There are no backports
to earlier minors — see [`CHANGELOG.md`](../CHANGELOG.md) for what each rung
means.

## What the attack surface actually is

Worth stating plainly, because the shape is unusual and it changes what is
worth reporting:

- **No network. At any flag.** Not an opt-out — there is no code path to opt
  out of, and no dependency that opens a socket. Nothing is fetched, resolved
  or phoned home at any point.
- **No model, no API key.** Classification is a deterministic weighted
  signal sum. There is no remote to compromise and no prompt to inject through.
- **No `unsafe`.** `unsafe_code = "forbid"` in `Cargo.toml`, so it cannot be
  reintroduced locally — memory-safety bugs are not representable here.
- **Five direct dependencies**, all of them parsers or directory walkers, none
  of them optional, none with a network feature enabled. See
  [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md).

So the realistic classes, in the order they matter:

1. **A write that loses or corrupts a corpus.** `apply` deletes a span from
   your `CLAUDE.md` and `revert` restores it from the ledger. If a span is
   deleted and the ledger cannot restore it byte-for-byte, that is the worst
   thing this program can do to you, and it is a vulnerability in the only
   sense that matters here — your file. Report these first and say so.
2. **A leak of corpus content into somewhere it does not belong.** A user root
   written into a tracked `rekall.toml`, private text copied into a generated
   artifact that lands in a shared directory, a statement echoed into a log
   somebody else reads. There is no network path, so every leak here is a
   local one, and local is where the corpus lives.
3. **A gate that silently passes.** `rekall check` reporting clean on an
   extraction that is not finished — a rule with a placeholder runner, a skill
   with an empty trigger — means a rule left the model's window and nothing
   replaced it. That is strictly worse than never having extracted it, and is
   treated as a defect of the same seriousness as data loss.
4. **A crash or hang on hostile input.** Panics are denied by lint, but a
   pathological corpus reaching an unwrap-free path and still aborting, or one
   causing unbounded time or memory, is worth reporting. `hook` runs in the
   request path of every tool call, so a hang there stalls the harness rather
   than one report.

## Something that is deliberately not sandboxed

`rekall` executes the runner scripts under `.rekall/rules/` when `hook` fires a
mechanical rule. **Those are shell scripts from your own repository, run with
your own permissions.** That is the feature: a rule that cannot run a check
cannot gate anything.

It means a hostile `.rekall/rules/*.sh` is a hostile shell script in your
checkout, with everything that implies. Treat that directory as code, review it
in pull requests the way you review code, and do not run `rekall hook` against a
repository you would not run `make` in. Reports that a crafted rule file can
execute commands describe the design rather than a defect; reports that
`rekall` can be induced to run a script from *outside* that directory, or one
the ledger does not name, are real and wanted.

Runners are bounded by CPU time rather than wall-clock, with the limit
configurable as `[triggers].runner_timeout_ms`. A runner that ignores the bound
is a defect.

## What is out of scope

- The absence of a feature, or a classification you disagree with. That is
  [`SPEC.md`](../SPEC.md) and an ordinary issue.
- A rule you wrote that does not catch what you wanted. `rekall` gates that a
  runner *exists* and *exits nonzero*; what it checks is yours.
- Third-party advisories against a dependency with no reachable path from this
  crate. Report them anyway if you are unsure — say which call reaches it.
