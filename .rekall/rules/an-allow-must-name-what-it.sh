#!/bin/sh
# Extracted by rekall from CLAUDE.md:31-32 (id b2c5684).
#
# THE RULE, verbatim:
# - An `allow` must name what it exempts and why. An exemption is not a
# suppression.
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
# An `allow` is legal; an UNEXPLAINED one is not. The line above every
# `#[allow(...)]` must be a comment, which is where the name and the reason
# go. Bare attributes are what turn a reviewed exemption into a silent
# suppression, and the difference is invisible in a diff without this.
bare=$(awk '
  /#\[allow\(/ {
    if (prev !~ /^[[:space:]]*(\/\/|\/\*|\*)/) { printf "  %s:%d  %s\n", FILENAME, FNR, $0 }
  }
  { prev = $0 }
' $(find src -name '*.rs'))
[ -z "$bare" ] || { echo 'rekall: an `allow` with no comment above it:' >&2; echo "$bare" >&2; echo 'Name what it exempts and why on the line above. An exemption is not a suppression.' >&2; exit 1; }
