#!/bin/sh
# Extracted by rekall from CLAUDE.md:18-22 (id 12b9139).
#
# THE RULE, verbatim:
# rekall:payload
# - Decide in the spec before building. A judgment records what it rejected
# and what would reverse it, so a wrong call can be undone knowingly
# rather than archaeologically. `SPEC.md` V35 is the rule; this line only
# points at it, because two copies of one rule is the defect this whole
# tool exists to remove.
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
[ -s SPEC.md ] || { echo 'rekall: SPEC.md is missing or empty; decide in the spec before building.' >&2; exit 1; }
