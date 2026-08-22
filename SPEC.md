# SPEC -- rekall

Self-contained spec. `rekall` develops inside a larger workspace but is designed to leave it standalone. It carries its own law: no load-bearing reference outside this directory (V14). Lineage (`itok` - `microlith`) is SEE-ALSO, cited as evidence, never as authority. Both are PUBLIC ∴ an outside reader can CHECK a §R row. A row an outsider cannot check is marked `internal` & carries no name (V21).

## §G GOAL

Turn always-on agent prose into TANGIBLES: mine agent memory & `CLAUDE.md`-class corpora for statements that are MECHANICAL (become a CPU rule with a runner) or SITUATIONAL (become a skill with a trigger), extract them, and GATE that the extraction stayed honest.

MOTIVATING SHAPE: a rule a model must REMEMBER is already lost. Prose stated at turn 3 competes with everything after it, dies at compaction, and costs window on every turn it does NOT fire. A rule that arrives at its trigger point uninvited costs nothing until it matters.

CAPABILITY ⊥ MEMORY ∴ a stronger model is ⊥ the fix: the rule left the WINDOW, it was ⊥ too hard to apply. & the second cost outlives the first -- rationale decays, ∴ nobody dares DELETE & the file only grows (R13). `--dead` (V11) answers that with a MEASUREMENT, ⊥ with nerve: a rule that never fired is one you can drop & PROVE you could.

## §C CONSTRAINTS

- Rust, edition **2024**, MSRV **1.95** = the FLEET PIN, ⊥ a local choice: `nixpkgs-lock` pins `nixos-26.05` for every repo in the fleet (R8) ∴ CONFORM -- a crate that picks its own floor is a crate whose `nix build` reproduces only on its author's box. `itok` & `microlith` both declare 1.95, AGREEING with the pin ⊥ setting it. MEASURE here regardless: a declared floor MIRRORS the pin until a build PROVES it ∴ this crate proves its own IN THE GATE.
- ONE bin `rekall`; CRATE `rekall`; REPO `rekall`. MIT.
- NAME: repo = crate = theme = invocation, ONE word, ⊥ a short form ∴ nothing here is an exception to record. 6 chars is the `cargo`/`docker` rung; an `rkl` abbreviation would buy nothing & cost muscle memory. Theme = Total Recall's memory-implant company, named ∵ the job is memory NOBODY has to hold. Three names were rejected & the REASONS are load-bearing: `tr` ∵ POSIX coreutils -- a bin on `PATH` would SHADOW it, hard ⊥ INDEPENDENT of any registry (R1); `total-recall` ∵ TAKEN (R2); `outception` ∵ Inception already NAMES its inverse -- extraction -- ∴ the coinage renames a thing that has a word, & `out-` reads as OUTPUT (R10).
- CPU-ONLY, WHOLE CRATE -- ⊥ a "core" qualifier, ∵ a qualifier is where an exception later hides. ∀ verb deterministic & offline. ⊥ an opt-in inference tier (V5).
- ⊥ NETWORK, ⊥ MODEL, ⊥ INFERENCE, at any flag. The crate runs in a NETWORKLESS SANDBOX. THREE reasons, each SUFFICIENT ALONE: `check` is the GATE ∴ it ! run with no key & no network (V6) · the corpus is private user facts ∴ ⊥ a byte egresses (V15) · a DETERMINISTIC classifier IS the product (V10), & a model tier makes a class depend on a remote's uptime.
- ASCII-only Rust source (Trojan-Source, LLM-friendliness). SPEC symbols are FORMAT, ⊥ source.
- Corpus is PRIVATE. Agent memory & `CLAUDE.md` hold user facts ∴ ⊥ egress (V15).
- ONE config file `rekall.toml` from commit one. `itok` grew four dotfiles then began migrating back to unified (R6) -- start where it landed.
- Token accounting is `itok`'s, TOKENIZER-backed ∴ counts are MEASURED & deterministic, ⊥ estimated by a model & ⊥ a char/4 heuristic. ⊥ reimplemented here (V8).
- `SPEC.md` FORMAT owned by `microlith`, enforced by `mth` in the gate.
- Zero host-project internals ∴ extraction of THIS crate is a move.
- PLATFORMS = `nixpkgs-lock`'s `supportedSystems`, ALL FOUR: `aarch64-darwin` · `x86_64-darwin` · `x86_64-linux` · `aarch64-linux` (R12). SUPPORT & CI COVERAGE are ⊥ the same set: GitHub offers ⊥ a free x86_64 macOS runner ∴ CI builds THREE & `x86_64-darwin` is SUPPORTED-BUT-UNBUILT. NAME that gap in CI output & in the README. An untested platform is worse than an absent one ONLY when nobody says it is untested -- V26's rule, one level out: the SKIP is legal, the SILENCE is ⊥.
- `.envrc` = `use flake`, `.direnv/` gitignored. `flake.nix` at the repo ROOT, ⊥ a subdirectory: a flake's source root is its OWN dir ∴ a nested one cannot SEE `Cargo.toml`/`src/` & cannot offer a real package. ENTERING the shell IS the toolchain CI uses (V23) ∴ "works on my box" & "passes CI" stop being two questions.
- The dev shell puts `mth` & `rekall` on PATH via cargo-run shims ∴ the gate's own tools need ⊥ a global install, & this crate checks its OWN spec & its OWN corpus (T19, consumer #0). A tool absent from PATH is a check silently DEFERRED, ⊥ a check that failed (V26).
- Gate = `hk` (`hk.pkl`), ONE definition, local & CI (V23). Coverage floor RATCHETS (V27).
- GATE SET, tiered by COST. COMMIT: `cargo fmt --check` · `cargo clippy --all-targets -- -D warnings` · `cargo test` · `mth fmt --check` · `mth check` · ASCII gate · the `.context-limits` runner (V22). PUSH: coverage · doctest · rustdoc · `--no-default-features` · `cargo package`. A ~60s step on COMMIT is a step someone learns to bypass, & a bypassed hook is worse than none.
- `-D warnings` goes AFTER `--`, ⊥ in `RUSTFLAGS`: RUSTFLAGS reaches PATH DEPS, & `itok` is one (V8) ∴ a sibling's stray warning would redden THIS gate for code this crate ⊥ owns.
- ⊥ a lint-debt file & ⊥ an allow-list: this crate starts at ZERO code ∴ `-D warnings` runs CLEAN from commit one. An allow added later NAMES what it exempts & why -- an exemption is ⊥ a suppression.

## §I INTERFACES

- `rekall init` -- `--format human|json` · `--force`. THE COLD START. DETECT corpus roots (memory dir · `CLAUDE.md` · `AGENTS.md` · skill dirs) & write `rekall.toml`. Names every file before writing; ⊥ overwrites an existing config without `--force`. ⊥ this verb, first contact is `scan` failing on a config nothing ever wrote.
- `rekall scan [<path>...]` -- `--format human|json` · `--class M|S|U` · `--sharpness 1|2|3` · `--top N` · `--sources` · `-C <dir>`. Inventory the corpus: one row per STATEMENT -- `id` · `src` (`file:line-line`) · `tokens` · `class` · `sharpness` · `signals`. Deterministic, report-only. THE CPU CORE.
- ID = `<hash>` -- the first 7 hex of a digest over the statement's NORMALIZED text, scoped by source PATH. A second identical statement in one file takes `<hash>.2`, `.3`. Input accepts any unambiguous PREFIX & a shorter one that matches two ids is a USAGE error, ⊥ a coin toss. STABLE means: an edit ELSEWHERE in the file ⊥ moves this id -- which is what `file:line` fails & why V13 rules it out. Editing THE STATEMENT ITSELF changes its id, & that is CORRECT: a reworded rule is a new CLAIM & ! be reclassified, ⊥ silently inherit a verdict passed on different words. Deterministic, offline, STATELESS ∴ `scan` stays report-only (V7) -- an id assigned at first sight would have to be PERSISTED, & then scanning would mutate.
- CLASS × SHARPNESS. Class = `M`|`S`|`U`, R7's vocabulary UNCHANGED. Sharpness = `1`|`2`|`3`, printed joined ∴ `M2`. Sharpness is a property of the STATEMENT, ⊥ of the classifier: how sharp a runner or trigger the statement ADMITS, ⊥ how confident the classifier FEELS. `M1` runner deterministic, ⊥ judgment · `M2` runner needs ONE human-set parameter · `M3` runner DETECTS, ⊥ resolves. `S1` trigger EXACT (tool · path · extension) · `S2` trigger a signal-matchable CLASS of situations · `S3` trigger SEMANTIC ∴ only a model notices it. `U` carries ⊥ sharpness -- absence of a class has no ladder. Both ladders run SHARP -> FUZZY ∴ the digit PREDICTS fire rate & the `3` rows are where `--dead` (V11) comes from. ⊥ a third word (R7): `M`/`S` stand, the digit is a DEGREE. `--class M` matches ∀ `M*`; `--sharpness` filters across classes. JSON carries `class` · `sharpness` · `label` as SEPARATE fields ∴ an agent ⊥ string-surgery `M2` (V17).
- `rekall show <id>` -- `--format human|json`. ONE statement or artifact in FULL: source span · ORIGINAL TEXT · class × sharpness · EVERY signal that fired & its weight · artifact path & FIRE count if extracted. Report-only. `scan` truncates to a row ∴ `show` is where a class is ARGUED with (V10: a class is a claim, & a claim ! be inspectable).
- `rekall plan <id>...` -- `--format human|json` · `--out FILE` · `--to <dir>`. The DIFF of an extraction: per statement, the source span to DELETE, the artifact to WRITE, the wiring to ADD. Report-only, writes ⊥ except the plan file itself. `--out` makes the plan an ARTIFACT ∴ reviewable, diffable, committable before a byte of corpus moves.
- `rekall apply <id>... | <PLAN>` -- `--format human|json` · `--auto-approve` · `--to <dir>`. EXECUTE. `M` -> script + runner wiring. `S` -> skill file + trigger. Deletes the source span & leaves a pointer (V1). Names EVERY file touched. The ONLY mutating verb besides `catch`/`revert`. Given a PLAN it re-checks the corpus FINGERPRINT first & REFUSES a stale plan (V19). Confirms on a tty; off-tty demands `--auto-approve` (V20).
- `rekall check` -- `--format human|json`. THE GATE. ∀ extracted `M` ! has a runner (V2) · ∀ `S` ! has a trigger (V3) & a ⊥-fire clause (V4) · ⊥ orphan artifact · ⊥ source span still present. Exit 1 on drift. CPU-only ∴ runs in `hk` & CI with no key & no network (V6).
- `rekall recall <situation>` -- `--format human|json` · `--tool X` · `--path P` · `--cwd D`. Which situational skills ! load HERE. Deterministic matcher, report-only. This is the reload rule V3 demands. SAME matcher as `hook` (V18).
- TRIGGER FORMAT. `## Fires when` & `## Does NOT fire when` each carry a FENCED `rekall` block in TOML -- §C's ONE format & ONE parser, ⊥ a third grammar for three keys. Keys: `tool` (exact names) · `path` (globs) · `word` (literals vs the situation text). Within a key ANY value matches; across keys ALL PRESENT keys ! match. ⊥-fire takes the SAME keys & WINS (V29). `S1` admits `tool`/`path`, `S2` `word`, `S3` an EMPTY block (V29).
- `rekall hook` -- harness hook JSON on stdin -> decision JSON on stdout. ADAPTER shape: no daemon, no interception, ⊥ in the request path. Signals in JSON, ⊥ via exit code. Fires `M` rules & injects `S` skills at the TRIGGER point. SAME matcher as `recall` (V18) ∴ what `recall` PRINTS is what `hook` DECIDES.
- `rekall log` -- `--format human|json` · `--dead` · `--since D`. READS the ledger. Per artifact: source span · original text · artifact path · FIRE count · tokens reclaimed. `--dead` = never fired (V11). VERB is `log`, STORE is the ledger -- the store is a ledger & is called one everywhere it is described.
- `rekall catch [<session>]` -- `--format human|json`. Second intake: a VIOLATION in a transcript -> candidate statement, classed like any other. Report-only FOR THE CORPUS: it writes CANDIDATE rows to the ledger (V7) & touches ⊥ a source file, ⊥ an artifact. Promotion goes through `plan` then `apply`. A transcript is EPHEMERAL ∴ a violation seen at turn 200 is gone tomorrow unless the candidate outlives the session that produced it -- which is the whole reason this verb exists.
- `rekall revert <id>` -- `--format human|json` · `--auto-approve`. Reverse one extraction from the ledger, verbatim (V9). MUTATES ∴ confirms like `apply` (V20).
- Config: `rekall.toml` -- `[sources]` corpus roots & globs · `[signals]` classifier weights · `[triggers]` matcher defaults · `[budget]` ceilings.
- CONFIG SCOPE: TWO files, ONE format, ONE parser ∴ ⊥ the sprawl R6 records (that was four files & four grammars). PROJECT `./rekall.toml`, found by walking UP from cwd to the repo root; USER `~/.config/rekall/rekall.toml`. Merge is PER KEY & project WINS -- except `[sources]` roots, which UNION. That exception is the whole reason two scopes exist: the corpus SPANS them (per-user memory dir & `~/.claude/CLAUDE.md`; per-project `./CLAUDE.md` & `./AGENTS.md`) ∴ letting a project file REPLACE the roots would silently stop scanning the user's memory -- the largest half of the corpus, gone with no error. `init` writes the PROJECT file & names which scope each root came from.
- Exit: 0 ok · 1 drift/violation · 2 usage. ⊥ a network code ∵ ⊥ a network path.

## §R RESEARCH

id|topic|finding|src
R1|name `tr`|TAKEN crates.io v0.1.11 (i18n) AND `tr` = POSIX coreutils ∴ bin would SHADOW it -- ⊥ regardless of registry|crates.io/api/v1/crates/tr
R2|name `total-recall`|TAKEN crates.io v0.3.0, GUI to-do app ∴ crate name unavailable|crates.io/api/v1/crates/total-recall
R3|name `rekall`|FREE on crates.io @ 2026-08-21 ∴ repo · crate · bin collapse to ONE word, no bend to document|crates.io/api/v1/crates/rekall
R4|context ceiling|`itok` = 136,811 tok (spec+code) vs 102,529 WORKING @ 131,072 window ∴ a small disciplined tool ⊥ fit its own best-case hardware. BOX: M-series 24GB, ⊥ recorded further -- the defect V31 now forbids, left VISIBLE ⊥ back-filled with a guess. ⊥ either box in R14 ∴ RATIO stands, absolutes want re-measuring (T36)|MEASURED, internal
R5|conditional load|~50% of a repo never loads for impl work; conditional slicing brings one node to ~6% of the whole ∴ trigger-gated load is MEASURED, ⊥ hoped|MEASURED, internal
R6|config sprawl|`itok` main carries FOUR dotfiles LIVE -- `.context-limits` · `.context-models` · `.context-policy` · `.context-hosts` ∴ the sprawl is the SHIPPED state, ⊥ a near miss. A unification to `itok.toml` (project root wins, `~/.config` the fallback) exists on an UNLANDED branch ∴ EVIDENCE OF INTENT, ⊥ of outcome. Unify from commit one & pay ⊥ the migration|github.com/pr0d1r2/itok SPEC.md @ main
R7|class taxonomy|`mth check` already ranks each direction `Mechanical` \| `Judgment` -- the SAME 2-class split this crate needs ∴ reuse the vocabulary, ⊥ invent a third word|github.com/pr0d1r2/microlith SPEC.md §I @ main
R8|MSRV|the FLEET PIN is `nixpkgs-lock` @ `3677ad2` -> `github:NixOS/nixpkgs/nixos-26.05` ∴ 1.95 is fleet LAW, ⊥ this crate's preference; `itok` & `microlith` both declare `rust-version = "1.95"` on main, AGREEING with it. A declared floor mirrors the pin until a build proves it|github.com/pr0d1r2/nixpkgs-lock flake.nix @ 3677ad2
R9|adapter shape|`itok guard` = hook JSON stdin -> decision JSON stdout, opt-in, ⊥ in request path ∴ proven shape, copy it|github.com/pr0d1r2/itok SPEC.md §V @ main
R10|name `outception`|FREE, but Inception's OWN word for the inverse of inception is EXTRACTION ∴ the coinage names an already-named thing; `extraction` also free but a generic-noun squat|the film's vocabulary
R12|platforms|`nixpkgs-lock` -- the fleet PIN repo, ∴ the authority -- declares `supportedSystems` = `aarch64-darwin` · `x86_64-darwin` · `x86_64-linux` · `aarch64-linux`. Consumers `itok` & `microlith` each declare that list MINUS `x86_64-darwin` ∴ the 4 are SUPPORT & the 3 are what GitHub CI can BUILD, ⊥ two disagreeing support claims. Read a consumer as authority & you conclude Intel-mac is unsupported -- it is UNBUILT|github.com/pr0d1r2/nixpkgs-lock flake.nix @ main
R13|instruction growth|MEASURED over 247,694 instruction lifetimes in 1,867 repos: agent instruction files grow +226% across their life at +4.9 net instructions/commit, & the deletion hazard FALLS with age (-0.032/commit) ∴ an old rule is never removed. A wholesale rewrite dropping ~40% is followed by FASTER regrowth (+4.9%/commit vs +4.1%) ∴ manual pruning is ⊥ a fix. "Catastrophic remembering" = the RATIONALE is lost ∴ deletion is UNSAFE, ⊥ merely unpleasant|alphaxiv.org/abs/2608.11095
R14|dev boxes|TWO boxes, ~5x apart: timings @ 2026-08-22 ran on Apple M4 10-core 16GB; the BUILD box is M1 Pro 8-core 32GB. CONSEQUENCE: one `itok` spawn per statement = 57ms fast ∴ 11s / 200 statements, but ~57s slow -- past §C's bypass threshold -- while ONE batched call = 74ms. The RATIO decides, ⊥ either absolute ∴ ∀ timing ! name its box (V31)|MEASURED, internal

## §V INVARIANTS

V1: extraction is a MOVE, ⊥ a copy. Source span deleted (pointer left) in the SAME commit the artifact lands. A copy leaves two hand-maintained statements of one rule -- `microlith`'s founding defect -- and leaves the context cost UNPAID ∴ the whole purpose lost.
V2: ∀ extracted `M` -> a RUNNER, same commit. Rule with no runner gates nothing.
V3: ∀ extracted `S` -> a TRIGGER. A skill with no trigger is always-on prose, which is exactly what it was extracted FROM.
V4: ∀ trigger -> an explicit ⊥-fire clause, ⊥ only a fire clause. Absence is ⊥ PROVABLE from a positive description: a list of what FIRES says nothing about what does ⊥, & a matcher ! decide both.
V5: CPU-ONLY, ∀ verb, ⊥ exception. Deterministic & offline everywhere. ⊥ model, ⊥ network, ⊥ inference tier at ANY opt-in. An opt-in that CAN fire is a path that WILL fire, & then the classifier's floor is a remote model's uptime. If inference is EVER wanted it is a SEPARATE consumer reading `scan --format json`, ⊥ a tier inside this crate (V8).
V6: `check` is the GATE ∴ CPU-only, no key, no network. A gate needing a model runs nowhere it is needed.
V7: report-only DEFAULT. Only `apply` · `catch` -> ledger · `revert` touch the CORPUS or the LEDGER, & each NAMES every file touched before writing. `init` writes ONE file & only its OWN (`rekall.toml`), refusing an existing one without `--force` ∴ it is ⊥ in that set: it cannot reach a source span, & V20's confirm guards the DESTRUCTIVE path, ⊥ every write.
V8: token counts DELEGATED to `itok`. Two estimators disagreeing is the same defect as two rule sets. GENERALIZE it, ∵ the shape recurs: ⊥ a second implementation of a capability a sibling already OWNS, in ANY domain. Cite this row, ⊥ restate it.
V9: ∀ extraction REVERSIBLE. Ledger holds source path · line span · ORIGINAL TEXT · artifact path ∴ `revert` is mechanical, ⊥ a rewrite.
V10: a class is a CLAIM, ⊥ truth. ∀ row carries the SIGNALS that fired & its SHARPNESS. `U` (unknown) is legal & is the DEFAULT. A classifier that never says "I do not know" is lying at a fixed rate. Sharpness is DERIVED from the same signals ∴ also a claim, & a `3` is the spec SAYING OUT LOUD that this artifact may never fire.
V11: ledger counts FIRES ∴ a never-fired artifact is DETECTABLE. A rule that never fires is a wrong trigger or dead law; both need to be visible, ⊥ inferred. SHARPNESS predicts what `--dead` MEASURES ∴ the two are checkable against each other: a `1` gone dead is a CLASSIFIER defect, a `3` that fires often is a LADDER defect. Neither is visible without both numbers.
V12: spec CAPS ITSELF from commit one. `.context-limits` ceiling lands BEFORE the file grows into it -- a ceiling set after the growth RATIFIES it -- & lands IN THE SAME COMMIT as the runner that EXITS NONZERO on it (V22).
V13: idempotent. `scan(scan(x))` identical; `apply` of an already-extracted id = no-op, exit 0; `plan` of one yields an EMPTY diff, ⊥ an error.
V14: SELF-CONTAINED spec. ⊥ load-bearing reference outside this dir. Lineage rows in §R are EVIDENCE; every invariant stands on its own reasoning.
V15: ⊥ EGRESS of corpus content, ZERO exception. Memory & `CLAUDE.md` hold private user facts. `scan` READS; ⊥ ONE byte leaves the process. ⊥ a LAN host, ⊥ a user-named host, ⊥ an opt-in flag -- an egress path that EXISTS is an egress path that FIRES, & the corpus is the user's private facts.
V16: harness memory dirs are READ-ONLY unless `apply` NAMED that file. A tool that mines memory ! ⊥ corrupt it.
V17: ∀ verb -> BOTH `--format human` & `--format json`, SAME anatomy, ⊥ a verb exempt. An agent ⊥ parse prose, & an unknown format is a USAGE error, ⊥ a silent fall back. ⊥ a json-only verb (a human then reads a wire format to debug) & ⊥ a human-only verb (an agent then regex-scrapes it). ONE exception, & it is ⊥ a verb: `hook` speaks the harness's JSON on BOTH ends ∵ a harness is its only caller.
V18: `recall` & `hook` share ONE matcher. `hook` = `recall` + `M`-rule firing + a stdin/stdout adapter; `recall` is the HUMAN & DEBUG view of the SAME decision. Two matchers is two rule sets (V8), & the INVISIBLE kind: each looks correct alone, & the divergence only shows where a skill fails to load in production but `recall` swears it would.
V19: a PLAN carries a corpus FINGERPRINT: a content hash per source file it touches. `apply <PLAN>` REHASHES & REFUSES on mismatch, exit 1, ⊥ an override flag. Spans are addressed `file:line-line` & the corpus is LIVE prose a human edits between the two commands ∴ a stale plan deletes the WRONG lines from the user's private memory. `revert` restores the BYTES but ⊥ the trust: the artifact was materialized from text that was never the rule, & the ledger records the mistake as if it were intended. V16 makes memory dirs near-sacred; a plan is the only place the promise can be CHECKED.
V20: `apply` CONFIRMS before it mutates: PROMPTS on a tty, DEMANDS `--auto-approve` off-tty & exits 2 without it. ⊥ prompting into a pipe -- that hangs a CI job until someone kills it -- & ⊥ proceeding silently -- that makes the DESTRUCTIVE path the QUIET one. The corpus is the user's private memory ∴ the single verb that deletes from it ! be deliberate, & "deliberate" ! survive being run by a machine.
V21: ∀ §R src is either PUBLICLY RESOLVABLE (a URL, pinned at a ref) or marked `internal` & carrying ⊥ a name. This crate SHIPS ∴ a src an outside reader cannot fetch is ⊥ evidence, it is an ASSERTION wearing a citation's clothes; & a src naming an unpublished repo is a LEAK. VERIFY against the REMOTE ref, ⊥ a local checkout: a checkout is one BRANCH at one MOMENT, & both drift.
V22: ⊥ SHIP A DECLARATION NO RUNNER READS. A ceiling, a registry, a budget, a limit file: if nothing EXITS NONZERO on it, it is a WISH, & wishes drift silently ∵ nothing reports them. The runner lands in the SAME COMMIT as the declaration, ⊥ a task later -- "later" is how a limit file sits unread while every number under it goes over. A command someone REMEMBERS to type is the same defect one step on.
V23: ONE gate DEFINITION, many callers. `hk.pkl` holds the ops; CI CALLS it, ⊥ COPIES it. A second copy in a workflow file is a second rule set that passes review ∵ both halves look right alone -- & the copy is the one that rots, ∵ the local one is the one anybody runs.
V24: a gate step is a PLAIN command a human can PASTE. `hk` decides WHEN a step runs -- which files changed, which hook, what order -- & NEVER what it means to pass. A verdict that rests on the runner's own logic is ⊥ reproducible without the runner.
V25: the GATE is NETWORKLESS, extending V6 to the RUNNER. Its schema is VENDORED, ⊥ fetched at eval. A gate that resolves a URL to decide anything goes soft on a plane, in a locked-down CI, & on the day that host is down.
V26: a MISSING runner is ⊥ a pass. A step this crate OWNS -> FAIL. An OPTIONAL third-party tool -> SKIP, NAMED IN OUTPUT. A silent skip is a pass nobody earned, & it reads GREEN.
V27: a RATCHET moves ONE WAY & its floor TRACKS REALITY. Its `fix` half REFUSES to record a regression -- a ratchet that writes down whatever it measures files down its own teeth on the commit it should have refused. & an UNRECORDED RISE is a FAILURE too: a floor below what the code actually reaches is a floor LYING about what it protects, & every line above it may silently go uncovered again. ∴ new code RAISES the floor, & the gate ⊥ green until it does -- the same shape as `cargo fmt --check` red on an unformatted file, cleared by ONE command.
V28: success is SILENCE; a FAILING gate NAMES THE FIX. Output that ALWAYS appears is output nobody reads ∴ the one real failure hides in noise everyone learned to scroll past.

V29: a TRIGGER is MACHINE-READABLE or it ⊥ FIRES. The fenced `rekall` block IS the trigger; prose beside it is for the human. EXCLUSION WINS -- a ⊥-fire match refuses the load even when the fire block matched, ∵ V4 makes absence a CLAUSE & a clause that loses to a positive match states NOTHING. An `S3` carries an EMPTY block ∴ ⊥ fires, BY CONSTRUCTION: its trigger is SEMANTIC & V5 forbids the model that would notice it. ⊥ a gap -- the LADDER being honest, ∵ V10 already calls a `3` the spec saying this artifact may never fire & V11's `--dead` MEASURES it. `check` REFUSES a block that ⊥ parses (V22).
V30: `[signals]` WEIGHTS decide CLASS, ⊥ SHARPNESS. Class is a BALANCE -- directive evidence against hedge -- ∴ it takes a weight & a DEADBAND, & the deadband IS V10's "I do not know". Sharpness is a LADDER of KINDS (§I: `M2` = runner needs ONE human-set parameter) ∴ ⊥ a score: a sum cannot say WHICH KIND of runner a statement admits, & rounding one to a rung INVENTS the property V10 assigns to the STATEMENT. `show` prints ∀ signal WITH its weight ∴ the balance is ARGUABLE.
V31: a MEASURED figure carries its BOX & DATE, or it is ⊥ EVIDENCE. V21 one level in: `src` says WHO can check it, the box says WHAT ! be re-created to check it. R14's two boxes are ~5x apart ∴ a timing compared across them without both names compares NOTHING, & what survives is the RATIO.
V32: a SCRATCH path is unique PER CALL, ⊥ per process. A pid-named dir is shared by every thread in that process ∴ concurrent runs delete each other's files, & the symptom shows ONLY under load -- the kind that passes review & every hand-run.
V33: a gate MESSAGE is DATA, ⊥ CODE. ⊥ backticks & ⊥ `$(...)` in a shell-quoted advisory: the shell EXECUTES them & the advice is REPLACED by what it ran.

## §T TASKS

id|status|task|cites
T1|x|scaffold: root `flake.nix`, `Cargo.toml`, `hk.pkl` (vendored schema), rustfmt/clippy, MIT, ASCII gate, `mth` in gate|§C,V23,V24,V25
T2|x|`.context-limits` ceiling + its RUNNER, one commit, gate-wired|V12,V22
T3|x|`rekall.toml` loader, `[sources]`, 2 scopes merged|§I,R6
T4|x|corpus reader: roots (file or dir) × globs, sorted & deduped|I.scan,V13,V15,V16
T5|x|statement splitter: prose -> statements, path-scoped hash ids, spans|V13,§I
T6|x|deterministic classifier: signals -> `M`/`S`/`U` × sharpness 1-3|V10,V11,§I
T7|x|`rekall scan` human + json, filters, `--sources`, `-C`|I.scan,V7,V13,V17
T8|x|ledger store `.rekall/ledger.toml`: CANDIDATE & EXTRACTED rows, prefix lookup, fire count|V7,V9,V11,V13
T9|x|`rekall apply` `M` -> script + runner wiring|V1,V2,V13
T10|x|`rekall apply` `S` -> skill + trigger + ⊥-fire clause|V1,V3,V4
T11|x|`rekall check` gate, wired into `hk.pkl`|V2,V3,V6
T12|.|`rekall recall` matcher, reading the trigger BLOCK|I.recall,V3,V29
T13|.|`rekall hook` adapter|I.hook,V5,V29,R9
T14|x|`rekall log` + `--dead`|V11
T15|x|`rekall revert`|V9
T16|x|`itok` delegation for token columns|V8
T18|.|`rekall catch` transcript intake|I.catch,V10
T19|.|measure: tokens reclaimed on THIS repo's own `CLAUDE.md`, consumer #0, box NAMED|R4,R5,V31,T35
T20|.|`set-and-setting` integration: lefthook/`hk` fragment + pinned check|-
T21|x|`rekall init`: detect roots, write `rekall.toml`, refuse to clobber|§I,V7,R6
T22|x|`rekall show <id>`: verbatim text, class, signals, PREFIX ids|§I,V10
T23|x|`rekall plan`: extraction DIFF, artifact + wiring named, `--out` anchored to `-C`|§I,V7
T24|x|plan FINGERPRINT + `apply` REFUSES a stale plan|V19
T25|x|confirm gate on `apply`: tty prompt, `--auto-approve` off-tty. `revert` waits on T15|V20
T26|.|ONE matcher behind `recall` & `hook`, ⊥ two code paths|V18
T27|x|config SCOPE decided: 2 files 1 parser, project wins per key, `[sources]` roots UNION|§I,R6
T28|x|`id` shape DECIDED: path-scoped hash of normalized text, 7 hex, `.n` for repeats, prefix input|§I,T5,V13
T29|.|CONFIRM `catch` persists CANDIDATE rows (assumed, ⊥ chosen)|V7,T8
T30|x|gate step MESSAGES: ∀ failing step names the FIX, ⊥ only the breach|V28
T31|x|gate runner PRESENCE: owned steps FAIL when absent, optional tools SKIP & SAY SO|V26
T32|x|`doctest` step RETURNS once a lib target exists|§C,V22
T33|.|`[signals]` WEIGHTS: config table + weighted CLASS + DEADBAND + weight in `show`; sharpness stays a LADDER|§C,§I,V10,V22,V30
T34|x|trigger BLOCK: `apply` EMITS it, `check` PARSES it, `S3` empty, exclusion WINS|V29,V4,V22
T35|.|this repo gets its own `CLAUDE.md` ∴ consumer #0 has a corpus & `rekall-check`'s glob stops being inert|T19,V26
T36|.|∀ MEASURED figure NAMES its box: re-measure R4, stamp T19's output|V31,R4,R14

## §B BUGS

id|date|cause|fix
B1|2026-08-22|`tokens` scratch dir named by PID alone ∴ concurrent counts in ONE process deleted each other's files mid-count. Symptom appeared ONLY under load, never on a hand-run|V32
B2|2026-08-22|gate message wrote its fix in backticks inside a double-quoted shell string ∴ the shell EXECUTED `direnv` & printed `command not found` WHERE THE ADVICE SHOULD HAVE BEEN|V33
