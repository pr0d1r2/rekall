# SPEC

## §G GOAL

PROSE INTO STATEMENTS: block splitting, normalisation, the path-scoped id, and the line span.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/statement|PROSE INTO STATEMENTS: block splitting, normalisation, the path-scoped id, and the line span.
sib|src/apply|EXECUTING an extraction: the artifact written, the source span deleted, the pointer left in its place, and the ledger row that makes it reversible.
sib|src/catch|the SECOND intake: a transcript read tolerantly, a human turn recognised, a candidate proposed.
sib|src/check|THE GATE over extractions: a rule with no runner, a skill with no trigger or no refusal clause, an orphan artifact, a span that should have gone.
sib|src/classify|the VERDICT: signals, their weights, the deadband, and class x sharpness.
sib|src/cli|the VERB SURFACE: dispatch, argument parsing, the two output formats, and the exit codes.
sib|src/config|`rekall.toml`: the two scopes, the per-key merge, and the union that is the exception to it.
sib|src/corpus|REACHING the files: roots x globs, the walk, symlink loops, and what could not be read.
sib|src/hook|the HARNESS ADAPTER: a payload read tolerantly, a decision written, the fire counter, and the runner fired at the trigger point.
sib|src/init|THE COLD START: detecting roots and writing a project config without clobbering one.
sib|src/issue|ISSUING a proven extraction to the loop that tends it: the portable copy written out, the local copy left standing, and the ledger row that says which stage it is in.
sib|src/ledger|THE STORE: extracted rows, candidate rows, fire counts, and prefix lookup.
sib|src/log|READING the ledger back: fire counts, net reclaim, and what never fired.
sib|src/plan|the extraction DIFF: which span goes, which artifact arrives, what wiring is named, and the corpus fingerprint that makes it stale.
sib|src/recall|WHICH situational skills load in a given situation.
sib|src/revert|REVERSING one extraction verbatim from what the ledger kept.
sib|src/runner|EXECUTING a rule's script under a CPU bound, and reporting what it said.
sib|src/scan|the INVENTORY: one row per statement, filtered, sorted, report-only.
sib|src/show|ONE statement argued in full: every signal that fired and what it was worth.
sib|src/tokens|DELEGATING every count to `itok`, and the per-call scratch that keeps concurrent counts apart.
sib|src/trigger|the fenced `rekall` BLOCK: its keys, how they combine, and the refusal clause that wins.

## §V INVARIANTS

V62: an INDEX ENTRY is ⊥ a STATEMENT. `- [name](file.md) -- hook` POINTS at prose; it does ⊥ STATE policy ∴ it is ⊥ a candidate, & splicing one DESTROYS the index. MEASURED 2026-09-05: a `MEMORY.md` line scored `M2` on the `always` in its hook, `apply` replaced the whole entry with a pointer, & the memory file it named was ORPHANED -- reachable by nothing -- while the runner took its slug from markdown syntax (`a-factafactmd-...`). ⊥ `apply:B20`, which FAILED loudly: this SUCCEEDS. DETECTED ⊥ assumed: a file is an INDEX where ≥2 lines are entries & ≥1 target EXISTS beside it. The SIBLING is the evidence -- a bullet carrying a link is ordinary prose, & a directory of the files it names is ⊥.

V64: a STATEMENT CARRIES its SECTION. The LAST heading before it, or `None` where it sits before the first -- read off the SPLIT, ∵ `split` is the ONE pass that sees a heading at all: `is_structure` DROPS heading lines ∴ by the time the classifier has the text, WHICH SECTION it came from is gone & ⊥ recoverable from the bytes. A `#` INSIDE a FENCE is ⊥ a heading -- it is a comment or a shell prompt, & taking one hands the NEXT statement a section that exists only in an EXAMPLE (∀ skill file showing a `sh` snippet has one). The heading is CARRIED, ⊥ JUDGED: what it is WORTH is `classify:V64`'s, ∵ this module OWNS the split & that one owns the verdict. `heading` does ⊥ enter the ID (`src:V13`): a statement RE-FILED under a renamed heading is the SAME claim & ! keep its id, ⊥ silently become a new row whose ledger entry points nowhere.

## §T TASKS

id|status|task|cites
T49|.|id PORTABILITY: same file via two root spellings = two ids. NAME the trap in §I|§I,`src:V13`
T78|x|`scan` emits ⊥ statements from a DETECTED index: ≥2 entry lines & ≥1 linked target existing beside the file|V62,`apply:V60`
T81|x|a `Statement` CARRIES the last heading before it; a fenced `#` is ⊥ one|V64,`classify:V64`

## §B BUGS

id|date|cause|fix
B21|2026-09-05|`scan` offered a `MEMORY.md` INDEX LINE as an extraction & `apply` TOOK it: the entry became `<!-- rekall <id> -->` & the memory file it named was orphaned from its own index. SILENT, ⊥ `apply:B20` which at least failed ∴ rekall CORRUPTED a file it does ⊥ own, which is what `apply:V16` exists to stop|V62
