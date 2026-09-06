# A measured example

The README's walkthrough uses a four-line corpus so every number fits on the
page. This is the other kind of evidence: `rekall` pointed at a real corpus
that somebody else maintains, with the commands to reproduce it.

Nothing here was extracted. Every command below is report-only — `scan`,
`show` and `plan` do not touch a corpus, which is what makes it safe to run
them against a repository you do not own.

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
