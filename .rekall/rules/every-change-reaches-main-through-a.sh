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
#
# BEING ON `main` IS NOT THE VIOLATION -- having work there is. This step
# runs at PUSH as well as at COMMIT, and pushing a merged `main` to the
# mirror is exactly what a mirror is for. What the rule forbids is local
# `main` carrying commits `origin/main` has never seen, which is the state
# a direct commit creates and a merge never does.
upstream=$(git rev-parse --verify --quiet origin/main) || {
  echo 'rekall: no `origin/main` to compare against, so this rule has nothing to judge. Fetch first if you meant to check.' >&2
  exit 0
}
ahead=$(git rev-list --count "$upstream"..HEAD 2>/dev/null) || ahead=0
[ "$ahead" -eq 0 ] && exit 0
printf 'rekall: %s commit(s) on `main` that `origin/main` does not have, and main takes MERGES only.\n' "$ahead" >&2
echo 'Move them -- `git switch -c <name>` then `git reset --hard origin/main` on main -- and open a pull request.' >&2
exit 1
