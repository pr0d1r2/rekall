---
description: Retire a memory entry whose extraction has FIRED -- the human's word, twice
disable-model-invocation: true
---

Retire the memory entry for extraction `$ARGUMENTS` (a rekall id or prefix).

`rekall` has no model and no verb that deletes a memory entry. Extraction
(`rekall apply`) leaves the entry and its index line STANDING. Retirement is
this separate act, and the JUDGMENT in it is yours and the user's.

1. Run `rekall log --format json` and find the row for the id. Read its
   `fires`, and run `rekall log --dead` to see whether the counter is
   instrumented at all.
2. STOP, and say why, unless the row has `fires` > 0. `fires = 0` is not
   evidence the rule is safe to forget, and "unmeasured" is not "dead": a
   runner the gate executes every commit never records a fire.
   Then find every row whose source is that same memory file: a file can
   hold more than one extracted statement, and deleting it takes them all.
   STOP unless each of those rows has `fires` > 0, and unless the file holds
   nothing beyond their `<!-- rekall <id> -->` pointers (`apply` left one
   where each statement stood) -- anything else in it has no artifact yet.
3. Run `rekall show <full-id>`, using the `id` of the single log row that
   matches `$ARGUMENTS` (not the prefix the user typed: a prefix can name a
   live statement instead), and read the original text. Then run
   `rekall show <full-id>` for every sibling row found in step 2 as well, so
   the user sees each affected ID and its original text. Name the memory file
   (the source in the ledger row) and its `MEMORY.md` index line to the user.
   Say that deleting the file removes the pointer `rekall revert` needs for
   EACH listed ID, so every one of those extractions stops being reversible
   in place.
4. Ask the user to confirm that file and that index line, by name. A note
   about how someone wants to be talked to scores like one about hooks:
   class is a candidate, never a permission. No clear yes, no deletion.
5. Only then re-read the memory file and `MEMORY.md`. If either changed
   since step 2, STOP: redo step 2 and ask again, since the yes was for what
   you showed. Otherwise delete the memory file and its one line in
   `MEMORY.md`, and nothing else. Name each file touched, and report what
   was removed.
