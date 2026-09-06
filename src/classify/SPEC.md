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
sib|src/issue|ISSUING a proven extraction to the loop that tends it: the portable copy written out, the local copy left standing, and the ledger row that says which stage it is in.
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

V30: `[signals]` WEIGHTS decide CLASS, ⊥ SHARPNESS. Class is a BALANCE -- directive against hedge -- ∴ it takes a weight & a DEADBAND, & that deadband IS `.:V10`'s "I do not know". Sharpness is a LADDER of KINDS (§I: `M2` = runner needs ONE human-set parameter) ∴ ⊥ a score: a sum cannot say WHICH KIND of runner a statement admits, & rounding one to a rung INVENTS the property `.:V10` assigns to the STATEMENT. `show` prints â signal WITH its weight ∴ the balance is ARGUABLE.
V40: MOOD is a SIGNAL, & it counts ONLY inside a LIST ITEM. Modal vocabulary ALONE left 53% `U` on this crate's OWN corpus, & 3 of those rows are gate rules whose runners ALREADY exist (`.:R15`) ∴ the classifier answered "I do ⊥ know" exactly where the gate ENFORCES -- a VOCABULARY gap wearing `.:V10`'s humility, & it BLOCKS `.:T19`. THREE families, each worth HALF a directive (as a CONDITIONAL is half a hedge) ∴ ONE hedge still BEATS a bare imperative: IMPERATIVE opener · ABSOLUTE quantifier (`every`·`all`·`only`·`no`·`none`·`nothing`·`any`) · a `, not ` CONTRAST, ∵ a statement naming its own NEGATIVE case is STATING a rule -- `trigger:V4`'s logic one level out. The SCOPE is the load-bearing half: a corpus states its RULES as bullets & its CONTEXT as paragraphs, & UNSCOPED these families read the PARAGRAPH "Read that as a warning ..., ⊥ a claim ..." as an `M` rule (`.:R15`) ∴ prose becomes law inside the dir `apply:V16` makes near-sacred. REJECTED: a POS tagger or a model (`.:V5`) · UNSCOPED mood (that false positive) · a WIDER deadband to force corroboration ("when editing `.rs`, never unwrap" sums to +1 & ! stay `M` ∴ the deadband is tuned for HEDGES, ⊥ for moods) · leaving it `U` for a human to override (there is ⊥ a class override on `plan`/`apply`). REVERSES on a corpus that states its rules in PARAGRAPHS; `[signals]` tunes every weight & `0` switches one OFF.

V64: a HEADING PROMOTES a paragraph to a LIST ITEM's mood treatment. `.:V40` is REFINED, ⊥ REVERSED: it suppressed mood in paragraphs ∵ "a corpus states its RULES as bullets & its CONTEXT as paragraphs", & that reading is TRUE of a `CLAUDE.md` & FALSE of a SKILL file, where the rule IS the prose under `## Applying X` · `## Signals of violation` · `## Limits`. MEASURED (Apple M1 Pro 8-core 32GB, 2026-09-06, `set-and-setting` @ `set/skills`, 193 files, 1388 statements): 107 `U` rows were PARAGRAPHS under a heading, incl. POLA's whole thesis ("Every interface ... should behave the way its user would reasonably expect"). WITH the 13 new IMPERATIVE words (`.:V40`'s vocabulary, ⊥ a new rule), `U` 828 -> 613 of 1388 (59.7% -> 44.2%) & `U` TOKENS 23097 -> 14817 of 41986 (55.0% -> 35.3%). The SECTION is EVIDENCE of the SAME KIND as the bullet marker, ONE LEVEL UP: a heading is the corpus SAYING OUT LOUD that what follows is a section & ⊥ preamble. `.:V40`'s MEASURED false positive is still CAUGHT & that was VERIFIED ⊥ asserted -- this crate's own "Read that as a warning ..., ⊥ a claim ..." sits BEFORE the first `##` ∴ stays `U`, while the bullet under `## Working agreement` is `M2`. REJECTED: a WHITELIST of heading names (`Applying`·`Limits`·`Signals of violation`) -- an English vocabulary that ROTS per corpus, & the PRESENCE of any heading already carries the evidence · heading DEPTH (H2+ only, H1 = title) -- measured NOTHING extra here & adds a rule a reader ! hold · a corpus-level `form = "skill"` switch -- `[signals]` already tunes per word ∴ a second, coarser switch is a SECOND RULE SET · a POS tagger or a model (`.:V5`). REVERSES on a corpus writing genuine CONTEXT in paragraphs UNDER headings -- a narrative `## Background` now scores as rules; `[signals]` tunes every weight & `0` switches one OFF, the same escape `.:V40` names.

## §T TASKS

id|status|task|cites
T82|x|`Form` reads the HEADING beside the marker ∴ a paragraph in a SECTION is classified as a rule|V64,`statement:V64`
