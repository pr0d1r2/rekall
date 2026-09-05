# SPEC

## §G GOAL

the extraction DIFF: which span goes, which artifact arrives, what wiring is named, and the corpus fingerprint that makes it stale.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/plan|the extraction DIFF: which span goes, which artifact arrives, what wiring is named, and the corpus fingerprint that makes it stale.
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
V19: a PLAN carries a corpus FINGERPRINT: a content hash per source file it touches. `apply <PLAN>` REHASHES & REFUSES on mismatch, exit 1, ⊥ an override flag. Spans are addressed `file:line-line` & the corpus is LIVE prose a human edits between the two commands ∴ a stale plan deletes the WRONG lines from the user's private memory. `revert` restores the BYTES but ⊥ the trust: the artifact was materialized from text that was never the rule, & the ledger records the mistake as if it were intended. `apply:V16` makes memory dirs near-sacred; a plan is the only place the promise can be CHECKED.
V57: `plan` NAMES the DELIVERY wiring, ⊥ only the artifact's own. An `M` row already says "add the script to the gate" ∵ writing the artifact ⊥ enforces it; an `S` row ! say the SAME about `rekall hook`, ∵ `src/apply:V52` turns the host's own loading OFF & the skill then reaches ⊥ ONE READER until a hook is wired. SAID AT PLAN TIME, before a byte moves (`.:V7`, `.:V19`), ∴ `src/check:V56`'s refusal is a PROMISE KEPT & ⊥ a surprise at the gate. ONLY where a hook is ⊥ already wired: a sentence telling you to do what you have done is the noise people learn to read past. MEASURED 2026-09-05: `apply` in a fresh project wrote a guarded skill & `check` refused the SAME tree seconds later, with ⊥ a word about it at plan time -- the two verbs disagreed & the user met the disagreement as a red gate on their first run.

## §T TASKS

id|status|task|cites
T48|.|`plan` NAMES the host's own format gates before rewriting a file this crate ⊥ owns|`apply:V16`
T73|x|`wiring_for` takes DELIVERED: an `S` row in a project with ⊥ hook names wiring `rekall hook` beside its trigger obligation|V57,`src/check:V56`

## §B BUGS

id|date|cause|fix
B13|2026-09-05|`apply` wrote the `src/apply:V52` guard UNCONDITIONALLY & `src/check:V56` called the result drift ∴ the crate shipped a default its OWN gate refuses: fresh project, one `S` extraction, `apply` exit 0 & `check` exit 1 seconds later. Two verbs decided hours apart, & ⊥ verb told the user at the point the decision was theirs|V57
