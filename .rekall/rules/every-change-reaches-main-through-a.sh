#!/bin/sh
# Extracted by rekall from CLAUDE.md:15-15 (id 6d3751c).
#
# THE RULE, verbatim:
# rekall:payload
# - Every change reaches `main` through a pull request. No direct pushes, and that includes yours.
# rekall:/payload
#
# EARLY WARNING, not the authority. GitHub's branch protection is what
# actually refuses a direct push, and it cannot be talked out of it. This
# runs first and costs a second, so the answer arrives before five commits
# have to be moved off main rather than after.
#
# A DETACHED head is not a branch: CI checks out a commit, and a gate that
# reddens on every CI run is a gate someone turns off. `git branch
# --show-current` prints nothing when detached, which is the distinction.
branch=$(git branch --show-current 2>/dev/null) || {
  echo 'rekall: cannot read the current branch, so this rule cannot be judged. Run it from inside a git checkout (V26 -- a check that cannot run is a failure, not a pass).' >&2
  exit 1
}
[ -z "$branch" ] && exit 0
[ "$branch" != main ] && exit 0
echo 'rekall: committing on `main`, and main takes MERGES only -- every change reaches it through a pull request.' >&2
echo 'Open a branch -- `git switch -c <name>` -- and put the work there; `gh pr create` when it is ready.' >&2
exit 1
