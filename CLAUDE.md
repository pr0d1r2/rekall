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
- Run `hk check` before pushing.

## Code

- Rust source is ASCII only. `SPEC.md` symbols are FORMAT and do not apply
  here.
- Never raise a threshold in `clippy.toml` to make a build pass. Those
  numbers are reviewed decisions and raising one is the reflex they exist
  to catch.
- An `allow` must name what it exempts and why. An exemption is not a
  suppression.
- Token counting belongs to `itok`. Do not write a second counter.
- Do not add a dependency for something the standard library already does.

## The spec

- `SPEC.md` is edited through `/spec`, never by hand.
- `/build` flips a `§T` status cell and touches nothing else in the spec.
- When a test fails, decide first whether it is a code bug or a spec gap. A
  gap goes through `SPEC.md` before the fix, so the invariant that would
  have caught it exists first.

## The gate

- The coverage floor only ever rises. If it falls, cover the gap rather
  than lowering the number.
- A gate step that cannot run is a failure, not a pass.
- Every failing step names the fix, not just the breach.
