# SPEC

## §G GOAL

ONE statement argued in full: every signal that fired and what it was worth.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/show|ONE statement argued in full: every signal that fired and what it was worth.
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
sib|src/statement|PROSE INTO STATEMENTS: block splitting, normalisation, the path-scoped id, and the line span.
sib|src/tokens|DELEGATING every count to `itok`, and the per-call scratch that keeps concurrent counts apart.
sib|src/trigger|the fenced `rekall` BLOCK: its keys, how they combine, and the refusal clause that wins.

## §V INVARIANTS

WHAT MUST STAY TRUE HERE, one line each, numbered from the first id. Delete this line.
V67: an ID OUTLIVES its statement ∴ `show` resolves against the CORPUS, then the LEDGER. `apply` DELETES the span & leaves a pointer (`src/apply:V1`) ∴ the id `scan` printed, the ledger recorded & `log` echoes stops resolving the moment it is ACTED ON -- & the verdict most worth arguing with (`.:V10`) is precisely the one already acted on. The ledger keeps the TEXT verbatim (`.:V9`) & ⊥ the heading it sat under ∴ the class is RE-DERIVED from marker form alone & `recorded` carries the ledger's own label WHERE THE TWO DIFFER: printing one verdict as if it were the other would be a claim about context this crate threw away. ONE anatomy across both halves (`.:V17`) -- `artifact` & `fires` are ABSENT for a live statement, ⊥ empty.

## §T TASKS

id|status|task|cites
T85|x|`show` falls back to the ledger: an extracted id prints its span, text, signals, artifact & fire count|V67,`.:V22`

## §B BUGS

id|date|cause|fix
B24|2026-09-06|`rekall show <id>` answered `no statement matches` for EVERY extracted id, exit 2. §I has claimed "ONE statement OR ARTIFACT in FULL ... artifact path & FIRE count if extracted" since commit one & the module read the CORPUS only ∴ the branch was ⊥ built, ⊥ tested & ⊥ noticed -- `.:V22`'s shape, one clause out. MEASURED 2026-09-06 on a corpus of 4: every live id resolved, every extracted id failed|V67
