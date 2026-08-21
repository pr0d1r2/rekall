# SPEC -- rekall

Self-contained spec. `rekall` develops inside a larger workspace but is designed to leave it standalone. It carries its own law: no load-bearing reference outside this directory (V14). Lineage (`itok` - `microlith` - `blackbox`) is SEE-ALSO, cited as evidence, never as authority.

## §G GOAL

Turn always-on agent prose into TANGIBLES: mine agent memory & `CLAUDE.md`-class corpora for statements that are MECHANICAL (become a CPU rule with a runner) or SITUATIONAL (become a skill with a trigger), extract them, and GATE that the extraction stayed honest.

MOTIVATING SHAPE: a rule a model must REMEMBER is already lost. Prose stated at turn 3 competes with everything after it, dies at compaction, and costs window on every turn it does NOT fire. A rule that arrives at its trigger point uninvited costs nothing until it matters.

## §C CONSTRAINTS

- Rust, edition **2024**, MSRV **1.95** = fleet pin (`nixpkgs-lock` -> nixos-26.05). MSRV is MEASURED, ⊥ copied: `itok`/`microlith` declare 1.96 & compile clean on 1.95 (R8).
- ONE bin `rekall`; CRATE `rekall`; REPO `rekall`. MIT.
- NAME: repo = crate = theme = invocation, ONE word, ⊥ a short form ∴ nothing here is an exception to record. 6 chars is the `cargo`/`docker` rung; an `rkl` abbreviation would buy nothing & cost muscle memory. Theme = Total Recall's memory-implant company, named ∵ the job is memory NOBODY has to hold. Three names were rejected & the REASONS are load-bearing: `tr` ∵ POSIX coreutils -- a bin on `PATH` would SHADOW it, hard ⊥ INDEPENDENT of any registry (R1); `total-recall` ∵ TAKEN (R2); `outception` ∵ Inception already NAMES its inverse -- extraction -- ∴ the coinage renames a thing that has a word, & `out-` reads as OUTPUT (R10).
- CPU-ONLY, WHOLE CRATE -- ⊥ a "core" qualifier, ∵ a qualifier is where an exception later hides. ∀ verb deterministic & offline. ⊥ an opt-in inference tier (V5).
- ⊥ NETWORK, ⊥ MODEL, ⊥ INFERENCE, at any flag. The crate runs in a NETWORKLESS SANDBOX. Inference belongs to `blackbox` & is EXPERIMENTAL there (R11) ∴ importing it would make a GATE inherit an experiment's stability, & would stand up a SECOND inference client beside a sibling's -- the defect V8 already names for token counters.
- ASCII-only Rust source (Trojan-Source, LLM-friendliness). SPEC symbols are FORMAT, ⊥ source.
- Corpus is PRIVATE. Agent memory & `CLAUDE.md` hold user facts ∴ ⊥ egress (V15).
- ONE config file `rekall.toml` from commit one. `itok` grew four dotfiles then began migrating back to unified (R6) -- start where it landed.
- Token accounting is `itok`'s, TOKENIZER-backed ∴ counts are MEASURED & deterministic, ⊥ estimated by a model & ⊥ a char/4 heuristic. ⊥ reimplemented here (V8).
- `SPEC.md` FORMAT owned by `microlith`, enforced by `mth` in the gate.
- Zero host-project internals ∴ extraction of THIS crate is a move.
- Gate = `hk` (`hk.pkl`), same definition local & CI. Coverage floor RATCHETS.

## §I INTERFACES

- `rekall scan [<path>...]` -- `--format human|json` · `--class M|S|U` · `--sharpness 1|2|3` · `--top N` · `--sources` · `-C <dir>`. Inventory the corpus: one row per STATEMENT -- `id` · `src` (`file:line-line`) · `tokens` · `class` · `sharpness` · `signals`. Deterministic, report-only. THE CPU CORE.
- CLASS × SHARPNESS. Class = `M`|`S`|`U`, R7's vocabulary UNCHANGED. Sharpness = `1`|`2`|`3`, printed joined ∴ `M2`. Sharpness is a property of the STATEMENT, ⊥ of the classifier: how sharp a runner or trigger the statement ADMITS, ⊥ how confident the classifier FEELS. `M1` runner deterministic, ⊥ judgment · `M2` runner needs ONE human-set parameter · `M3` runner DETECTS, ⊥ resolves. `S1` trigger EXACT (tool · path · extension) · `S2` trigger a signal-matchable CLASS of situations · `S3` trigger SEMANTIC ∴ only a model notices it. `U` carries ⊥ sharpness -- absence of a class has no ladder. Both ladders run SHARP -> FUZZY ∴ the digit PREDICTS fire rate & the `3` rows are where `--dead` (V11) comes from. ⊥ a third word (R7): `M`/`S` stand, the digit is a DEGREE. `--class M` matches ∀ `M*`; `--sharpness` filters across classes. JSON carries `class` · `sharpness` · `label` as SEPARATE fields ∴ an agent ⊥ string-surgery `M2` (V17).
- `rekall plan <id>...` -- `--format human|json` · `--out FILE` · `--to <dir>`. The DIFF of an extraction: per statement, the source span to DELETE, the artifact to WRITE, the wiring to ADD. Report-only, writes ⊥ except the plan file itself. `--out` makes the plan an ARTIFACT ∴ reviewable, diffable, committable before a byte of corpus moves.
- `rekall apply <id>... | <PLAN>` -- `--format human|json` · `--auto-approve` · `--to <dir>`. EXECUTE. `M` -> script + runner wiring. `S` -> skill file + trigger. Deletes the source span & leaves a pointer (V1). Names EVERY file touched. The ONLY mutating verb besides `catch`/`revert`. Given a PLAN it re-checks the corpus FINGERPRINT first & REFUSES a stale plan (V19). Confirms on a tty; off-tty demands `--auto-approve` (V20).
- `rekall check` -- `--format human|json`. THE GATE. ∀ extracted `M` ! has a runner (V2) · ∀ `S` ! has a trigger (V3) & a ⊥-fire clause (V4) · ⊥ orphan artifact · ⊥ source span still present. Exit 1 on drift. CPU-only ∴ runs in `hk` & CI with no key & no network (V6).
- `rekall recall <situation>` -- `--tool X` · `--path P` · `--cwd D` · `--format json`. Which situational skills ! load HERE. Deterministic matcher, report-only. This is the reload rule V3 demands.
- `rekall hook` -- harness hook JSON on stdin -> decision JSON on stdout. ADAPTER shape: no daemon, no interception, ⊥ in the request path. Signals in JSON, ⊥ via exit code. Fires `M` rules & injects `S` skills at the TRIGGER point.
- `rekall log` -- `--format json` · `--dead` · `--since D`. READS the ledger. Per artifact: source span · original text · artifact path · FIRE count · tokens reclaimed. `--dead` = never fired (V11). VERB is `log`, STORE is the ledger -- the store is a ledger & is called one everywhere it is described.
- `rekall catch [<session>]` -- `--format json`. Second intake: a VIOLATION in a transcript -> candidate statement, classed like any other. Report-only; promotion goes through `plan` then `apply`.
- `rekall revert <id>` -- reverse one extraction from the ledger, verbatim (V9).
- Config: `rekall.toml` -- `[sources]` corpus roots & globs · `[signals]` classifier weights · `[triggers]` matcher defaults · `[budget]` ceilings.
- Exit: 0 ok · 1 drift/violation · 2 usage. ⊥ a network code ∵ ⊥ a network path.

## §R RESEARCH

id|topic|finding|src
R1|name `tr`|TAKEN crates.io v0.1.11 (i18n) AND `tr` = POSIX coreutils ∴ bin would SHADOW it -- ⊥ regardless of registry|crates.io/api/v1/crates/tr
R2|name `total-recall`|TAKEN crates.io v0.3.0, GUI to-do app ∴ crate name unavailable|crates.io/api/v1/crates/total-recall
R3|name `rekall`|FREE on crates.io @ 2026-08-21 ∴ repo · crate · bin collapse to ONE word, no bend to document|crates.io/api/v1/crates/rekall
R4|context ceiling|`itok` = 136,811 tok (spec+code) vs 102,529 WORKING on a 24GB M-series box @ 131,072 window ∴ a small disciplined tool ⊥ fit its own best-case hardware|../blackbox/README.md
R5|conditional load|~50% of a repo never loads for impl work; facet × horizontal brings one node to ~6% ∴ trigger-gated load is MEASURED, ⊥ hoped|../blackbox/README.md
R6|config sprawl|`itok` grew `.context-limits` · `.context-models` · `.context-policy` · `.context-hosts`, then began migrating to `itok.toml` (its V109) ∴ unified from commit one|../itok/SPEC.md §I
R7|class taxonomy|`mth check` already ranks each direction `Mechanical` \| `Judgment` -- the SAME 2-class split this crate needs ∴ reuse the vocabulary, ⊥ invent a third word|../microlith/SPEC.md §I
R8|MSRV|`blackbox` MEASURED 1.95 clean while siblings DECLARE 1.96 ∴ the sibling floor is a stale pin mirror, ⊥ a minimum|../blackbox/SPEC.md §C
R9|adapter shape|`itok guard` = hook JSON stdin -> decision JSON stdout, opt-in, ⊥ in request path (its V52/V53) ∴ proven shape, copy it|../itok/SPEC.md §I
R10|name `outception`|FREE, but Inception's OWN word for the inverse of inception is EXTRACTION ∴ the coinage names an already-named thing; `extraction` also free but a generic-noun squat|the film's vocabulary
R11|inference|`blackbox` OWNS ollama: `src/ollama` 23,032B, its own hardware §R rows (`gpt-oss:20b` ctx 131,072 @ a LAN box), coverage 70.9% -- 13th of 14 nodes ∴ EXPERIMENTAL, ⊥ a foundation. A client here would DUPLICATE a sibling module & bind this crate's GATE to a moving target|../blackbox/SPEC.md §C,R9,R34,R50

## §V INVARIANTS

V1: extraction is a MOVE, ⊥ a copy. Source span deleted (pointer left) in the SAME commit the artifact lands. A copy leaves two hand-maintained statements of one rule -- `microlith`'s founding defect -- and leaves the context cost UNPAID ∴ the whole purpose lost.
V2: ∀ extracted `M` -> a RUNNER, same commit. Rule with no runner gates nothing.
V3: ∀ extracted `S` -> a TRIGGER. A skill with no trigger is always-on prose, which is exactly what it was extracted FROM.
V4: ∀ trigger -> an explicit ⊥-fire clause, ⊥ only a fire clause. Absence ⊥ provable from a positive description (`blackbox`'s `⊥owns` byte, R5).
V5: CPU-ONLY, ∀ verb, ⊥ exception. Deterministic & offline everywhere. ⊥ model, ⊥ network, ⊥ inference tier at ANY opt-in. Inference is `blackbox`'s (R11) ∴ ⊥ reimplemented here -- two inference clients is exactly the defect V8 names for two token counters. An opt-in that CAN fire is a path that WILL fire, & then the classifier's floor is a remote model's uptime.
V6: `check` is the GATE ∴ CPU-only, no key, no network. A gate needing a model runs nowhere it is needed.
V7: report-only DEFAULT. Only `apply` · `catch` -> ledger · `revert` mutate, & each NAMES every file touched before writing.
V8: token counts DELEGATED to `itok`. Two estimators disagreeing is the same defect as two rule sets.
V9: ∀ extraction REVERSIBLE. Ledger holds source path · line span · ORIGINAL TEXT · artifact path ∴ `revert` is mechanical, ⊥ a rewrite.
V10: a class is a CLAIM, ⊥ truth. ∀ row carries the SIGNALS that fired & its SHARPNESS. `U` (unknown) is legal & is the DEFAULT. A classifier that never says "I do not know" is lying at a fixed rate. Sharpness is DERIVED from the same signals ∴ also a claim, & a `3` is the spec SAYING OUT LOUD that this artifact may never fire.
V11: ledger counts FIRES ∴ a never-fired artifact is DETECTABLE. A rule that never fires is a wrong trigger or dead law; both need to be visible, ⊥ inferred. SHARPNESS predicts what `--dead` MEASURES ∴ the two are checkable against each other: a `1` gone dead is a CLASSIFIER defect, a `3` that fires often is a LADDER defect. Neither is visible without both numbers.
V12: spec CAPS ITSELF from commit one. `.context-limits` ceiling lands BEFORE the file grows into it -- a ceiling set after the growth RATIFIES it (`microlith` V9).
V13: idempotent. `scan(scan(x))` identical; `apply` of an already-extracted id = no-op, exit 0; `plan` of one yields an EMPTY diff, ⊥ an error.
V14: SELF-CONTAINED spec. ⊥ load-bearing reference outside this dir. Lineage rows in §R are EVIDENCE; every invariant stands on its own reasoning.
V15: ⊥ EGRESS of corpus content, ZERO exception. Memory & `CLAUDE.md` hold private user facts. `scan` READS; ⊥ ONE byte leaves the process. ⊥ a LAN host, ⊥ a user-named host, ⊥ an opt-in flag -- an egress path that EXISTS is an egress path that FIRES, & the corpus is the user's private facts.
V16: harness memory dirs are READ-ONLY unless `apply` NAMED that file. A tool that mines memory ! ⊥ corrupt it.
V17: ∀ verb -> `--format json` with the SAME anatomy as its human output. An agent ⊥ parse prose, & an unknown format is a USAGE error, ⊥ a silent fall back.

## §T TASKS

id|status|task|cites
T1|.|scaffold: flake, `Cargo.toml`, `hk.pkl`, rustfmt/clippy, MIT, ASCII gate, `mth` in gate|-
T2|.|`.context-limits` ceiling set before growth|V12
T3|.|`rekall.toml` loader, `[sources]` only|§C,R6
T4|.|corpus reader: memory dir · `CLAUDE.md` · `AGENTS.md` · skill dirs|I.scan,V15,V16
T5|.|statement splitter: prose -> addressable statements, STABLE ids across edits|V13
T6|.|deterministic classifier: signals -> `M`/`S`/`U` × sharpness 1-3|V10,V11,§I
T7|.|`rekall scan` human + json|I.scan,V7,V17
T8|.|ledger store: span · original text · artifact · fire count|V9,V11
T9|.|`rekall apply` `M` -> script + runner wiring|V1,V2
T10|.|`rekall apply` `S` -> skill + trigger + ⊥-fire clause|V1,V3,V4
T11|.|`rekall check` gate, wired into `hk.pkl`|V2,V3,V6
T12|.|`rekall recall` matcher|I.recall,V3
T13|.|`rekall hook` adapter|I.hook,V5,R9
T14|.|`rekall log` + `--dead`|V11
T15|.|`rekall revert`|V9
T16|.|`itok` delegation for token columns|V8
T18|.|`rekall catch` transcript intake|I.catch,V10
T19|.|measure: tokens reclaimed on THIS repo's own corpus, consumer #0|R4,R5
T20|.|`set-and-setting` integration: lefthook/`hk` fragment + pinned check|-

## §B BUGS

id|date|cause|fix
