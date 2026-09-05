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
V48: an `S` ARTIFACT is REKALL'S, PUBLISHED by SYMLINK where a host indexes one. CANONICAL under `.rekall/` beside the rules the ledger already names: ONE store, ONE reversal, & `.claude/skills/` is ⊥ general ∵ Codex has NO skills directory. Where a host DOES have one, `apply` LINKS rather than copies -- Claude Code follows a `<skill-name>` symlink & loads the target ONCE however many paths reach it (`.:R17`) ∴ ONE file, two paths, ⊥ drift, & V1 holds ∵ a link is ⊥ a copy. `check` verifies the link RESOLVES (`src/check:T74`) & `revert` removes LINK then TARGET. CORRECTED TWICE, & V52 is what the second correction became: this rule once said the host never indexes the file, then that the ⊥-fire clause is ADVISORY there. Both wrong -- the HEAD turns host loading OFF ∴ the clause is ENFORCED on both paths.

V52: the HEAD DISABLES the host's own loading. `disable-model-invocation: true` in every `S` artifact, ∵ the host CANNOT express a ⊥-fire clause -- `paths:` limits WHEN it auto-loads & there is ⊥ frontmatter for WHEN IT ! NOT -- ∴ the only rendering that keeps the REFUSAL CLAUSE true is to switch the host's automatic path OFF & leave delivery to `hook`, which reads the block. The file stays PRESENT: indexed, deduped, `/name`-invocable BY A HUMAN, which is a person choosing & ⊥ a model guessing. `paths:` is deliberately ⊥ emitted alongside it: with auto-loading off the key does nothing, & shipping a key the host ignores is what `hook:V49` forbids one file over. `check` VERIFIES the guard as it already verifies the head (T59) -- a head is a CONTROL SURFACE & ⊥ only an index entry.
V59: `apply` RESOLVES the source it NAMED, wherever it lives. V16 makes a memory dir read-only UNLESS `apply` named the file ∴ a `~`-prefixed or absolute source is EXTRACTABLE, ⊥ forbidden -- & `B20` measures what happens instead. What is REFUSED is what cannot be resolved, by name & with a reason. CORRECTED: an earlier draft said REFUSE any source outside the base, which contradicts the V16 it cites.
V60: an INDEXED corpus is edited WHOLE or ⊥ at all. A memory dir carries `MEMORY.md`, one line per file, & extracting a fact leaves that line RESTATING the rule which now lives in an artifact -- V1's duplication, & `MEMORY.md` loads every session ∴ the cost is still paid. ∴ where an index NAMES the source, `apply` maintains BOTH or REFUSES the row & says the index is unhandled. rekall does ⊥ own this convention ∴ it is DETECTED, ⊥ assumed. NARROWED: the ENTRY itself is `statement:V62`'s subject, ⊥ this rule's -- an index line was ⊥ merely unmaintained, it was OFFERED as an extraction & taking it orphaned the file (`statement:B21`). This rule is about the fact file's OWN extraction.
V61: memory LEAVES only by a HUMAN's word, twice. FIRST whether: a `feedback_*` note about how someone wants to be TALKED TO scores identically to one about hooks ∴ class is a CANDIDATE & ⊥ a permission, & moving a private note into a TRACKED `.rekall/` changes who may read it. SECOND when: the entry STANDS until the artifact has FIRED -- `log:V55`'s counter, ⊥ a green gate (`log:B11`) -- & retirement is its own act, `src/issue:V54` one level EARLIER. REJECTED: extracting on class alone, a tool deciding what a person meant.

## §T TASKS

id|status|task|cites
T47|.|runner AUTHORING: `apply` ships an example per class, ⊥ a bare `exit 1`|`.:V2`
T50|.|slug ⊥ truncates mid-phrase|§I

T62|x|DONE, folded: artifacts under `.rekall/` published by symlink & the link verified; the head carries `disable-model-invocation` & written artifacts REISSUED. What each row DID is in `git log`; what it DECIDED is in the invariants beside it|V48,V52,V1,`src:V43`,`src/revert:V9`,`src/trigger:V4`
T75|x|`apply` RESOLVES a `~`-prefixed or absolute source instead of joining it onto the base; what it cannot resolve is REFUSED by name. `plan` ⊥ prints a `delete` for a row `apply` cannot perform|V59,V16
T76|.|`apply` MAINTAINS the index entry when the fact file it names is extracted, or refuses the row & says the index is unhandled. `revert` restores it (`.:V9`)|V60,`statement:V62`
T77|.|STAGED memory retirement: extraction leaves the entry standing, `log` shows the fires, & a separate act removes entry+index. The JUDGMENT half is a `/rekall` slash command ∵ this crate has ⊥ a model (`.:V5`) & ! ⊥ grow one|V61,`log:V55`

## §B BUGS

id|date|cause|fix
B6|2026-08-24|`apply` prefixed only the FIRST line of a quoted statement ∴ every WRAPPED bullet put prose into a shell script as CODE -- the generated runner printed `line 6: here.: command not found`. Every test used a ONE-LINE fixture ∴ 509 green tests, & the defect appeared on the first statement a human actually wrote|`.:V42`
B10|2026-09-05|EVERY `S` artifact this crate has ever written is MODEL-INVOCABLE, ∴ the refusal clause was unenforceable on the host's path & the gate said nothing. MEASURED: 0 of the artifacts under `.rekall/` carry `disable-model-invocation`, & this repo's OWN skill is symlinked into `.claude/skills/` where the host will auto-load it whatever its ⊥-fire block says. FOUND by reading a sibling's frontmatter (`set-and-setting`), ⊥ by the gate, & V48 had already recorded the bypass as TOLERATED one hour before -- a trade-off written down as settled that a documented frontmatter key had always made avoidable|V52
B14|2026-09-05|T62 was flipped DONE with one of `V48`'s clauses unbuilt: "`check` verifies the link RESOLVES -- a dangling one is an ORPHAN". MEASURED: a dangling `.claude/skills/<slug>` passes the gate clean, & `src/check` holds ⊥ a symlink read anywhere. A `§T` status is a CLAIM ∴ flipping one over a partly-built row makes the whole table unciteable|`src/check:T74`
B20|2026-09-05|`V16` ("memory dirs are READ-ONLY") is enforced by ⊥ code. MEASURED: pointing a user-scope root at `~/.claude/projects` scans the memory files -- `MEMORY.md` & the notes beside it -- & `plan` then offers to DELETE a span from one. The file survives only ∵ `base.join("~/...")` cannot resolve ∴ exit 2 with a bare errno. Safe BY ACCIDENT, in the crate's most private surface|V59
