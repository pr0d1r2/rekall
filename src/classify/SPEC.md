# SPEC

## §G GOAL

the VERDICT: signals, their weights, the deadband, and class x sharpness.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/classify|the VERDICT: signals, their weights, the deadband, and class x sharpness.
sib|src/apply|EXECUTING an extraction: the artifact written, the source span deleted, the pointer left in its place, and the ledger row that makes it reversible.
sib|src/catch|the SECOND intake: a transcript read tolerantly, a human turn recognised, a candidate proposed.
sib|src/check|THE GATE over extractions: a rule with no runner, a skill with no trigger or no refusal clause, an orphan artifact, a span that should have gone.
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
V30: `[signals]` WEIGHTS decide CLASS, ⊥ SHARPNESS. Class is a BALANCE -- directive against hedge -- ∴ it takes a weight & a DEADBAND, & that deadband IS `.:V10`'s "I do not know". Sharpness is a LADDER of KINDS (§I: `M2` = runner needs ONE human-set parameter) ∴ ⊥ a score: a sum cannot say WHICH KIND of runner a statement admits, & rounding one to a rung INVENTS the property `.:V10` assigns to the STATEMENT. `show` prints â signal WITH its weight ∴ the balance is ARGUABLE.
V40: MOOD is a SIGNAL, & it counts ONLY inside a LIST ITEM. Modal vocabulary ALONE left 53% `U` on this crate's OWN corpus, & 3 of those rows are gate rules whose runners ALREADY exist (`.:R15`) ∴ the classifier answered "I do ⊥ know" exactly where the gate ENFORCES -- a VOCABULARY gap wearing `.:V10`'s humility, & it BLOCKS `.:T19`. THREE families, each worth HALF a directive (as a CONDITIONAL is half a hedge) ∴ ONE hedge still BEATS a bare imperative: IMPERATIVE opener · ABSOLUTE quantifier (`every`·`all`·`only`·`no`·`none`·`nothing`·`any`) · a `, not ` CONTRAST, ∵ a statement naming its own NEGATIVE case is STATING a rule -- `trigger:V4`'s logic one level out. The SCOPE is the load-bearing half: a corpus states its RULES as bullets & its CONTEXT as paragraphs, & UNSCOPED these families read the PARAGRAPH "Read that as a warning ..., ⊥ a claim ..." as an `M` rule (`.:R15`) ∴ prose becomes law inside the dir `apply:V16` makes near-sacred. REJECTED: a POS tagger or a model (`.:V5`) · UNSCOPED mood (that false positive) · a WIDER deadband to force corroboration ("when editing `.rs`, never unwrap" sums to +1 & ! stay `M` ∴ the deadband is tuned for HEDGES, ⊥ for moods) · leaving it `U` for a human to override (there is ⊥ a class override on `plan`/`apply`). REVERSES on a corpus that states its rules in PARAGRAPHS; `[signals]` tunes every weight & `0` switches one OFF.

## §T TASKS

id|status|task|cites
