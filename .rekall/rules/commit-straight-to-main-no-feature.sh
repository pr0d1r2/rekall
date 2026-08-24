#!/bin/sh
# Extracted by rekall from CLAUDE.md:15-15 (id 551a009).
#
# THE RULE, verbatim:
# rekall:payload
# - Commit straight to `main`. No feature branches in this repo.
# rekall:/payload
#
# Exits NONZERO until the check is written. A runner that passes without
# testing anything gates nothing, and is worse than no runner (V2, V22).
#
# ## Fires when
#
# This rule already gates at COMMIT -- that is what the wiring line in
# `rekall plan` asks you to do. The block below is different: it makes the
# rule arrive at a TOOL CALL too, before the mistake instead of after
# (V37).
#
# It arrives EMPTY, which means GATE-ONLY: nothing fires it early until
# you say when. Keys are `tool` (exact names), `path` (globs) and `word`
# (literals tested against the situation text). Within a key ANY value
# matches; across keys ALL present keys must match.
#
# ```rekall
# tool = []
# path = []
# word = []
# ```
#
# ## Does NOT fire when
#
# A match here refuses the load even when the block above matched. State
# the absence rather than leaving it inferred (V4).
#
# ```rekall
# tool = []
# path = []
# word = []
# ```
# A branch is a decision that outlives the person who made it: work parked
# on one drifts from main until the merge is an event. This repo commits
# straight to main, so a NAMED branch other than main is the state worth
# reporting -- before the work piles up on it, not after.
#
# A DETACHED head is not a branch and is not a violation. CI checks out a
# commit, and a gate that reddens on every CI run is a gate someone turns
# off. `git branch --show-current` prints nothing when detached, which is
# the exact distinction needed.
branch=$(git branch --show-current 2>/dev/null) || {
  echo 'rekall: cannot read the current branch, so this rule cannot be judged. Run it from inside a git checkout (V26 -- a check that cannot run is a failure, not a pass).' >&2
  exit 1
}
[ -z "$branch" ] && exit 0
[ "$branch" = main ] || {
  echo "rekall: on branch \`$branch\`, and this repo commits straight to main." >&2
  echo 'Move the work over -- `git switch main` and cherry-pick or rebase it -- or decide out loud in SPEC.md that branches are now allowed here.' >&2
  exit 1
}
