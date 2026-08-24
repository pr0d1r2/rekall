#!/bin/sh
# Extracted by rekall from CLAUDE.md:49-49 (id 80c695d).
#
# THE RULE, verbatim:
# rekall:payload
# - A gate step that cannot run is a failure, not a pass.
# rekall:/payload
#
# MOVE THE CHECK HERE. Gate step `runners` already enforces this
# rule. Move its body into this script and leave that step calling
# `sh .rekall/rules/a-gate-step-that-cannot-run.sh` -- one definition, many callers (V41, V23).
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
missing=''
for tool in cargo mth itok rekall; do
  command -v "$tool" >/dev/null 2>&1 || missing="$missing $tool"
done
[ -z "$missing" ] || { echo "rekall: MISSING owned runner(s):$missing -- a step that cannot run is a FAILURE, not a pass, so this gate is RED rather than quietly shorter. Enter the dev shell: 'direnv allow', or 'nix develop' from the repo root." >&2; exit 1; }
