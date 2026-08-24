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
# with no defence at review time because there is nothing in the diff to
# read.
#
# So this catches the floor of the rule, and the rest stays with the person
# writing the message. That is an honest M3: it detects, it does not
# resolve.
silent=$(grep -nE '(^|[;&|{[:space:]])exit 1' hk.pkl .rekall/rules/*.sh 2>/dev/null \
  | grep -v 'echo' \
  | grep -vE '^[^:]*:[0-9]+:[[:space:]]*#' || true)
[ -z "$silent" ] || { echo 'rekall: a gate branch exits nonzero and says NOTHING:' >&2; echo "$silent" >&2; echo 'Give it an echo to stderr naming what broke AND where to go next. A silent failure is a breach nobody can act on.' >&2; exit 1; }
