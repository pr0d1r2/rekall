# SPEC

## §G GOAL

the SECOND intake: a transcript read tolerantly, a human turn recognised, a candidate proposed.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/catch|the SECOND intake: a transcript read tolerantly, a human turn recognised, a candidate proposed.
sib|src/apply|EXECUTING an extraction: the artifact written, the source span deleted, the pointer left in its place, and the ledger row that makes it reversible.
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

WHAT MUST STAY TRUE HERE, one line each, numbered from the first id. Delete this line.
V44: a VIOLATION in a transcript is a USER turn the CLASSIFIER calls a RULE -- ⊥ `U`. WHO: the ASSISTANT's own text is ⊥ EVIDENCE ∵ a model RESTATING the rule it just broke would mint a candidate from its own apology, & the ledger would fill with rules NOBODY WROTE. WHAT: the CLASS, ⊥ a SIGNAL FAMILY -- §I said "classed like any other" & that IS the test ∴ `U` filters a transcript's REQUESTS on its own. SAME classifier as `scan`: a violation classed by different code is TWO implementations of one judgment (`apply:V1`), & `.:V10` makes a class a CLAIM ∴ arguable by the SAME `show`. DETERMINISTIC & CPU-only (`.:V5`). REJECTED: a MODEL reading the transcript (`.:V5`) · DIFFING assistant output against the existing rules (CIRCULAR -- it needs the rules this verb exists to FIND) · EVERY user turn (a transcript is mostly REQUESTS) · SCOPING to `classify:V40`'s MOOD families, which this invariant SAID for one commit until B8 MEASURED the miss. REVERSES on a harness that MARKS a correction STRUCTURALLY ∴ read the MARK, ⊥ the class.
V45: REPORT-ONLY is about the CORPUS, ⊥ about the DISK. TWO stores & they are ⊥ the same KIND: the CORPUS is someone's PROSE (`.:V15`, `apply:V16`) & the LEDGER is this crate's OWN record (`revert:V9`). `.:V7` forbids a report-only verb touching the FIRST; `catch` writes the SECOND & that is ⊥ an exception to it. §I stated both halves in ONE sentence ("Report-only FOR THE CORPUS ... writes CANDIDATE rows") ∴ the QUALIFIER carried the whole rule with ⊥ an invariant behind it (T29) -- state it or lose it at the first refactor. A CANDIDATE is ⊥ an EXTRACTION: `src` is the TRANSCRIPT ⊥ a corpus span · ⊥ an artifact · ⊥ a span deleted ∴ `check` ⊥ gate it & `revert` has NOTHING to reverse. PROMOTION is `plan` then `apply`, which is where the corpus is finally touched & where `apply:V20`'s consent ALREADY sits ∴ ⊥ a second consent here. REJECTED: a SECOND file for candidates (two stores, one record -- `apply:V1`) · `catch` PRINTING only (copy-paste IS the failure the verb exists to remove) · writing them as `U` statements into the corpus (`apply:V16`, & a REPORT would GROW the corpus).
V46: a transcript is SOMEONE ELSE'S DOCUMENT on its own release cadence ∴ READ what is known & IGNORE the rest. Its FORMAT is the AGENT's, ⊥ a constant: Claude Code writes one JSONL per session, Codex nests a turn under `payload` & calls the human `developer`, & opencode writes ⊥ JSONL at all -- one JSON per message, with the TEXT in a SECOND tree. An UNKNOWN field is IGNORED ⊥ rejected -- `hook` already learned this shape (`hook:B7`, `hook:T57`) & a reader that refuses an unfamiliar payload BREAKS on the harness's next release. An UNPARSABLE line is SKIPPED & COUNTED, & the count is REPORTED: a silent skip reads as "⊥ violations found", which is `.:V26`'s lie in a verb instead of a step. ABSENT `<session>` = the NEWEST transcript under the CONFIGURED root ∴ the common case needs ⊥ a path a human would have to look up. REJECTED: REQUIRING a path always (the EPHEMERALITY §I names is the whole reason this verb exists) · a HARDCODED harness directory, ∵ a path baked into the binary reproduces on ONE box.

## §T TASKS

id|status|task|cites
T18|x|`rekall catch` transcript intake|I.catch,`.:V10`,V44,V45,V46
T29|x|CHOSEN: `catch` persists CANDIDATE rows to the LEDGER; report-only is about the CORPUS, ⊥ the disk|`.:V7`,V45,`.:T3`

## §B BUGS

id|date|cause|fix
B8|2026-09-05|V44 scoped a transcript VIOLATION to MOOD & the verb found almost nothing. MEASURED on the three forms a correction takes: "Never commit a `.env` file" = `M1` on `never` & "Always run the gate before pushing" = `M2` on `always` -- both DIRECTIVE, both with no mood signal, both INVISIBLE, while "Use the helper, not the macro" was caught on `contrast`. Mood is HALF a directive (`classify:V40`) therefore the scope DROPPED the whole-weight vocabulary that states a rule outright -- the strongest evidence, excluded for not needing a mood word|V44
