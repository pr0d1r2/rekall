# SPEC

## §G GOAL

the CRATE: one node per module, each owning the rules for its own directory. This node routes; it holds no law of its own.

## §F FEDERATION

dir|owns|⊥owns|tokens
apply|EXECUTING an extraction: the artifact written, the source span deleted, the pointer left in its place, and the ledger row that makes it reversible.|the DIFF that decided it (`plan`), the store's shape (`ledger`), and undoing it (`revert`)|-
catch|the SECOND intake: a transcript read tolerantly, a human turn recognised, a candidate proposed.|the CLASS a candidate gets (`classify`) and where candidates are kept (`ledger`)|-
check|THE GATE over extractions: a rule with no runner, a skill with no trigger or no refusal clause, an orphan artifact, a span that should have gone.|writing artifacts (`apply`) and reversing them (`revert`)|-
classify|the VERDICT: signals, their weights, the deadband, and class x sharpness.|what a statement IS (`statement`) and where the weights are configured (`config`)|-
cli|the VERB SURFACE: dispatch, argument parsing, the two output formats, and the exit codes.|what any verb actually does, which lives in the module of the same name|-
config|`rekall.toml`: the two scopes, the per-key merge, and the union that is the exception to it.|finding the corpus those roots name (`corpus`)|-
corpus|REACHING the files: roots x globs, the walk, symlink loops, and what could not be read.|turning their text into statements (`statement`)|-
hook|the HARNESS ADAPTER: a payload read tolerantly, a decision written, the fire counter, and the runner fired at the trigger point.|deciding WHAT matches (`recall`, `trigger`) and executing the script (`runner`)|-
init|THE COLD START: detecting roots and writing a project config without clobbering one.|parsing that config afterwards (`config`)|-
issue|ISSUING a proven extraction to the loop that tends it: the portable copy written out, the local copy left standing, and the ledger row that says which stage it is in.|a registry's LAYOUT, which the registry owns, and the DECISION to retire, which is a human's|-
ledger|THE STORE: extracted rows, candidate rows, fire counts, and prefix lookup.|what any row MEANS, which belongs to the verb that wrote it|-
log|READING the ledger back: fire counts, net reclaim, and what never fired.|the store itself (`ledger`) and the counting (`tokens`)|-
plan|the extraction DIFF: which span goes, which artifact arrives, what wiring is named, and the corpus fingerprint that makes it stale.|performing any of it (`apply`)|-
recall|WHICH situational skills load in a given situation.|parsing a trigger block (`trigger`) and firing mechanical rules (`hook`)|-
revert|REVERSING one extraction verbatim from what the ledger kept.|the store's shape (`ledger`) and the artifact's shape (`apply`)|-
runner|EXECUTING a rule's script under a CPU bound, and reporting what it said.|what the rule checks, which is the rule author's|-
scan|the INVENTORY: one row per statement, filtered, sorted, report-only.|reaching the files (`corpus`), splitting them (`statement`), and judging them (`classify`)|-
show|ONE statement argued in full: every signal that fired and what it was worth.|the verdict itself (`classify`)|-
statement|PROSE INTO STATEMENTS: block splitting, normalisation, the path-scoped id, and the line span.|what those statements mean (`classify`)|-
tokens|DELEGATING every count to `itok`, and the per-call scratch that keeps concurrent counts apart.|the counts themselves, which are `itok`'s|-
trigger|the fenced `rekall` BLOCK: its keys, how they combine, and the refusal clause that wins.|who asks (`recall`, `hook`)|-

## §N NAV

rel|path|lens
up|.|-
self|src|the CRATE, federated one node per module: what each verb & each subsystem ! hold true

## §V INVARIANTS

WHAT MUST STAY TRUE HERE, one line each, numbered from the first id. Delete this line.
V2: ∀ extracted `M` -> a RUNNER, same commit. Rule with no runner gates nothing.
V3: ∀ extracted `S` -> a TRIGGER. A skill with no trigger is always-on prose, which is exactly what it was extracted FROM.
V13: idempotent. `scan(scan(x))` identical; `apply` of an already-extracted id = no-op, exit 0; `plan` of one yields an EMPTY diff, ⊥ an error.
V18: `recall` & `hook` share ONE matcher. `hook` = `recall` + `M`-rule firing + a stdin/stdout adapter; `recall` is the HUMAN & DEBUG view of the SAME decision. Two matchers is two rule sets (`.:V8`), & the INVISIBLE kind: each looks correct alone, & the divergence only shows where a skill fails to load in production but `recall` swears it would.
V38: a fired `M` rule ADVISES, ⊥ BLOCKS, & a runner that ⊥ finish in BOUNDED time ⊥ fired. TWO reasons, each sufficient: it runs in the TOOL-CALL path ∴ a hung runner stalls the harness · a WRONG rule that blocks costs the user their WORK, one that advises costs a LINE. Nothing is lost -- the runner still GATES at commit (V2) ∴ hook = EARLY word, gate = LAST. REJECTED: `permissionDecision: deny` (one buggy rule wedges every tool call, & unwedging means editing the corpus mid-task). The bound is CONFIGURED, ⊥ a constant: WALL-CLOCK under contention kills a rule that cost MILLISECONDS (`runner:B5`), & a limit nobody can raise is one that lies about what happened.
V41: an extraction MOVES the RUNNER too. `apply:V1` one level in: where the gate ALREADY enforces the rule, the artifact takes the CHECK ITSELF & the gate step becomes a CALLER (`sh .rekall/rules/<slug>.sh`) ∴ ONE definition & many callers (`.:V23`), & the rule sits WITH the runner that proves it. A script that RESTATES the step's command is the COPY `apply:V1` forbids, one layer down. This crate ⊥ own the host's gate file ∴ it NAMES the move & ⊥ performs it (`plan:T48`). WHICH step is a human's answer, given in the PLAN: a `runner` field per step, EMPTY = write the inert stub, a NAME = the wiring line becomes the MOVE. ⊥ a flag -- a per-id flag ⊥ scale past two ids, & the plan is ALREADY the reviewed artifact (`plan:V19`). Editing that field is ⊥ tampering: the FINGERPRINT covers the CORPUS, ⊥ the plan's own fields. REJECTED: a ledger field naming a gate step with ⊥ an artifact (`hook` then ⊥ fire it (`hook:V37`), & `check` ⊥ verify a step inside a format this crate ⊥ parse); `apply` EDITING `hk.pkl` (same format, & `.:V7` makes it name every file it touches).
V42: corpus text QUOTED in a generated artifact is INERT on EVERY line, ⊥ only the first. `apply` writes an `M` runner ∴ a wrapped statement's continuation lines land in a SHELL SCRIPT, & unprefixed they are COMMANDS (`apply:B6`). The corpus is private PROSE a human wrote for a reader (`.:V15`, `apply:V16`) ∴ it is INPUT & the one place this crate quotes it ! quote it -- `.:V33`'s shape (a gate MESSAGE is DATA, ⊥ CODE) one file over. TEETH, ⊥ a careful template: `check` VERIFIES the quoted block, ∵ a template is edited by the same hand that will forget (`.:V22`). SEVERITY is ⊥ cosmetic: `hook` runs these per TOOL CALL (V38), the text is the user's own memory, & TODAY it hides behind the stub's `exit 1`, surfacing only once someone writes the check.
V43: a generated artifact has THREE readers, each reading a DIFFERENT part: PAYLOAD = the rule VERBATIM · SCAFFOLD = the trigger blocks & the notes explaining them · HEAD = what the HOST indexes &, through `apply:V52`, what it may DO -- a CONTROL SURFACE, ⊥ only an index entry. `hook` injects the PAYLOAD ALONE. MEASURED 2026-08-24 (Apple M4 10-core 16GB): a 36-tok rule arrived as 318 tok of FILE, 8.8x, & §G says a triggered rule costs NOTHING until it fires ∴ 282 tok of MAINTENANCE NOTES at the fire point INVERTS that -- `.:V39`'s arithmetic, one file over. The HEAD is FRONTMATTER ∵ `apply` writes into the HOST's OWN dir & a file there is INDEXED: MEASURED, the first `S` lists under its HASH. `check` VERIFIES both (`.:V22`). REJECTED: injecting the FILE & trusting the model to skip the scaffold (SPENT by then); a SECOND file for the payload (two files, one rule -- `apply:V1`)
V47: the agent comes from PROVENANCE where there is one, & is NAMED where there is ⊥. `catch` needs ⊥ a flag: it ITERATES every known transcript root, & a file under `~/.codex/sessions` is Codex BY LOCATION -- a fact about where it SITS, ⊥ a guess about what it holds, ∴ ⊥ the sniffing this rule was written against. `hook` has ⊥ a location: a payload arrives bare ∴ `--agent` NAMES it there, & ABSENT means `claude` -- a DEFAULT, admitted as one, ∵ it keeps every hook line already pasted into a `settings.json` working & a Codex user passes the flag or gets `hook:V49`'s refusal. An UNKNOWN name is exit 2. REJECTED: SNIFFING a payload's discriminators (Codex sends `turn_id` & `permission_mode` where Claude Code sends neither ∴ it rests on ABSENCE, & absence is what the next release changes) · a MANDATORY flag on `hook`, which breaks every config in the field for a decision the caller already made when they wrote the line.

## §T TASKS

id|status|task|cites
T44|x|`recall`/`hook`: a MISSING `M` block is GATE-ONLY, ⊥ unreadable|`hook:V37`,B4
T60|x|`--agent <name>` on `hook`; an UNKNOWN name is exit 2, ⊥ a fallback. The `catch` half is SUPERSEDED: V47 made provenance do that job -- a file under `~/.codex/sessions` is Codex BY LOCATION, so a flag there would ask for what the path already answers|V47,I.hook
T61|x|CODEX adapter: a human turn is `payload.role` = `developer`, & `PreToolUse` takes `systemMessage`, ⊥ `additionalContext`|`catch:V46`,V47,`hook:V49`

## §B BUGS

id|date|cause|fix
B4|2026-08-24|`recall`/`hook` call a generated `M` artifact "trigger could not be read": a MISSING block is a parse failure reached BEFORE `hook:V37`'s gate-only branch ∴ a legal state reads as a defect, & `check` disagrees with `recall` about one file|`hook:V37`,T44
