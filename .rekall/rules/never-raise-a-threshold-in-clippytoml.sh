#!/bin/sh
# Extracted by rekall from CLAUDE.md:28-30 (id 76785f7).
#
# THE RULE, verbatim:
# rekall:payload
# - Never raise a threshold in `clippy.toml` to make a build pass. Those
# numbers are reviewed decisions and raising one is the reflex they exist
# to catch.
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
if grep -nE 'threshold[[:space:]]*=[[:space:]]*(0|false|allow)' clippy.toml >/dev/null 2>&1; then
  echo 'rekall: clippy.toml contains a permissive threshold; review the decision instead of weakening the gate.' >&2
  exit 1
fi
