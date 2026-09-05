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

WHAT MUST STAY TRUE HERE, one line each, numbered from the first id. Delete this line.
V1: extraction is a MOVE, ⊥ a copy. Source span deleted (pointer left) in the SAME commit the artifact lands. A copy leaves two hand-maintained statements of one rule -- `microlith`'s founding defect -- and leaves the context cost UNPAID ∴ the whole purpose lost.
V16: harness memory dirs are READ-ONLY unless `apply` NAMED that file. A tool that mines memory ! ⊥ corrupt it.
V20: `apply` CONFIRMS before it mutates: PROMPTS on a tty, DEMANDS `--auto-approve` off-tty & exits 2 without it. ⊥ prompting into a pipe -- that hangs a CI job until someone kills it -- & ⊥ proceeding silently -- that makes the DESTRUCTIVE path the QUIET one. The corpus is the user's private memory ∴ the single verb that deletes from it ! be deliberate, & "deliberate" ! survive being run by a machine.
V48: an `S` ARTIFACT is REKALL'S, PUBLISHED by SYMLINK where a host indexes one. CANONICAL under `.rekall/` beside the rules the ledger already names: ONE store, ONE reversal, & `.claude/skills/` is ⊥ general ∵ Codex has NO skills directory at all. Where a host DOES have one, `apply` links rather than copies -- Claude Code FOLLOWS a `<skill-name>` symlink & loads the target ONCE however many paths reach it (`.:R17`) ∴ ONE file, two paths, ⊥ drift, & V1 holds ∵ a link is ⊥ a copy. CORRECTED TWICE: this rule first said the host never indexed the file -- it DOES -- & then said the ⊥-fire clause is therefore ADVISORY on the host's path. It is ⊥: the HEAD turns host auto-loading OFF (V52) ∴ the clause is ENFORCED on BOTH paths & the link costs ⊥ a bypass. `check` verifies the link RESOLVES -- a dangling one is an ORPHAN, which the gate refuses -- & `revert` removes the LINK & the TARGET. the artifact HEAD keeps its indexer where a host has a dir, & is for a HUMAN where ⊥.

V52: the HEAD DISABLES the host's own loading. `disable-model-invocation: true` in every `S` artifact, ∵ the host CANNOT express a ⊥-fire clause -- `paths:` limits WHEN it auto-loads & there is ⊥ frontmatter for WHEN IT ! NOT -- ∴ the only rendering that keeps the REFUSAL CLAUSE true is to switch the host's automatic path OFF & leave delivery to `hook`, which reads the block. The file stays PRESENT: indexed, deduped, `/name`-invocable BY A HUMAN, which is a person choosing & ⊥ a model guessing. `paths:` is deliberately ⊥ emitted alongside it: with auto-loading off the key does nothing, & shipping a key the host ignores is what `hook:V49` forbids one file over. `check` VERIFIES the guard as it already verifies the head (T59) -- a head is a CONTROL SURFACE & ⊥ only an index entry.
V59: V16 needs a RUNNER, & the write path is where it goes. `apply` ! REFUSE a source outside the project base & SAY which file & why, before it opens anything. MEASURED 2026-09-05: there is ⊥ such check anywhere -- `edit_sources` does `base.join(&src)`, & a `~`-prefixed user root becomes `<project>/~/.claude/...`, which does ⊥ exist ∴ the memory file survives by a PATH ACCIDENT & the user sees `No such file or directory (os error 2)`. A privacy rule that holds ∵ a join happened to fail is a rule that stops holding the day the join is fixed. `plan` ALSO ! ⊥ promise the deletion: it prints `delete ~/.claude/.../a-fact.md:5-5` for a span `apply` cannot touch, which is `B13` again in the one place the cost is someone's private memory.

## §T TASKS

id|status|task|cites
T47|.|runner AUTHORING: `apply` ships an example per class, ⊥ a bare `exit 1`|`.:V2`
T50|.|slug ⊥ truncates mid-phrase|§I
T62|x|artifacts move to `.rekall/`, PUBLISHED by symlink where a host indexes one; `check` verifies the link resolves & `revert` removes both|V48,V1,`src:V43`,`src/revert:V9`

T66|x|`apply` writes `disable-model-invocation: true` into every `S` head; `check` VERIFIES it; the artifacts already written are REISSUED|V52,`src/trigger:V4`
T75|.|`apply` REFUSES a source outside the project base, naming the file & the reason; `plan` marks such a row UNAPPLIABLE rather than printing a `delete` for it|V59,V16,`src/plan:V57`

## §B BUGS

id|date|cause|fix
B6|2026-08-24|`apply` prefixed only the FIRST line of a quoted statement ∴ every WRAPPED bullet put prose into a shell script as CODE. MEASURED at this crate's first real extraction (consumer #0, Apple M4 10-core 16GB): the generated ASCII-rule runner printed `line 6: here.: command not found` -- `here.` being the second line of a 2-line bullet. Every test used a ONE-LINE fixture ∴ 509 green tests, & the defect appeared on the first statement a human actually wrote|`.:V42`
B10|2026-09-05|EVERY `S` artifact this crate has ever written is MODEL-INVOCABLE, ∴ the refusal clause was unenforceable on the host's path & the gate said nothing. MEASURED: 0 of the artifacts under `.rekall/` carry `disable-model-invocation`, & this repo's OWN skill is symlinked into `.claude/skills/` where the host will auto-load it whatever its ⊥-fire block says. FOUND by reading a sibling's frontmatter (`set-and-setting`), ⊥ by the gate, & V48 had already recorded the bypass as TOLERATED one hour before -- a trade-off written down as settled that a documented frontmatter key had always made avoidable|V52
B14|2026-09-05|T62 was flipped DONE with one of `V48`'s clauses unbuilt: "`check` verifies the link RESOLVES -- a dangling one is an ORPHAN". MEASURED: a dangling `.claude/skills/<slug>` passes the gate clean, & `src/check` holds ⊥ a symlink read anywhere. A `§T` status is a CLAIM ∴ flipping one over a partly-built row makes the whole table unciteable|`src/check:T74`
B20|2026-09-05|`V16` ("memory dirs are READ-ONLY") is enforced by ⊥ code. MEASURED: pointing a user-scope root at `~/.claude/projects` scans the memory files -- `MEMORY.md` & the notes beside it -- & `plan` then offers to DELETE a span from one. The file survives only ∵ `base.join("~/...")` cannot resolve ∴ exit 2 with a bare errno. Safe BY ACCIDENT, in the crate's most private surface|V59
