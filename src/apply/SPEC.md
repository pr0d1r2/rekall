# SPEC

## §G GOAL

EXECUTING an extraction: the artifact written, the source span deleted, the pointer left in its place, and the ledger row that makes it reversible.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/apply|EXECUTING an extraction: the artifact written, the source span deleted, the pointer left in its place, and the ledger row that makes it reversible.
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

WHAT MUST STAY TRUE HERE, one line each, numbered from the first id. Delete this line.
V1: extraction is a MOVE, ⊥ a copy. Source span deleted (pointer left) in the SAME commit the artifact lands. A copy leaves two hand-maintained statements of one rule -- `microlith`'s founding defect -- and leaves the context cost UNPAID ∴ the whole purpose lost.
V16: harness memory dirs are READ-ONLY unless `apply` NAMED that file. A tool that mines memory ! ⊥ corrupt it.
V20: `apply` CONFIRMS before it mutates: PROMPTS on a tty, DEMANDS `--auto-approve` off-tty & exits 2 without it. ⊥ prompting into a pipe -- that hangs a CI job until someone kills it -- & ⊥ proceeding silently -- that makes the DESTRUCTIVE path the QUIET one. The corpus is the user's private memory ∴ the single verb that deletes from it ! be deliberate, & "deliberate" ! survive being run by a machine.
V48: an `S` ARTIFACT is REKALL'S & DELIVERY is `hook`'s. `.claude/skills/` is ONE host's directory & Codex has NO skills directory at all ∴ a host-native location is ⊥ general, & choosing per host would fork the ledger, `check` & `revert` three ways. Artifacts live under `.rekall/` beside the rules the ledger already names: ONE store, ONE reversal. This costs nothing that was working -- `hook` INJECTS the payload (`.:V43`), so the host never indexed the file. `.:V43`'s HEAD is AMENDED by this: where the host has ⊥ a directory it has ⊥ an indexer, & the head is then for a HUMAN reading the artifact. PUBLISHING a copy into a host dir is SEPARATE & later; a copy is the duplication V1 removes.

## §T TASKS

id|status|task|cites
T47|.|runner AUTHORING: `apply` ships an example per class, ⊥ a bare `exit 1`|`.:V2`
T50|.|slug ⊥ truncates mid-phrase|§I
T62|.|artifacts move to `.rekall/`; ledger, `check` & `revert` follow the move, ⊥ a second store|V48,V1,`revert:V9`

## §B BUGS

id|date|cause|fix
B6|2026-08-24|`apply` prefixed only the FIRST line of a quoted statement ∴ every WRAPPED bullet put prose into a shell script as CODE. MEASURED at this crate's first real extraction (consumer #0, Apple M4 10-core 16GB): the generated ASCII-rule runner printed `line 6: here.: command not found` -- `here.` being the second line of a 2-line bullet. Every test used a ONE-LINE fixture ∴ 509 green tests, & the defect appeared on the first statement a human actually wrote|`.:V42`
