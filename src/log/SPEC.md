# SPEC

## §G GOAL

READING the ledger back: fire counts, net reclaim, and what never fired.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/log|READING the ledger back: fire counts, net reclaim, and what never fired.
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
sib|src/plan|the extraction DIFF: which span goes, which artifact arrives, what wiring is named, and the corpus fingerprint that makes it stale.
sib|src/recall|WHICH situational skills load in a given situation.
sib|src/revert|REVERSING one extraction verbatim from what the ledger kept.
sib|src/runner|EXECUTING a rule's script under a CPU bound, and reporting what it said.
sib|src/scan|the INVENTORY: one row per statement, filtered, sorted, report-only.
sib|src/show|ONE statement argued in full: every signal that fired and what it was worth.
sib|src/statement|PROSE INTO STATEMENTS: block splitting, normalisation, the path-scoped id, and the line span.
sib|src/tokens|DELEGATING every count to `itok`, and the per-call scratch that keeps concurrent counts apart.
sib|src/trigger|the fenced `rekall` BLOCK: its keys, how they combine, and the refusal clause that wins.

## §V INVARIANTS

WHAT MUST STAY TRUE HERE, one line each, numbered from the first id. Delete this line.
V55: `fires` counts ONE DELIVERY PATH & the artifacts travel THREE ∴ `--dead` ! ⊥ speak for the other two. A `hook` fire is DELIVERED-TO-AN-AGENT; an hk step running the runner is ENFORCED; a host indexing the head is INDEXED -- three quantities, & only the first is instrumented. ∴ ⊥ fire JOURNAL = UNMEASURED, ⊥ dead: `--dead` SAYS which it is & names ⊥ a single row when the counter has never been written, ∵ "delete this, it never fired" over a rule the gate runs on every commit is the most expensive thing this crate could say. MEASURED 2026-09-05 in this repo: 8 of 8 rows at `fires = 0`, 7 of them wired into `hk.pkl` BY PATH & executed on every commit, `record_fire` reached from ONE non-test caller (`src/cli/hook.rs`), `.rekall/fires` never created. REJECTED: counting an hk run as a fire -- a gate step runs whether or ⊥ the rule was RELEVANT ∴ it inflates the count & makes ∀ rule look alive, which breaks the measurement in the OTHER direction.

## §T TASKS

id|status|task|cites
T69|.|`--dead` REPORTS its own instrumentation: ⊥ journal = `unmeasured`, & a row is named dead ONLY where the counter has been written & stayed 0|V55,`.:V22`
T70|.|SECOND counter: an ENFORCED count the gate increments, separate from the delivered one. Needs the runner to call back ∴ a decision about the gate contract first|V55

## §B BUGS

id|date|cause|fix
B3|2026-08-24|`log` reported GROSS statement tokens as reclaimed while `apply` wrote a POINTER back 8 lines away in the same module ∴ TWO real extractions each made the corpus BIGGER & the column said smaller|`.:V39`
B11|2026-09-05|`--dead` named ALL 8 extractions droppable, 7 of them runners `hk.pkl` executes every commit. The counter measures `hook` deliveries & the generated runner note tells you to wire the rule into your GATE ∴ the path this crate RECOMMENDS is the path that records nothing, & the report that exists to end a rule pointed at the seven enforcing this repo|V55
