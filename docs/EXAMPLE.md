# A measured example

The README's walkthrough uses a four-line corpus so every number fits on the
page. This is the other kind of evidence: `rekall` pointed at a real corpus
that somebody else maintains, with the commands to reproduce it.

Most of it changes nothing. `scan`, `show` and `plan` are report-only, which
is what makes them safe to run against a repository you do not own — so the
measurements below were taken without touching that corpus at all. The last
section is the exception: one statement carried all the way through, into a
merged pull request.

## The corpus

[`pr0d1r2/set-and-setting`](https://github.com/pr0d1r2/set-and-setting) at
commit `9724383`: agent skill sets written as prose, in Markdown, the way
these files actually accumulate. 195 files under `set/skills/`.

```sh
git clone https://github.com/pr0d1r2/set-and-setting && cd set-and-setting
printf '[sources]\nroots = ["set/skills"]\nglobs = ["**/*.md"]\n' > rekall.toml
rekall scan
```

## What it found

**1388 statements across 193 files, 41,986 tokens.** Every one of them is
always-on: loaded whether or not it applies, on every turn.

| class | statements | tokens | share of tokens |
|---|---|---|---|
| `M` mechanical | 688 | 24,252 | 57.8% |
| `S` situational | 87 | 2,917 | 6.9% |
| `U` unknown | 613 | 14,817 | 35.3% |

By sharpness, which is what predicts whether a runner or trigger is worth
writing:

| | 1 | 2 | 3 |
|---|---|---|---|
| `M` | 138 | 506 | 44 |
| `S` | 15 | 15 | 57 |

The shape is worth reading before the totals. `M2` dominates — 506
statements whose runner needs one human-set parameter, which is real work
per rule rather than a batch job. `S3` outnumbers `S1` nearly four to one:
most situational statements here trigger on something only a model notices,
so they are the ones a trigger block cannot capture yet. The 153 `M1` and
`S1` rows are where the cheap wins are.

## Arguing with a class

`scan` prints a row. `show` prints the argument, and the argument is where
you find out the classifier is wrong.

```console
$ rekall show d497472
id       d497472
src      set/skills/generic/skill.md:3-9
class    M1
score    2
signals
  +1  contrast
  +1  every
  +0  exact-handle
text
  The `set/skills/` tree holds this project's behavioral rules,
  one concept per file. It is structured, not flat: a top-level
  `<topic>.md` captures the core rule for a topic, and
  ...
```

**That classification is wrong, and it is in this document on purpose.**
The statement describes how a directory is organised. It states no rule, so
there is nothing for a runner to check. It scored `M1` because it contrasts
("structured, not flat"), quantifies ("every rule"), and names an exact
handle (`set/skills/`) — three signals that usually mean a mechanical rule
and here mean prose about layout.

This is what `V10` means by a class being a *claim*: the digit is a
prediction, the signals are the reasoning, and `show` exists so a human can
reject both. A classifier that never produced a row like this one would be
a classifier that had stopped saying "I do not know" — which is what the
613 `U` rows are for.

## An extraction that would pay

```console
$ rekall show ab26d76
id       ab26d76
src      set/skills/git/repo/fleet.md:26-29
class    S1
signals
  -2  prefer
  +1  use
  +0  exact-handle
text
  Use the full `https://github.com/OWNER/REPO` form in Markdown prose.
  Inside GitHub's own text fields (issues, pull requests, commit
  messages read on the site) the short `OWNER/REPO#N` form renders as a
  live link carrying the target's title, so prefer it there.
```

That is a genuine situational rule: it applies when writing Markdown, and
differently when writing a PR body. It is loaded on every turn regardless.

```console
$ rekall plan d497472 ab26d76
d497472  M1
  delete  set/skills/generic/skill.md:3-9
  write   .rekall/rules/the-setskills-tree-holds-this-projects.sh
  wire    add the script to the gate so it EXITS NONZERO on a violation (V2, V22)
  net     +114 tokens
ab26d76  S1
  delete  set/skills/git/repo/fleet.md:26-29
  write   .rekall/skills/use-the-full-httpsgithubcomownerrepo-form-in/SKILL.md
  wire    give the skill a trigger AND an explicit do-not-fire clause (V3, V4), AND wire `rekall hook` -- the head disables the host's own loading, so nothing loads this skill until something delivers it (V57)
  net     +60 tokens
```

`net` is the honest number: the statement's tokens minus the pointer left
in its place. `plan` will compute it for a statement whose class it got
wrong, exactly as it does here — the arithmetic is about tokens, not about
whether the extraction is a good idea. That judgement stays with you, which
is why `plan` is a separate verb from `apply` and why `--out` makes it a
file you can review before a byte moves.

## What the numbers do not say

- **35.3% of the tokens are `U`, and that is the honest answer**, not a gap
  to close. Those statements state something the classifier cannot turn
  into a runner or a trigger. They stay prose, and they still cost context
  — a corpus is not a thing to be driven to zero.
- **The 57.8% in `M` is a ceiling, not a plan.** Most of it is `M2`: one
  parameter each, decided by a human, one rule at a time.
- **Short statements barely pay.** The two above are 121 and 68 tokens; a
  one-line rule often nets single digits, because the pointer left behind
  costs almost as much as the statement did. `plan` says so before the
  move, in those words.
- **Nothing here measures whether the rules are any good.** `rekall`
  classifies what a statement can become, not whether it should exist.

## One carried all the way through

Everything above is report-only. This section is the other half: a single
statement taken out of that corpus and landed as a guardrail in the
repository it came from, in
[set-and-setting#513](https://github.com/pr0d1r2/set-and-setting/pull/513),
merged.

The statement, `4116dcd`, was 74 tokens of always-on prose:

> Always use `sed` from the dev shell (GNU sed via nixpkgs `gnused`).
> Never use macOS built-in `/usr/bin/sed` which is BSD sed and has
> incompatible flag syntax (e.g. `-i ''` vs `-i`). The dev shell provides
> GNU sed on all architectures so scripts stay portable.

`M1`: a fact about which binary, no judgement, and exactly the kind of rule
an agent breaks by reflex rather than by disagreement. `rekall apply` wrote
the runner, deleted the span, left the pointer and recorded the row —
**net +65 tokens**. `rekall revert 4116dcd` would put the paragraph back
verbatim.

### What the runner had to get right

Scoping, and it is not obvious. `set/skills/gnu/awk.md` names
`/usr/bin/awk` **in order to forbid it**, so a rule that greps every file
for the forbidden string fails on the prose that forbids it. The check
reads executable content — `*.sh`, `*.bash`, `*.nix`, `*.yml`, `*.yaml`,
`justfile` — and leaves Markdown alone. `.rekall/` is excluded for the same
reason one line up: the script quotes the statement it enforces.

### The host repository decides what a guardrail is

This is the part a `plan` line cannot tell you. `rekall plan` says "add the
script to the gate so it EXITS NONZERO on a violation", and in that
repository the gate has opinions:

- **A guardrail is a registered check.** A lefthook command missing from
  `check-fragment-map.nix` fails `check-fragment-map-complete`; one in the
  map but absent from `coveragePerFileClass` fails it a second way. Both
  commands are now declared against the file classes they read.
- **`lefthook.yml` is generated.** The steps belong in
  `setting/integrations/lefthook/set.yml`; the assembled file is rebuilt by
  `nix run .#mkSetting`. A first attempt edited the output and the fidelity
  check refused it — which is this tool's own argument one layer up: a
  generated thing has one source, and editing the output is how the two
  stop agreeing.
- **No execute bit.** `rekall apply` writes the runner `0755` and that
  repository's `execute-permissions` check forbids it. The step invokes
  `sh <path>`, so the bit was never load-bearing there.
- **A new script needs a spec.** `tdd-order-bats` refused the push until
  one existed.

None of that is rekall's business to know, and none of it is optional. An
extraction lands in somebody else's repository on that repository's terms.

### The spec found a real defect in the rule

Writing the seven bats cases immediately caught something reading had not:
the runner's self-exclusion is **by path**. A copy of the script anywhere
outside `.rekall/` matches its own quoted payload and reports itself as a
violation. That is a property of where `apply` installs it, and it is now
pinned by a test rather than assumed.

### What it cost rekall

Pointing the tool at prose it did not grow up with found three defects in
the tool, each fixed before this section was written:

| | found | |
|---|---|---|
| `B27` | `scan [<path>...]` was documented and unimplemented; a path was reported as an `unknown flag` | [#7](https://github.com/pr0d1r2/rekall/pull/7) |
| `B28` | `recall` said `load` for a rule `hook` would keep silent about, against a README claiming they cannot disagree | [#8](https://github.com/pr0d1r2/rekall/pull/8) |
| `B29` | an extraction that empties its source file said nothing — which is what happened to `sed.md` | [#9](https://github.com/pr0d1r2/rekall/pull/9) |

The first was wanted within four minutes of real use. The second needs a
matched trigger and a passing runner at the same moment, which only a real
rule in a real repository produces. The third is visible in this very
example: `set/skills/gnu/sed.md` is now a heading and a pointer, because
the whole file was that one statement.

Whether such a file should then be deleted is a decision for whoever owns
the corpus. `rekall` reports it and does not act: the pointer is what makes
the extraction reversible.

## Reproducing this

```sh
git clone https://github.com/pr0d1r2/set-and-setting && cd set-and-setting
git checkout 9724383
printf '[sources]\nroots = ["set/skills"]\nglobs = ["**/*.md"]\n' > rekall.toml
rekall scan --format json | jq '[.rows[] | .class] | group_by(.) | map({(.[0]): length}) | add'
```

Deterministic: same corpus in, same rows out. No model decides a class, so
the table above is a property of the input and this version of `rekall`,
not of the day it was run.
