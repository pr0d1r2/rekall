# SPEC

## §G GOAL

the HARNESS ADAPTER: a payload read tolerantly, a decision written, the fire counter, and the runner fired at the trigger point.

## §N NAV

rel|path|lens
up|.|-
up|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true
self|src/hook|the HARNESS ADAPTER: a payload read tolerantly, a decision written, the fire counter, and the runner fired at the trigger point.
sib|src/apply|EXECUTING an extraction: the artifact written, the source span deleted, the pointer left in its place, and the ledger row that makes it reversible.
sib|src/catch|the SECOND intake: a transcript read tolerantly, a human turn recognised, a candidate proposed.
sib|src/check|THE GATE over extractions: a rule with no runner, a skill with no trigger or no refusal clause, an orphan artifact, a span that should have gone.
sib|src/classify|the VERDICT: signals, their weights, the deadband, and class x sharpness.
sib|src/cli|the VERB SURFACE: dispatch, argument parsing, the two output formats, and the exit codes.
sib|src/config|`rekall.toml`: the two scopes, the per-key merge, and the union that is the exception to it.
sib|src/corpus|REACHING the files: roots x globs, the walk, symlink loops, and what could not be read.
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

V37: `hook` fires an `M` rule ONLY where it carries a TRIGGER, & runs ONLY what the LEDGER names. An EMPTY `M` block = GATE-ONLY, ⊥ broken & ⊥ nagged at: the runner lives in the gate (`.:V2`), a trigger is how it ADDITIONALLY arrives uninvited (§G) ∴ both hold. LEDGER-named is the teeth: a script DROPPED into `.rekall/rules/` is an ORPHAN, ⊥ something a tool call runs. Trust is a git hook's -- user-authored, user's repo -- ∴ the surface is SELECTION, ⊥ execution. REJECTED: firing ∀ `M` on ∀ call (a spawn per rule per call; `ledger:V11` would count invocations); `hook` never firing `M` (⊥ §I).
V34: `hook` writes the FIRE COUNTER & NOTHING ELSE -- ⊥ a span, ⊥ an artifact, ⊥ a row's content. `.:V7` named three writers & left `ledger:V11`'s counter with NO author ∴ `--dead` reports everything dead forever & `.:R13`'s argument collapses. `apply:V20` ⊥ applies: it guards the DESTRUCTIVE path, a counter deletes nothing, & `hook` is unattended ∴ a prompt hangs the harness `apply:V20` protects. `recall` counts NOTHING: the number ! mean LOADED. It ! survive CONCURRENT hooks -- one per tool call ∴ read-modify-write RACES. REJECTED: a second store (⊥ `ledger:V11`; two disagree -- `.:V8`); `hook` report-only (nothing counts).
V49: a DIALECT that cannot DELIVER at an event FAILS or SAYS SO, ⊥ emits a key the host ignores. MEASURED 2026-09-05: Codex `PreToolUse` accepts `permissionDecision` · `updatedInput` · `systemMessage` & ⊥ `additionalContext`, which is legal only at `SessionStart` · `SubagentStart` · `PostToolUse` ∴ rekall's CURRENT reply is a silent no-op on Codex -- exit 0, nothing loaded, nothing said. `.:V26` one level out: the rule there is that a SKIP ! BE SAID, & this is a skip wearing a successful exit. ∀ (agent, event) pair either RENDERS the injection or NAMES its refusal on stderr. The default agent is `claude` (`src:V47`) ∴ a Codex user who omits `--agent` gets the REFUSAL, ⊥ a silent no-op -- which is the whole difference this rule buys.
V58: `V49`'s refusal is SAID on stderr at exit 0, ⊥ a nonzero exit. This adapter sits in the request path of EVERY tool call ∴ a nonzero exit is ⊥ one bad report, it is the harness meeting an error on every action a person takes -- & the first fix anyone reaches for is deleting the hook line, which loses the delivery this rule was protecting. STDOUT stays a VALID EMPTY DECISION `{}` ∵ the harness parses it ∴ ⊥ a parse error on top of the refusal. `.:V26` is satisfied by the SAYING & ⊥ by the exiting: the skip is visible either way, & only one of the two costs the user their tool call. The (agent, event) pairs this crate has MEASURED are the only ones it renders: `claude` at any event takes `hookSpecificOutput.additionalContext` (RUNNING, this repo, 2026-09-05), `codex` at `PreToolUse` takes `systemMessage`. ∀ OTHER pair REFUSES & NAMES what was measured -- ⊥ a guess at an envelope nobody here has run, ∵ a shape guessed right is indistinguishable from one guessed wrong until it silently drops a skill.

## §T TASKS

id|status|task|cites
T39|x|`hook` FIRES an `M` rule that carries a trigger; empty block = gate-only; ADVISES ⊥ blocks, bounded time|V37,`.:V38`,`.:V2`,`.:T3`
T57|x|`hook`'s situation TEXT carries the TOOL INPUT, ⊥ `prompt` alone; `recall` & `hook` agree on ONE payload in a test|`.:V18`,B7

## §B BUGS

id|date|cause|fix
B7|2026-08-24|`hook` built the situation TEXT from `prompt` ALONE ∴ a `word` trigger could ⊥ EVER match a TOOL CALL -- the only event it runs on. MEASURED on this crate's FIRST `S` extraction: `recall "cargo test" --tool Bash` said `load`, the same payload through `hook` said `{}`. `.:V18`'s named failure, VERBATIM: "each looks correct alone, & the divergence only shows where a skill fails to load in production but `recall` swears it would". ⊥ two matchers -- ONE matcher fed two different SITUATIONS, which `.:V18` ⊥ say out loud|`.:V18`,T57
