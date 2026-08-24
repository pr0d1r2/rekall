#!/bin/sh
# Extracted by rekall from CLAUDE.md:23-23 (id c75aeab).
#
# THE RULE, verbatim:
# rekall:payload
# - Run `hk check` before pushing.
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
# The rule is about a HABIT, and a habit is not checkable -- but the thing
# that makes the habit unnecessary IS. If the pre-push hook is installed and
# calls hk, then `hk check` runs on every push whether or not anyone
# remembers to type it. So this checks the mechanism, not the memory.
#
# The dev shell rewrites this hook on entry (flake.nix). A checkout that has
# not entered the shell has no hook, which is exactly the state worth
# naming: the gate is one `direnv allow` away from running.
root=$(git rev-parse --git-common-dir 2>/dev/null) || {
  echo 'rekall: not a git checkout, so there is no pre-push hook to look for. Run this from inside the repo.' >&2
  exit 1
}
hook="$root/hooks/pre-push"
[ -f "$hook" ] || {
  echo "rekall: no pre-push hook at $hook, so nothing runs \`hk check\` before a push." >&2
  echo 'Enter the dev shell -- `direnv allow`, or `nix develop` from the repo root -- and it writes the hook on entry.' >&2
  exit 1
}
grep -q 'hk' "$hook" || {
  echo "rekall: $hook exists but never calls hk, so a push is unguarded." >&2
  echo 'Re-enter the dev shell to have it rewritten, or add the `hk run pre-push` call by hand.' >&2
  exit 1
}
