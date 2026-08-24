# 8fe6c04

Extracted by rekall from CLAUDE.md:39-41.

<!-- rekall:payload -->
- When a test fails, decide first whether it is a code bug or a spec gap. A
  gap goes through `SPEC.md` before the fix, so the invariant that would
  have caught it exists first.
<!-- rekall:/payload -->

## Fires when

The block below IS the trigger. Prose here is for you and is never read
(V29). Keys: `tool` (exact names), `path` (globs), `word` (literals tested
against the situation text). Within a key ANY value matches; across keys
ALL present keys must match.

A test run is a Bash call naming the test runner. That is the moment the
decision has to be made -- before the fix is typed, not after it lands.

```rekall
tool = ["Bash"]
word = ["cargo test", "cargo nextest", "hk check", "cargo llvm-cov"]
```

## Does NOT fire when

State the absence rather than leaving it inferred (V4). A list of what
fires says nothing about what does not, and a matcher has to decide both.
This block WINS: a match here refuses the load even when the block above
matched.

LISTING tests is not RUNNING them, and neither is asking for help. Both
name the runner, and neither can fail in the way this rule is about.

```rekall
word = ["--list", "--help", "--no-run"]
```
