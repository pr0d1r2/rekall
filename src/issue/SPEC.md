# SPEC

## §G GOAL

ISSUING a proven extraction to the loop that tends it: the portable copy written out, the local copy left standing, and the ledger row that says which stage it is in.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/issue|ISSUING a proven extraction to the loop that tends it: the portable copy written out, the local copy left standing, and the ledger row that says which stage it is in.
sib|src/apply|EXECUTING an extraction: the artifact written, the source span deleted, the pointer left in its place, and the ledger row that makes it reversible.
sib|src/catch|the SECOND intake: a transcript read tolerantly, a human turn recognised, a candidate proposed.
sib|src/check|THE GATE over extractions: a rule with no runner, a skill with no trigger or no refusal clause, an orphan artifact, a span that should have gone.
sib|src/classify|the VERDICT: signals, their weights, the deadband, and class x sharpness.
sib|src/cli|the VERB SURFACE: dispatch, argument parsing, the two output formats, and the exit codes.
sib|src/config|`rekall.toml`: the two scopes, the per-key merge, and the union that is the exception to it.
sib|src/corpus|REACHING the files: roots x globs, the walk, symlink loops, and what could not be read.
sib|src/hook|the HARNESS ADAPTER: a payload read tolerantly, a decision written, the fire counter, and the runner fired at the trigger point.
sib|src/init|THE COLD START: detecting roots and writing a project config without clobbering one.
sib|src/ledger|THE STORE: extracted rows, candidate rows, fire counts, and prefix lookup.
sib|src/log|READING the ledger back: fire counts, net reclaim, and what never fired.
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

V53: an artifact is TRACKED from the moment it exists, & ISSUING STARTS a move `V54` finishes. `src/apply:V1` ONE LEVEL OUT: prose leaves the CORPUS & a pointer stays; the artifact leaves the REPO in its own time & the LEDGER ROW stays, ∴ what a registry adopts is ⊥ a second copy of anything. TRACKED & ⊥ gitignored through the trial ∵ the AUDIT TRAIL is the whole point -- this repo's history says what was EXTRACTED & when, the registry's says what was ADOPTED & when, & a gitignored trial has ⊥ history to show. It also keeps ONE ledger: MEASURED, a gitignored artifact against a tracked row makes `check` say `missing-artifact` on every CLONE, & splitting the ledger to quiet that would put reversibility in two stores. PROMOTION is EARNED, ⊥ automatic: fires are the evidence, & `--dead` already NAMES the rules that never earned one. REJECTED: a gitignored trial, which hides a rule from the measurement meant to end it.

V54: the move is ZERO-DOWNTIME ∴ BOTH copies stand for a while, & the LEDGER is what makes that a TRANSITION rather than a DUPLICATION. Removing the artifact AT `issue` would leave the rule enforced by NOTHING until the registry materializes it back -- the exact gap the extraction existed to close, reopened by the step meant to complete it. ∴ `issue` writes OUT & leaves the local artifact STANDING, & the row records WHERE it went. RETIRING the local copy is a HUMAN's call ∵ this crate knows ⊥ a registry's layout & cannot SEE a materialization it did not perform -- guessing one would be the sniffing `src:V47` refuses, one repo out. `check` REPORTS an issued row whose artifact still stands: INFORMATION, ⊥ drift, ∵ the overlap is INTENDED & the report is the only thing stopping it becoming permanent. `src/apply:V1` is ⊥ bent by this: it forbids a rule stated twice with ⊥ a RECORD, & the record is precisely what an issued row is. A RETIRED row is ⊥ `missing-artifact` ∴ `check` READS the row's stage: the ledger that keeps the audit trail cannot be the ledger that fails the gate.

## §T TASKS

id|status|task|cites
T65|x|a fresh checkout gets ⊥ published links: `publish` runs at `apply` ONLY. Republish the ledger's `S` rows where a host dir exists|`src/apply:V48`,`.:V22`
T67|x|`rekall issue <id>... --to <dir>`: MOVE a proven extraction to where a loop tends it -- portable `SKILL.md` out, local artifact STANDS (V54), ledger row records WHERE. LOCAL WRITE ONLY, ⊥ a remote & ⊥ a `--push` (`.:V5`, `.:V15`). Knows ⊥ a destination's layout ∴ the registry ADOPTS. Subsumes T65's republish & T66's reissue: BOTH are `issue` over rows that already exist|`src/apply:V52`,V53,V54,`src/apply:V48`
T68|x|RETIREMENT: `rekall issue --retire <id>` drops the local artifact & its link, KEEPS the row & its `issued_to`, & RESTORES ⊥ prose to the corpus -- the rule now lives one repo out, ∴ this is ⊥ `revert` (`revert:V9`). `check` treats a retired row as COMPLETE|V54,V53
