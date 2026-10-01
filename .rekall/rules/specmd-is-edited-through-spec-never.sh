#!/bin/sh
# Extracted by rekall from CLAUDE.md:37-37 (id 1780949).
#
# THE RULE, verbatim:
# rekall:payload
# - `SPEC.md` is edited through `/spec`, never by hand.
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
bad=$(find src -name SPEC.md -print0 | xargs -0 grep -L '^## §G GOAL$' || true)
[ -z "$bad" ] || { echo 'rekall: a module SPEC.md is missing the required §G GOAL section:' >&2; echo "$bad" >&2; exit 1; }
