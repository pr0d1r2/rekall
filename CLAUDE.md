# Working on rekall

This file is CONSUMER #0's corpus. `rekall` mines files like this one for
statements worth turning into rules and skills, so the tool is pointed at
its own conventions before it is pointed at anyone else's -- a tool that
gates other people's extractions and not its own is a tool nobody has run
in anger.

Read that as a warning about the file's future, not a claim about its
present: every statement below is a candidate for extraction, and the ones
that stay are the ones that could not be made mechanical.

## Working agreement

- Commit straight to `main`. No feature branches in this repo.
- One decision per commit, and the reasoning goes in the message, not in a
  comment nobody will find.
- Decide in the spec before building. A judgment records what it rejected
  and what would reverse it, so a wrong call can be undone knowingly
  rather than archaeologically. `SPEC.md` V35 is the rule; this line only
  points at it, because two copies of one rule is the defect this whole
  tool exists to remove.
- Run `hk check` before pushing.

## Code

<!-- rekall 8043abe -->
- Never raise a threshold in `clippy.toml` to make a build pass. Those
  numbers are reviewed decisions and raising one is the reflex they exist
  to catch.
<!-- rekall b2c5684 -->
- Token counting belongs to `itok`. Do not write a second counter.
- Do not add a dependency for something the standard library already does.

## The spec

- `SPEC.md` is edited through `/spec`, never by hand.
- `/build` flips a `§T` status cell and touches nothing else in the spec.
- When a test fails, decide first whether it is a code bug or a spec gap. A
  gap goes through `SPEC.md` before the fix, so the invariant that would
  have caught it exists first.

## The gate

<!-- rekall b83698e -->
<!-- rekall 80c695d -->
- Every failing step names the fix, not just the breach.
