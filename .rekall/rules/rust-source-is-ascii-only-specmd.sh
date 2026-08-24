#!/bin/sh
# Extracted by rekall from CLAUDE.md:27-28 (id 8043abe).
#
# THE RULE, verbatim:
# - Rust source is ASCII only. `SPEC.md` symbols are FORMAT and do not apply
# here.
#
# MOVE THE CHECK HERE. Gate step `ascii` already enforces this
# rule. Move its body into this script and leave that step calling
# `sh .rekall/rules/rust-source-is-ascii-only-specmd.sh` -- one definition, many callers (V41, V23).
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
hits=$(LC_ALL=C grep -rn '[^ -~]' src/ || true)
[ -z "$hits" ] || { echo 'rekall: non-ASCII or control character in Rust source:' >&2; echo "$hits" >&2; echo 'SPEC.md symbols are FORMAT, not source -- spell it in ASCII here.' >&2; exit 1; }
