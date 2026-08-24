#!/bin/sh
# Extracted by rekall from CLAUDE.md:47-48 (id b83698e).
#
# THE RULE, verbatim:
# - The coverage floor only ever rises. If it falls, cover the gap rather
# than lowering the number.
#
# MOVE THE CHECK HERE. Gate step `coverage` already enforces this
# rule. Move its body into this script and leave that step calling
# `sh .rekall/rules/the-coverage-floor-only-ever-rises.sh` -- one definition, many callers (V41, V23).
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
out=$(cargo llvm-cov --summary-only 2>&1) || { echo "$out" >&2; exit 1; }
now=$(printf '%s' "$out" | awk '/^TOTAL/{gsub("%","",$10); print $10}')
was=$(awk '/^lines /{print $2}' .coverage)
[ -n "$now" ] || { echo 'rekall: could not read a coverage total -- that is an ERROR, not a floor breach' >&2; exit 1; }
awk -v n="$now" -v w="$was" 'BEGIN{exit !(n+0 < w+0 - 0.3)}' && { echo "rekall: coverage FELL, $was% -> $now% (margin 0.3). A line that stopped being covered is a test that stopped asserting. Cover it, or say why in .coverage with the drop." >&2; exit 1; }
awk -v n="$now" -v w="$was" 'BEGIN{exit !(n+0 > w+0 + 0.5)}' && { echo "rekall: coverage ROSE, $was% -> $now%, and the floor was not raised. An unrecorded rise is a floor that lies about what is protected -- it would let every one of those lines go uncovered again in silence. Run: hk fix --all" >&2; exit 1; }
echo "coverage $now% (floor $was%)"
