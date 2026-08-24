#!/bin/sh
# Extracted by rekall from CLAUDE.md:47-47 (id 3edbbbe).
#
# THE RULE, verbatim:
# - Every failing step names the fix, not just the breach.
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
# THE DETECTABLE HALF. Whether a message NAMES THE FIX is a judgment a
# runner cannot make -- "coverage fell, cover it" and "coverage fell" differ
# by meaning, not by shape. What a runner CAN see is SILENCE: a branch that
# exits nonzero having printed nothing at all, which is the failure mode
# with no defence at review time, because there is nothing in the diff to
# read.
#
# So this catches the floor of the rule and leaves the rest with whoever
# writes the message. An honest M3: it detects, it does not resolve.
#
# A failure block may span LINES. The one-liner form used through hk.pkl
# puts the echo and the exit together, but a multi-line block puts the echo
# above -- so a window of the preceding lines counts as the same branch, and
# a BLANK LINE ends it. Without that reset an unrelated echo six lines up
# vouches for a branch it has nothing to do with: MEASURED, a silent `if`
# block appended to another runner passed.
# Judged per line, this rule's OWN first runner called three such blocks
# silent, which is how the window got here.
silent=$(awk '
  /^[[:space:]]*$/ { for (i = 1; i <= WINDOW; i++) { back[i] = "" } }
  { for (i = WINDOW; i > 1; i--) { back[i] = back[i-1] } back[1] = $0 }
  /(^|[;&|[:space:]{])exit 1/ {
    if ($0 ~ /^[[:space:]]*#/) { next }
    said = 0
    for (i = 1; i <= WINDOW; i++) { if (back[i] ~ /echo/) { said = 1 } }
    if (!said) { printf "  %s:%d  %s\n", FILENAME, FNR, $0 }
  }
' WINDOW=6 hk.pkl .rekall/rules/*.sh)
[ -z "$silent" ] || { echo 'rekall: a gate branch exits nonzero and says NOTHING:' >&2; echo "$silent" >&2; echo 'Give it an echo to stderr naming what broke AND where to go next. A silent failure is a breach nobody can act on.' >&2; exit 1; }
