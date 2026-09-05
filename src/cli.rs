//! Argument parsing and dispatch.
//!
//! Hand-rolled rather than derived. The verb set is small and fixed by
//! section I, and the exit codes are part of the published contract -- 0 ok,
//! 1 drift, 2 usage -- so the mapping from argument to exit stays visible
//! here instead of inside a macro.

// The crate modules are reached by FULL PATH inside the verb modules,
// because `mod apply;` below would otherwise shadow `crate::apply`.
use crate::{statement, tokens};
use std::path::{Path, PathBuf};

/// One module per VERB. `cli.rs` keeps dispatch, the shared parse
/// helpers and the two report shapes; each verb took its own flags,
/// orchestration, renderer and tests with it.
///
/// This file reached 1,878 lines of code while every FUNCTION in it
/// passed its limits -- clippy caps methods and parameters but nothing
/// caps a FILE, so the one dimension nobody measured is the one that
/// drifted (V22, again). The gate measures it now.
mod apply;
mod catch;
mod check;
mod hook;
mod init;
mod log;
mod plan;
mod recall;
mod resolve;
mod revert;
mod scan;
mod show;

pub use apply::{
    ApplyArgs, Consent, NEEDS_APPROVAL, NO_APPLY_INPUT, apply_command,
    approved, parse_apply, read_answer, render_apply_human,
};
pub use check::{CheckArgs, Checked, check_command, parse_check};
pub use hook::hook_command;
pub use init::{InitArgs, init_command, parse_init};
pub use log::{LogArgs, log_command, parse_log};
pub use plan::{NO_PLAN_IDS, PlanArgs, parse_plan, plan_command};
pub use recall::{RecallArgs, parse_recall, recall_command};
pub use resolve::{Resolved, resolve};
pub use revert::{
    NO_REVERT_INPUT, RevertArgs, parse_revert, render_revert_human,
    revert_command,
};
pub use scan::{ScanArgs, parse_scan, scan_command};
pub use show::{NO_ID, ShowArgs, parse_show, show_command};

pub const USAGE_EXIT: u8 = 2;

/// Section I fixes the exit set: 0 ok, 1 DRIFT or violation, 2 usage. A
/// failing `check` is drift, not a usage mistake, and a gate that reported
/// both the same way would make "you typed it wrong" and "your corpus is
/// wrong" indistinguishable to the caller that has to react.
pub const DRIFT_EXIT: u8 = 1;

#[derive(Debug, PartialEq, Eq)]
pub enum Format {
    Human,
    Json,
}

/// A flag that takes a value, and says so when it did not get one.
/// NET tokens an extraction reclaims: the statement, LESS the pointer that
/// replaces it (V39).
///
/// Statements and pointers are counted in ONE `itok` call, not two. The
/// sibling pays a tokenizer-table load per process, so a second call would
/// double the entire cost of the measurement.
///
/// A `None` anywhere -- absent sibling, unreadable line -- yields `None`
/// for that row rather than a half-computed number. An arithmetic result
/// missing one of its terms is worse than no result.
pub(super) fn net_reclaim(
    ids: &[String],
    texts: &[String],
) -> Vec<Option<i64>> {
    let mut all: Vec<String> = texts.to_vec();
    all.extend(ids.iter().map(|id| crate::apply::pointer_of(id)));
    let Ok(counted) = tokens::count_all(&all) else {
        return vec![None; texts.len()];
    };
    (0..texts.len())
        .map(|at| one_net(&counted, at, texts.len()))
        .collect()
}

/// One row's arithmetic. The pointer for row `at` sits `len` places later
/// in the same batch, because both were counted in a single call.
fn one_net(counted: &[Option<u32>], at: usize, len: usize) -> Option<i64> {
    let text = counted.get(at).copied().flatten()?;
    let pointer = at
        .checked_add(len)
        .and_then(|n| counted.get(n))
        .copied()
        .flatten()?;
    i64::from(text).checked_sub(i64::from(pointer))
}

pub(super) fn need<'a>(
    flag: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<String, String> {
    rest.next()
        .cloned()
        .ok_or_else(|| format!("`{flag}` needs a value"))
}

/// An unknown `--format` is a USAGE , never a silent fall back to
/// prose (V17). An agent that asked for json and received a table would
/// parse garbage rather than fail.
pub fn parse_format(raw: &str) -> Result<Format, String> {
    match raw {
        "human" => Ok(Format::Human),
        "json" => Ok(Format::Json),
        other => Err(format!(
            "unknown --format `{other}` -- expected `human` or `json`"
        )),
    }
}

/// Every verb SPEC.md section I defines, in the order it lists them.
pub const VERBS: [&str; 11] = [
    "init", "scan", "show", "plan", "apply", "check", "recall", "hook", "log",
    "catch", "revert",
];

pub const USAGE: &str = "\
rekall -- extract always-on agent prose into rules and skills

usage: rekall <verb> [args]

verbs:
  init    detect corpus roots and write rekall.toml
  scan    inventory the corpus, one row per statement
  show    one statement or artifact in full
  plan    diff an extraction, optionally into a plan file
  apply   execute an extraction; confirms before it mutates
  check   the gate -- exit 1 on drift
  recall  which situational skills load here
  hook    harness hook JSON on stdin, decision JSON on stdout
  log     read the ledger
  catch   mine a transcript for candidate statements
  revert  reverse one extraction, verbatim; confirms before it mutates

exit: 0 ok, 1 drift or violation, 2 usage
";

/// What the binary should do, decided without performing any of it.
///
/// Separating the DECISION from the printing is what makes the dispatch
/// testable: a test asserts on this value, while `dispatch` does the
/// unavoidable side effects in three lines nothing can cover.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    PrintUsage {
        code: u8,
    },
    PrintVersion,
    Scan(Vec<String>),
    Init(Vec<String>),
    Show(Vec<String>),
    Plan(Vec<String>),
    Apply(Vec<String>),
    Revert(Vec<String>),
    Catch(Vec<String>),
    Check(Vec<String>),
    Log(Vec<String>),
    Recall(Vec<String>),
    Hook(Vec<String>),
    /// A verb section I defines that this build cannot perform.
    Unimplemented(String),
    /// A verb the binary has never heard of -- a typo, not a backlog row.
    Unknown(String),
}

/// Decide what a command line asks for.
///
/// An unimplemented verb and an unknown one carry DIFFERENT messages and
/// the SAME exit code. Section I fixes the exit set at 0 ok, 1 drift, 2
/// usage, and neither case is drift -- inventing a fourth code to make a
/// scaffold more expressive would change a published contract for a state
/// that stops existing once the verbs land.
fn rest(args: &[String]) -> Vec<String> {
    args.get(1..).unwrap_or_default().to_vec()
}

#[must_use]
pub fn decide(args: &[String]) -> Action {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => Action::PrintUsage { code: 0 },
        Some("-V" | "--version") => Action::PrintVersion,
        Some(verb) => verb_action(verb, rest(args)),
    }
}

/// The verb table. Split from `decide` when the line limit fired on it,
/// and the split is a real seam: the two flag cases above are fixed
/// forever, while this table grows by one row per verb the crate learns.
/// The verb table as DATA. A tuple variant is already a constructor
/// function, so the mapping needs no match arm per verb -- which is what
/// the line limit kept objecting to as this list grew.
type Make = fn(Vec<String>) -> Action;
const IMPLEMENTS: [(&str, Make); 11] = [
    ("scan", Action::Scan),
    ("init", Action::Init),
    ("show", Action::Show),
    ("plan", Action::Plan),
    ("apply", Action::Apply),
    ("revert", Action::Revert),
    ("check", Action::Check),
    ("log", Action::Log),
    ("recall", Action::Recall),
    ("hook", Action::Hook),
    ("catch", Action::Catch),
];

fn verb_action(verb: &str, flags: Vec<String>) -> Action {
    if let Some((_, make)) = IMPLEMENTS.iter().find(|(name, _)| *name == verb) {
        return make(flags);
    }
    if VERBS.contains(&verb) {
        return Action::Unimplemented(verb.to_string());
    }
    Action::Unknown(verb.to_string())
}

/// Perform the decided action and return the EXIT CODE as a value.
///
/// A value rather than an `ExitCode` so tests can assert on it: `ExitCode`
/// is deliberately opaque and implements no comparison, which would leave
/// every arm here exercised but unasserted. `dispatch` converts once.
/// The process facts the library needs but must not READ for itself.
///
/// `current_dir` and `HOME` are process globals. A library that reaches for
/// them is a library whose behaviour depends on state no caller passed and
/// no test can set -- `std::env::set_var` is unsafe in edition 2024, and
/// mutating it would race other tests anyway. The binary reads them once,
/// at the edge, and hands them in.
#[derive(Debug, Clone)]
pub struct Env {
    pub cwd: PathBuf,
    pub home: Option<String>,
}

#[must_use]
pub fn perform(action: Action, env: &Env) -> u8 {
    match action {
        Action::PrintUsage { code } => show_usage(code),
        Action::PrintVersion => show_version(),
        // The two verbs that do NOT answer with an `Output`: `check`
        // carries its verdict in the exit code (section I's drift 1), and
        // `hook` speaks the harness's JSON on both ends (V17's exception).
        Action::Check(flags) => run_check(&flags, env),
        Action::Hook(flags) => hook::run_hook(&flags, env),
        Action::Unimplemented(verb) => say_unimplemented(&verb),
        Action::Unknown(other) => say_unknown(&other),
        reporting => emit(reported(reporting, env)),
    }
}

/// Every verb that answers with an `Output` -- same shape, same emit.
///
/// Split from `perform` when the line limit fired, along the seam that
/// was already there: these eight are interchangeable at the call site
/// and the four above are each special for a stated reason.
fn reported(action: Action, env: &Env) -> Result<Output, String> {
    match action {
        Action::Scan(flags) => scan_command(&flags, env),
        Action::Init(flags) => init_command(&flags, env),
        Action::Show(flags) => show_command(&flags, env),
        Action::Plan(flags) => plan_command(&flags, env),
        Action::Apply(flags) => apply_command(&flags, env),
        Action::Revert(flags) => revert_command(&flags, env),
        Action::Log(flags) => log_command(&flags, env),
        Action::Catch(flags) => catch::catch_command(&flags, env),
        Action::Recall(flags) => recall_command(&flags, env),
        other => Err(format!("{other:?} does not report an Output")),
    }
}

fn show_usage(code: u8) -> u8 {
    print!("{USAGE}");
    code
}

fn show_version() -> u8 {
    println!("rekall {}", env!("CARGO_PKG_VERSION"));
    0
}

fn say_unimplemented(verb: &str) -> u8 {
    eprintln!(
        "rekall: `{verb}` is specified but not implemented in this build"
    );
    USAGE_EXIT
}

fn say_unknown(other: &str) -> u8 {
    eprintln!("rekall: unknown verb `{other}`\n");
    show_usage(USAGE_EXIT)
}

/// Print an outcome and return its exit code.
///
/// EVERY verb ends this way, so it is written once. Six copies of the same
/// four lines is six chances for one verb to print its warnings to stdout,
/// or to exit 0 on an error, and each copy looks correct on its own -- the
/// divergence only shows to whoever piped that one verb into something.
fn emit(result: Result<Output, String>) -> u8 {
    match result {
        Ok(output) => {
            for warning in &output.warnings {
                eprintln!("rekall: {warning}");
            }
            print!("{}", output.text);
            0
        }
        Err(message) => {
            eprintln!("rekall: {message}");
            USAGE_EXIT
        }
    }
}

/// Drift exits 1, a broken invocation exits 2 (section I).
fn run_check(flags: &[String], env: &Env) -> u8 {
    match check_command(flags, env) {
        Ok(checked) => report_check(&checked),
        Err(message) => {
            eprintln!("rekall: {message}");
            USAGE_EXIT
        }
    }
}

fn report_check(checked: &Checked) -> u8 {
    print!("{}", checked.output.text);
    if checked.drift == 0 {
        return 0;
    }
    eprintln!(
        "rekall: {} extraction problem(s). Each line above names the fix.",
        checked.drift
    );
    DRIFT_EXIT
}

/// What a scan produced for a caller to print.
#[derive(Debug)]
pub struct Output {
    pub text: String,
    /// Files that could not be read. NAMED, never silently dropped:
    /// reporting a smaller corpus as if it were the whole one is the skip
    /// V26 forbids.
    pub warnings: Vec<String>,
}

/// Put counts on rows, or turn the fault into the line that says why the
/// column is empty (V26).
///
/// Split from its callers so the SKIP path is testable. The alternative
/// would be a test that removes `itok` from PATH, and `set_var` is unsafe
/// in edition 2024 and would race every other test in the process -- so
/// that path would go permanently unverified while looking covered.
pub(super) fn spread<T>(
    into: &mut [T],
    counted: Result<Vec<Option<u32>>, tokens::Fault>,
    set: impl Fn(&mut T, Option<u32>),
) -> Vec<String> {
    match counted {
        Ok(counts) => {
            for (item, count) in into.iter_mut().zip(counts) {
                set(item, count);
            }
            Vec::new()
        }
        Err(fault) => vec![fault.to_string()],
    }
}

pub const NO_SOURCES: &str = "no corpus roots configured. \
Run `rekall init` to detect them, or add [sources].roots to rekall.toml";

pub(super) fn load_corpus(
    base: &Path,
    env: &Env,
) -> Result<crate::scan::Loaded, String> {
    let resolved = resolve(base)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    let at = crate::scan::Corpus {
        roots: &resolved.roots,
        globs: &resolved.globs,
        home: env.home.as_deref(),
        base,
        weights: &resolved.weights,
    };
    crate::scan::load(&at).map_err(|error| error.to_string())
}

/// Resolve each id prefix to exactly one statement.
///
/// Every id is resolved BEFORE any step is built, so a typo in the third
/// id does not produce a partial plan for the first two.
pub(super) fn choose(
    statements: &[statement::Statement],
    ids: &[String],
) -> Result<Vec<statement::Statement>, String> {
    let mut out = Vec::new();
    for id in ids {
        out.push(one(statements, id)?);
    }
    Ok(out)
}

pub(super) fn one(
    statements: &[statement::Statement],
    id: &str,
) -> Result<statement::Statement, String> {
    let hits: Vec<&statement::Statement> =
        statements.iter().filter(|s| s.id.starts_with(id)).collect();
    match hits.as_slice() {
        [] => Err(crate::plan::Error::Unknown(id.to_string()).to_string()),
        [only] => Ok((*only).clone()),
        many => Err(crate::plan::Error::Ambiguous(
            id.to_string(),
            many.iter().map(|s| s.id.clone()).collect(),
        )
        .to_string()),
    }
}
/// Report what a MUTATING verb did, in either format.
///
/// `apply` and `revert` differ in what they RENDER, not in how they
/// report: the machine-readable outcome goes to stdout and the list of
/// what actually happened to stderr, so `--format json` stays parseable
/// while the receipt is still visible (V17). Writing that branch twice
/// would be two rule sets of the smallest and most forgettable kind.
pub(super) fn report<T: serde::Serialize>(
    outcome: &T,
    json: bool,
    done: Vec<String>,
    human: impl Fn(&T, &[String]) -> String,
) -> Result<Output, String> {
    if json {
        let text = serde_json::to_string_pretty(outcome)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string())?;
        return Ok(Output {
            text,
            warnings: done,
        });
    }
    Ok(Output {
        text: human(outcome, &done),
        warnings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    // The CRATE modules, named explicitly. `use super::*` now also pulls in
    // the cli submodules of the same name, and an explicit import beats a
    // glob -- so this is what keeps `plan::Plan` meaning the domain type.
    use super::apply as apply_cli;
    use super::scan::{render, set_row_tokens, warnings};
    use crate::{apply, check, ledger, plan, scan};

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    /// A deterministic environment. Tests never read the real one: the
    /// point of `Env` is that process globals arrive as arguments.
    fn env() -> Env {
        Env {
            cwd: PathBuf::from("."),
            home: None,
        }
    }

    #[test]
    fn format_accepts_the_two_documented_values() {
        assert_eq!(parse_format("human").ok(), Some(Format::Human));
        assert_eq!(parse_format("json").ok(), Some(Format::Json));
    }

    /// V17: an unknown format is a USAGE error, never a silent fall back to
    /// prose. An agent that asked for json and got a table parses garbage.
    #[test]
    fn an_unknown_format_is_an_error_not_a_fallback() {
        assert!(parse_format("yaml").is_err());
    }

    #[test]
    fn no_flags_is_a_valid_scan() {
        let parsed = parse_scan(&args(&[]));
        assert!(parsed.is_ok());
    }

    #[test]
    fn flags_populate_the_filter() {
        let parsed = parse_scan(&args(&[
            "--class",
            "M",
            "--sharpness",
            "2",
            "--top",
            "5",
        ]));
        let filter = parsed.map(|parsed| parsed.filter).ok();
        assert_eq!(
            filter.as_ref().and_then(|f| f.class.clone()),
            Some("M".to_string())
        );
        assert_eq!(filter.as_ref().and_then(|f| f.sharpness), Some(2));
        assert_eq!(filter.and_then(|f| f.top), Some(5));
    }

    #[test]
    fn sources_and_json_are_flags_without_values() {
        let parsed = parse_scan(&args(&["--sources", "--format", "json"]));
        assert_eq!(
            parsed.map(|parsed| (parsed.sources, parsed.json)).ok(),
            Some((true, true))
        );
    }

    #[test]
    fn dash_c_sets_the_working_directory() {
        let parsed = parse_scan(&args(&["-C", "/tmp/x"]));
        assert_eq!(
            parsed.map(|parsed| parsed.cwd).ok().flatten(),
            Some(PathBuf::from("/tmp/x"))
        );
    }

    /// A truncated command line is an error rather than a default. Guessing
    /// what `--top` meant is how a scan silently reports the wrong slice.
    #[test]
    fn a_flag_missing_its_value_is_an_error() {
        assert!(parse_scan(&args(&["--top"])).is_err());
        assert!(parse_scan(&args(&["--class"])).is_err());
        assert!(parse_scan(&args(&["-C"])).is_err());
    }

    #[test]
    fn an_unknown_flag_is_an_error() {
        assert!(parse_scan(&args(&["--nope"])).is_err());
    }

    #[test]
    fn sharpness_outside_the_ladder_is_rejected() {
        assert!(parse_scan(&args(&["--sharpness", "4"])).is_err());
        assert!(parse_scan(&args(&["--sharpness", "0"])).is_err());
    }

    #[test]
    fn top_must_be_a_whole_number() {
        assert!(parse_scan(&args(&["--top", "-1"])).is_err());
        assert!(parse_scan(&args(&["--top", "many"])).is_err());
    }

    #[test]
    fn a_scan_with_no_configured_roots_says_what_to_run() {
        let outcome = scan_command(&args(&["-C", "/tmp"]), &env());
        assert!(
            outcome.err().is_some_and(|m| m.contains("rekall init")),
            "an unconfigured project must say what to run"
        );
    }

    #[test]
    fn an_unknown_flag_reaches_the_caller_as_an_error() {
        assert!(scan_command(&args(&["--nope"]), &env()).is_err());
    }

    /// V26: an absent sibling leaves the column empty and SAYS SO. The
    /// report still happens -- a missing column is a smaller loss than a
    /// verb that refuses to run.
    #[test]
    fn a_token_fault_becomes_a_named_warning_and_leaves_the_column_empty() {
        let mut rows = vec![row_for_spread()];
        let said =
            spread(&mut rows, Err(tokens::Fault::Absent), set_row_tokens);
        assert_eq!(said.len(), 1);
        assert!(
            said.first().is_some_and(|line| line.contains(tokens::TOOL)),
            "{said:?}"
        );
        assert_eq!(rows.first().and_then(|row| row.tokens), None);
    }

    #[test]
    fn counts_land_on_the_rows_they_belong_to() {
        let mut rows = vec![row_for_spread(), row_for_spread()];
        let said = spread(&mut rows, Ok(vec![Some(7), None]), set_row_tokens);
        assert!(said.is_empty(), "{said:?}");
        assert_eq!(
            rows.iter().map(|row| row.tokens).collect::<Vec<_>>(),
            vec![Some(7), None]
        );
    }

    fn row_for_spread() -> scan::Row {
        scan::Row {
            id: "aaa".to_string(),
            src: "CLAUDE.md:1-1".to_string(),
            tokens: None,
            class: "M".to_string(),
            sharpness: Some(1),
            label: "M1".to_string(),
            signals: Vec::new(),
        }
    }

    #[test]
    fn warnings_are_empty_when_everything_was_readable() {
        assert!(warnings(&empty_outcome()).is_empty());
    }

    #[test]
    fn an_unreadable_file_is_named_in_the_warnings() {
        let mut outcome = empty_outcome();
        outcome.unreadable = vec![PathBuf::from("/secret/notes.md")];
        let found = warnings(&outcome);
        assert_eq!(found.len(), 1);
        assert!(
            found
                .first()
                .is_some_and(|w| w.contains("/secret/notes.md")),
            "warnings were {found:?}"
        );
    }

    fn empty_outcome() -> scan::Outcome {
        scan::Outcome {
            report: scan::Report {
                rows: Vec::new(),
                sources: Vec::new(),
            },
            unreadable: Vec::new(),
            texts: Vec::new(),
        }
    }

    #[test]
    fn json_output_is_parseable_and_carries_the_report_anatomy() {
        let json_args = ScanArgs {
            json: true,
            ..ScanArgs::default()
        };
        assert!(
            render(&empty_outcome(), &json_args)
                .is_ok_and(|text| text.contains("\"rows\"")
                    && text.contains("\"sources\"")),
        );
    }

    #[test]
    fn human_output_is_not_json() {
        assert!(
            render(&empty_outcome(), &ScanArgs::default())
                .is_ok_and(|text| !text.contains("\"rows\"")),
        );
    }

    #[test]
    fn no_arguments_prints_usage_and_succeeds() {
        assert_eq!(decide(&args(&[])), Action::PrintUsage { code: 0 });
    }

    #[test]
    fn help_flags_print_usage() {
        assert_eq!(decide(&args(&["-h"])), Action::PrintUsage { code: 0 });
        assert_eq!(decide(&args(&["--help"])), Action::PrintUsage { code: 0 });
    }

    #[test]
    fn version_flags_are_recognized() {
        assert_eq!(decide(&args(&["-V"])), Action::PrintVersion);
        assert_eq!(decide(&args(&["--version"])), Action::PrintVersion);
    }

    #[test]
    fn scan_carries_its_remaining_flags() {
        assert_eq!(
            decide(&args(&["scan", "--top", "3"])),
            Action::Scan(args(&["--top", "3"]))
        );
    }

    /// A verb the spec defines but this build cannot perform is a BACKLOG
    /// ROW, and says so. A verb nobody has heard of is a typo. Same exit
    /// code, different message -- the distinction is the message's job.
    /// The verbs that actually do something. Kept beside the loop below so
    /// implementing a verb without dispatching it fails here.
    const IMPLEMENTED: [&str; 11] = [
        "scan", "init", "show", "plan", "apply", "revert", "check", "log",
        "recall", "hook", "catch",
    ];

    /// EVERY verb section I names now dispatches, so the loop below has an
    /// empty body -- and that is the assertion. `catch` was the last
    /// backlog row, and this test failed on the commit that landed it,
    /// which is the failure a stale test is supposed to produce.
    ///
    /// The `Unimplemented` arm STAYS. It is not dead: section I can name a
    /// verb before this build performs it, and the day it does, the
    /// distinction between a backlog row and a typo has to already exist.
    /// Deleting it would mean rebuilding it under pressure.
    #[test]
    fn a_specified_verb_is_unimplemented_not_unknown() {
        for verb in VERBS {
            if IMPLEMENTED.contains(&verb) {
                continue;
            }
            assert_eq!(
                decide(&args(&[verb])),
                Action::Unimplemented(verb.to_string()),
                "{verb} should be recognized"
            );
        }
    }

    /// The other half, which the loop above can no longer prove now that
    /// nothing is unimplemented: an unheard-of verb is still a typo.
    #[test]
    fn an_unheard_of_verb_is_a_typo_not_a_backlog_row() {
        assert_eq!(
            decide(&args(&["teleport"])),
            Action::Unknown("teleport".to_string())
        );
    }

    #[test]
    fn an_unrecognized_verb_is_unknown() {
        assert_eq!(
            decide(&args(&["scna"])),
            Action::Unknown("scna".to_string())
        );
    }

    /// The usage text and the verb list must not drift apart. They are the
    /// same eleven verbs section I defines, so the text is checked against
    /// the const rather than proof-read.
    #[test]
    fn usage_text_names_every_verb() {
        for verb in VERBS {
            assert!(USAGE.contains(verb), "usage text is missing {verb}");
        }
    }

    #[test]
    fn usage_text_states_the_exit_codes() {
        assert!(USAGE.contains("0 ok, 1 drift or violation, 2 usage"));
    }
    #[test]
    fn usage_exits_zero_when_asked_for() {
        assert_eq!(perform(Action::PrintUsage { code: 0 }, &env()), 0);
    }

    #[test]
    fn version_exits_zero() {
        assert_eq!(perform(Action::PrintVersion, &env()), 0);
    }

    /// Both are usage errors -- section I fixes the exit set, and neither
    /// an unknown verb nor an unimplemented one is drift.
    #[test]
    fn unknown_and_unimplemented_both_exit_two() {
        assert_eq!(
            perform(Action::Unimplemented("plan".to_string()), &env()),
            USAGE_EXIT
        );
        assert_eq!(
            perform(Action::Unknown("scna".to_string()), &env()),
            USAGE_EXIT
        );
        assert_eq!(USAGE_EXIT, 2);
    }

    #[test]
    fn usage_shown_after_an_unknown_verb_still_reports_failure() {
        assert_eq!(perform(Action::Unknown("nope".to_string()), &env()), 2);
    }

    #[test]
    fn a_scan_that_cannot_run_exits_two() {
        assert_eq!(
            perform(Action::Scan(args(&["--nope"])), &env()),
            USAGE_EXIT
        );
    }
    /// A whole project on disk: a config plus a corpus file. Under
    /// `target/` so `cargo clean` removes it, named after its test so two
    /// never collide.
    fn project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("test-project").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "- never commit to `main`\n\n- when writing tests, prefer tables\n",
        );
        dir
    }

    fn run_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags =
            vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        flags.extend(extra.iter().map(|item| (*item).to_string()));
        scan_command(&flags, &env())
    }

    #[test]
    fn a_configured_project_scans_end_to_end() {
        let dir = project("end-to-end");
        let output = run_in(&dir, &[]).ok();
        assert!(
            output.as_ref().is_some_and(|out| out.text.contains("M1")),
            "output was {output:?}"
        );
        assert_eq!(output.map(|out| out.warnings.len()), Some(0));
    }

    /// Paths in the report are relative to the project, not to wherever the
    /// process happened to start. This is what keeps ids findable later.
    #[test]
    fn output_paths_are_project_relative() {
        let dir = project("relative");
        let text = text_of(&dir, &[]);
        assert!(text.contains("CLAUDE.md:1-1"), "text was {text}");
        assert!(
            !text.contains("target/test-project"),
            "the absolute temp path leaked into the report"
        );
    }

    #[test]
    fn json_output_parses_and_matches_the_human_row_count() {
        let dir = project("json");
        let human_rows = text_of(&dir, &[]).lines().count();
        assert_eq!(json_rows(&dir), human_rows);
        assert!(human_rows > 0, "the fixture must produce rows");
    }

    /// The rendered text, or an empty string if the scan failed. A failed
    /// scan then shows up as a failed ASSERTION on content rather than as
    /// a panic in a helper -- which keeps every test path executable.
    fn text_of(dir: &Path, extra: &[&str]) -> String {
        run_in(dir, extra).map(|out| out.text).unwrap_or_default()
    }

    fn json_rows(dir: &Path) -> usize {
        let text = text_of(dir, &["--format", "json"]);
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .as_ref()
            .and_then(|parsed| parsed.get("rows"))
            .and_then(|rows| rows.as_array())
            .map_or(0, std::vec::Vec::len)
    }

    #[test]
    fn the_sources_flag_adds_a_source_line() {
        let dir = project("sources");
        let plain = text_of(&dir, &[]);
        let with_sources = text_of(&dir, &["--sources"]);
        assert!(with_sources.lines().count() > plain.lines().count());
        assert!(with_sources.contains("project"));
    }

    #[test]
    fn filters_reach_through_the_command() {
        let dir = project("filter");
        let text = text_of(&dir, &["--class", "S"]);
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("S2"), "text was {text}");
    }

    #[test]
    fn an_unreadable_corpus_file_becomes_a_named_warning() {
        let dir = project("warned");
        let _ = std::fs::write(dir.join("bad.md"), [0xff_u8, 0xfe]);
        let warnings = run_in(&dir, &[])
            .map(|out| out.warnings)
            .unwrap_or_default();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings.first().is_some_and(|w| w.contains("bad.md")),
            "{warnings:?}"
        );
    }

    /// A broken config is an error, not an empty scan. Treating it as "no
    /// config" would scan a corpus the user never described.
    #[test]
    fn a_broken_project_config_stops_the_scan() {
        let dir = project("broken");
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = \"not a list\"\n",
        );
        assert!(run_in(&dir, &[]).is_err());
    }
    /// The binary's SUCCESS path: a real project, printed, exit 0. This is
    /// the arm every ordinary invocation takes, and it was the last part
    /// of the command path never executed by the suite.
    #[test]
    fn a_successful_scan_exits_zero() {
        let dir = project("exit-zero");
        let flags = vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        assert_eq!(perform(Action::Scan(flags), &env()), 0);
    }

    /// A corpus with an unreadable file still SUCCEEDS -- the warning is
    /// printed, the readable statements are reported, and the exit code
    /// stays 0. A partial corpus is not a failed run, but it must not be
    /// a silent one either.
    #[test]
    fn a_scan_with_warnings_still_exits_zero() {
        let dir = project("exit-zero-warned");
        let _ = std::fs::write(dir.join("bad.md"), [0xff_u8, 0xfe]);
        let flags = vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        assert_eq!(perform(Action::Scan(flags), &env()), 0);
    }
    #[test]
    fn init_is_dispatched() {
        assert_eq!(
            decide(&args(&["init", "--force"])),
            Action::Init(args(&["--force"]))
        );
    }

    #[test]
    fn init_flags_parse() {
        assert_eq!(
            parse_init(&args(&["--force", "--format", "json"]))
                .map(|parsed| (parsed.force, parsed.json))
                .ok(),
            Some((true, true))
        );
    }

    #[test]
    fn an_unknown_init_flag_is_an_error() {
        assert!(parse_init(&args(&["--nope"])).is_err());
        assert!(parse_init(&args(&["-C"])).is_err());
    }

    #[test]
    fn init_writes_a_config_and_names_it() {
        let dir = PathBuf::from("target").join("cli-init").join("fresh");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("CLAUDE.md"), "- a rule\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let text = init_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("root   CLAUDE.md"), "text was {text}");
    }

    /// init then scan, with nothing hand-written in between. That is the
    /// cold start, and it is the whole reason this verb exists.
    #[test]
    fn init_then_scan_works_with_no_hand_written_config() {
        let dir = PathBuf::from("target").join("cli-init").join("cold-start");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ =
            std::fs::write(dir.join("CLAUDE.md"), "- never commit to `main`\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert!(init_command(&flags, &env()).is_ok());
        let scanned = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(scanned.contains("M1"), "scan output was {scanned}");
    }

    #[test]
    fn init_refusing_to_clobber_exits_two() {
        let dir = PathBuf::from("target").join("cli-init").join("clobber");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("rekall.toml"), "[sources]\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert_eq!(perform(Action::Init(flags), &env()), USAGE_EXIT);
    }

    #[test]
    fn a_successful_init_exits_zero() {
        let dir = PathBuf::from("target").join("cli-init").join("exit-zero");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert_eq!(perform(Action::Init(flags), &env()), 0);
    }

    #[test]
    fn init_json_is_parseable() {
        let dir = PathBuf::from("target").join("cli-init").join("json");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let flags = args(&["-C", &dir.to_string_lossy(), "--format", "json"]);
        let text = init_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "text was {text}"
        );
    }
    fn show_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-show").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "- never commit to `main`\n\n- when writing tests, prefer tables\n",
        );
        dir
    }

    #[test]
    fn show_is_dispatched_with_its_id() {
        assert_eq!(
            decide(&args(&["show", "abc"])),
            Action::Show(args(&["abc"]))
        );
    }

    #[test]
    fn show_parses_an_id_and_flags_in_any_order() {
        assert_eq!(
            parse_show(&args(&["--format", "json", "abc"]))
                .map(|parsed| (parsed.id, parsed.json))
                .ok(),
            Some((Some("abc".to_string()), true))
        );
    }

    /// A second positional is a MISTAKE. `show a b` most likely means the
    /// shell split something, and quietly showing only `a` would answer a
    /// question nobody asked.
    #[test]
    fn a_second_id_is_an_error_not_a_second_lookup() {
        assert!(parse_show(&args(&["abc", "def"])).is_err());
    }

    #[test]
    fn show_without_an_id_says_where_to_get_one() {
        let message =
            show_command(&args(&[]), &env()).err().unwrap_or_default();
        assert!(message.contains("rekall scan"), "message was {message}");
    }

    #[test]
    fn show_prints_the_verbatim_statement() {
        let dir = show_project("verbatim");
        let text = show_first(&dir);
        assert!(text.contains("never commit"), "text was {text}");
        assert!(text.contains("class    M1"), "text was {text}");
    }

    /// Scans, takes the first id from the output, and shows it -- the
    /// copy-an-id-from-scan path a user actually takes.
    fn show_first(dir: &Path) -> String {
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.push(id.to_string());
        show_command(&show_flags, &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }

    #[test]
    fn an_ambiguous_prefix_names_the_candidates() {
        let dir = show_project("ambiguous");
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(String::new());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(
            message.contains("matches 2 statements"),
            "message was {message}"
        );
        assert!(
            message.contains("Use more characters"),
            "message was {message}"
        );
    }

    #[test]
    fn an_unknown_id_says_so() {
        let dir = show_project("unknown");
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push("zzzzzzz".to_string());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    #[test]
    fn a_failed_show_exits_two() {
        assert_eq!(perform(Action::Show(args(&["zzz"])), &env()), USAGE_EXIT);
    }
    #[test]
    fn an_unknown_show_flag_is_an_error() {
        assert!(parse_show(&args(&["--nope"])).is_err());
        assert!(parse_show(&args(&["--format"])).is_err());
    }

    #[test]
    fn show_json_is_parseable_and_carries_the_text() {
        let dir = show_project("json");
        let text = show_first_as(&dir, &["--format", "json"]);
        let parsed = serde_json::from_str::<serde_json::Value>(&text).ok();
        assert!(parsed.is_some(), "text was {text}");
        assert!(text.contains("\"text\""), "text was {text}");
    }

    fn show_first_as(dir: &Path, extra: &[&str]) -> String {
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.extend(args(extra));
        show_flags.push(id.to_string());
        show_command(&show_flags, &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }

    #[test]
    fn a_successful_show_exits_zero() {
        let dir = show_project("exit-zero");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.push(id.to_string());
        assert_eq!(perform(Action::Show(show_flags), &env()), 0);
    }

    #[test]
    fn show_without_configured_roots_says_what_to_run() {
        let mut flags = args(&["-C", "/tmp"]);
        flags.push("abc".to_string());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(message.contains("rekall init"), "message was {message}");
    }
    fn plan_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-plan").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "- never commit to `main`\n\n- always prefer the simpler option\n",
        );
        dir
    }

    fn plan_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        plan_command(&flags, &env())
    }

    fn first_scan_id(dir: &Path) -> String {
        scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
            .map(|out| out.text)
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn plan_is_dispatched() {
        assert_eq!(
            decide(&args(&["plan", "abc"])),
            Action::Plan(args(&["abc"]))
        );
    }

    #[test]
    fn plan_without_ids_says_where_to_get_them() {
        let message = plan_in(Path::new("."), &[]).err().unwrap_or_default();
        assert!(message.contains("rekall scan"), "message was {message}");
    }

    #[test]
    fn plan_names_the_delete_the_write_and_the_wiring() {
        let dir = plan_project("human");
        let id = first_scan_id(&dir);
        let text = plan_in(&dir, &[&id])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("delete  CLAUDE.md:1-1"), "{text}");
        assert!(text.contains("write   .rekall/rules/"), "{text}");
        assert!(text.contains("wire "), "{text}");
    }

    /// Every id resolves BEFORE any step is built, so a typo in the second
    /// id does not hand back a partial plan for the first.
    #[test]
    fn one_bad_id_produces_no_plan_at_all() {
        let dir = plan_project("partial");
        let id = first_scan_id(&dir);
        let outcome = plan_in(&dir, &[&id, "zzzzzzz"]);
        assert!(outcome.is_err());
    }

    #[test]
    fn planning_an_unclassified_statement_is_refused() {
        let dir = plan_project("unclassified");
        let listed =
            scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
                .map(|out| out.text)
                .unwrap_or_default();
        let last = listed.lines().last().unwrap_or_default();
        let id = last.split_whitespace().next().unwrap_or_default();
        let message = plan_in(&dir, &[id]).err().unwrap_or_default();
        assert!(message.contains("unclassified"), "message was {message}");
    }

    #[test]
    fn an_unknown_plan_flag_is_an_error() {
        assert!(parse_plan(&args(&["--nope"])).is_err());
        assert!(parse_plan(&args(&["--out"])).is_err());
    }

    /// `--out` makes the plan an ARTIFACT: reviewable, diffable, and
    /// committable before a byte of corpus moves.
    #[test]
    fn out_writes_a_plan_file_that_parses_back() {
        let dir = plan_project("out");
        let id = first_scan_id(&dir);
        let result = plan_in(&dir, &[&id, "--out", "nested/x.plan"]);
        assert!(result.is_ok(), "{result:?}");
        let text = std::fs::read_to_string(dir.join("nested").join("x.plan"))
            .unwrap_or_default();
        assert!(
            toml::from_str::<plan::Plan>(&text).is_ok(),
            "plan was {text}"
        );
    }

    #[test]
    fn writing_a_plan_is_reported_as_a_warning_not_silently() {
        let dir = plan_project("reported");
        let id = first_scan_id(&dir);
        let warnings = plan_in(&dir, &[&id, "--out", "x.plan"])
            .map(|result| result.warnings)
            .unwrap_or_default();
        assert!(
            warnings.first().is_some_and(|w| w.contains("wrote plan")),
            "{warnings:?}"
        );
    }

    #[test]
    fn plan_json_is_parseable() {
        let dir = plan_project("json");
        let id = first_scan_id(&dir);
        let text = plan_in(&dir, &[&id, "--format", "json"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }

    #[test]
    fn a_successful_plan_exits_zero() {
        let dir = plan_project("exit-zero");
        let id = first_scan_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        assert_eq!(perform(Action::Plan(flags), &env()), 0);
    }

    #[test]
    fn a_failed_plan_exits_two() {
        assert_eq!(
            perform(Action::Plan(args(&["zzzzzzz"])), &env()),
            USAGE_EXIT
        );
    }

    #[test]
    fn a_plan_that_cannot_be_written_is_an_error() {
        let dir = plan_project("unwritable");
        let id = first_scan_id(&dir);
        let outcome =
            plan_in(&dir, &[&id, "--out", "/definitely/not/writable/x.plan"]);
        assert!(outcome.is_err());
    }
    #[test]
    fn an_ambiguous_plan_id_names_the_candidates() {
        let dir = plan_project("ambiguous");
        let message = plan_in(&dir, &[""]).err().unwrap_or_default();
        assert!(
            message.contains("matches 2 statements"),
            "message was {message}"
        );
    }

    /// The warning that a plan file was written goes to STDERR and the
    /// plan to stdout, so `rekall plan --out p | less` still shows the plan
    /// and the write is still announced.
    #[test]
    fn writing_a_plan_still_exits_zero() {
        let dir = plan_project("out-exit");
        let id = first_scan_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.extend(args(&["--out", "x.plan"]));
        assert_eq!(perform(Action::Plan(flags), &env()), 0);
    }
    /// `--out` is anchored to `-C` like every other path. A relative
    /// `--out` resolved against the PROCESS directory would drop the plan
    /// somewhere the project it describes cannot see.
    #[test]
    fn a_relative_out_lands_inside_the_project() {
        let dir = plan_project("relative-out");
        let id = first_scan_id(&dir);
        assert!(plan_in(&dir, &[&id, "--out", "x.plan"]).is_ok());
        assert!(
            dir.join("x.plan").is_file(),
            "plan did not land in the project"
        );
    }
    fn apply_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-apply").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        // A GENEROUS runner bound, and B5 is exactly why. This suite runs
        // ~490 tests in parallel, many spawning `itok`, which is enough
        // contention to blow a 2s WALL-CLOCK limit on a script whose body
        // is `echo`. That is not a test artifact -- it is the production
        // failure in miniature, so the tests answer it the way a user on a
        // loaded box would: by setting the knob.
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n\n[triggers]\nrunner_timeout_ms = 30000\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n\n- background prose\n",
        );
        dir
    }

    fn apply_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        apply_command(&flags, &env())
    }

    fn rule_id(dir: &Path) -> String {
        scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
            .map(|out| out.text)
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn apply_is_dispatched() {
        assert_eq!(
            decide(&args(&["apply", "abc"])),
            Action::Apply(args(&["abc"]))
        );
    }

    /// V20: off a tty, silence is not consent.
    #[test]
    fn without_a_terminal_and_without_the_flag_apply_refuses() {
        assert_eq!(
            approved(&Consent::Unattended),
            Err(NEEDS_APPROVAL.to_string())
        );
        assert!(NEEDS_APPROVAL.contains("--auto-approve"));
    }

    #[test]
    fn the_flag_is_consent_and_a_no_is_not() {
        assert!(approved(&Consent::Flag).is_ok());
        assert!(approved(&Consent::Answered("y\n".to_string())).is_ok());
        assert!(approved(&Consent::Answered("yes".to_string())).is_ok());
        assert!(approved(&Consent::Answered("n".to_string())).is_err());
        assert!(approved(&Consent::Answered(String::new())).is_err());
    }

    /// Bare Enter means NO. The prompt reads `[y/N]`, and a destructive
    /// default that triggers on a stray keypress is not a confirmation.
    #[test]
    fn an_empty_answer_cancels() {
        assert_eq!(
            approved(&Consent::Answered("\n".to_string())),
            Err("cancelled".to_string())
        );
    }

    #[test]
    fn apply_without_ids_or_a_plan_says_what_to_run() {
        let dir = apply_project("no-input");
        let message = apply_in(&dir, &["--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("rekall plan"), "message was {message}");
    }

    /// V1: the span is DELETED and a pointer left. Both halves asserted --
    /// a copy would leave the context cost unpaid.
    #[test]
    fn apply_moves_the_statement_and_leaves_a_pointer() {
        let dir = apply_project("moves");
        let id = rule_id(&dir);
        assert!(apply_in(&dir, &[&id, "--auto-approve"]).is_ok());
        let corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert!(
            !corpus.contains("never commit"),
            "source survived: {corpus}"
        );
        assert!(corpus.contains("<!-- rekall"), "no pointer: {corpus}");
        assert!(
            corpus.contains("- background prose"),
            "unrelated prose lost: {corpus}"
        );
    }

    #[test]
    fn apply_writes_the_artifact_and_records_the_ledger() {
        let dir = apply_project("artifact");
        let id = rule_id(&dir);
        assert!(apply_in(&dir, &[&id, "--auto-approve"]).is_ok());
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        let artifact = held
            .find(&id)
            .map(|row| row.artifact.clone())
            .unwrap_or_default();
        assert!(
            dir.join(&artifact).is_file(),
            "artifact missing: {artifact}"
        );
        assert_eq!(held.extracted.len(), 1);
    }

    /// V9: the ledger keeps the ORIGINAL TEXT, which is what makes `revert`
    /// a replay rather than a rewrite.
    #[test]
    fn the_ledger_keeps_the_original_text_verbatim() {
        let dir = apply_project("verbatim");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(
            held.find(&id).map(|row| row.text.clone()),
            Some("- never commit to `main`".to_string())
        );
    }

    /// V13: applying an id that is already extracted is a NO-OP at exit 0.
    /// Its statement is gone from the corpus -- `apply` replaced it with a
    /// pointer -- so the lookup that would fail must consult the ledger.
    #[test]
    fn re_applying_an_extracted_id_is_a_no_op() {
        let dir = apply_project("idempotent");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let before =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let second = apply_in(&dir, &[&id, "--auto-approve"]);
        assert!(second.is_ok(), "{second:?}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(before, after, "a second apply changed the corpus");
    }

    #[test]
    fn re_applying_exits_zero_and_says_it_skipped() {
        let dir = apply_project("idempotent-exit");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let text = apply_in(&dir, &[&id, "--auto-approve"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("already extracted"), "{text}");
    }

    #[test]
    fn an_id_in_neither_the_corpus_nor_the_ledger_is_still_an_error() {
        let dir = apply_project("unknown");
        let message = apply_in(&dir, &["zzzzzzz", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    /// V19: the whole reason the fingerprint exists.
    #[test]
    fn a_plan_applied_after_the_corpus_moved_is_refused() {
        let dir = apply_project("stale");
        let saved = save_plan(&dir);
        shift_corpus(&dir);
        let message = apply_in(&dir, &[&saved, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("refusing a stale plan"),
            "message was {message}"
        );
    }

    fn save_plan(dir: &Path) -> String {
        let id = rule_id(dir);
        let _ = plan_command(
            &args(&["-C", &dir.to_string_lossy(), &id, "--out", "p.plan"]),
            &env(),
        );
        dir.join("p.plan").to_string_lossy().to_string()
    }

    /// Insert lines ABOVE every span, which is the edit that moves them.
    fn shift_corpus(dir: &Path) {
        let path = dir.join("CLAUDE.md");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::write(&path, format!("# shifted\n\n{text}"));
    }

    #[test]
    fn a_fresh_plan_file_applies() {
        let dir = apply_project("from-plan");
        let id = rule_id(&dir);
        let _ = plan_command(
            &args(&["-C", &dir.to_string_lossy(), &id, "--out", "p.plan"]),
            &env(),
        );
        let plan_path = dir.join("p.plan");
        let result =
            apply_in(&dir, &[&plan_path.to_string_lossy(), "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn an_unknown_apply_flag_is_an_error() {
        assert!(parse_apply(&args(&["--nope"])).is_err());
        assert!(parse_apply(&args(&["--format"])).is_err());
    }

    #[test]
    fn a_successful_apply_exits_zero() {
        let dir = apply_project("exit-zero");
        let id = rule_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.push("--auto-approve".to_string());
        assert_eq!(perform(Action::Apply(flags), &env()), 0);
    }

    #[test]
    fn a_failed_apply_exits_two() {
        assert_eq!(
            perform(
                Action::Apply(args(&["zzzzzzz", "--auto-approve"])),
                &env()
            ),
            USAGE_EXIT
        );
    }

    #[test]
    fn apply_json_is_parseable() {
        let dir = apply_project("json");
        let id = rule_id(&dir);
        let text = apply_in(&dir, &[&id, "--auto-approve", "--format", "json"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }
    /// Without the flag and without a terminal, apply REFUSES rather than
    /// proceeding. Tests are never a tty, so this is the real path CI takes.
    #[test]
    fn apply_without_approval_refuses_and_changes_nothing() {
        let dir = apply_project("unattended");
        let id = rule_id(&dir);
        let before =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let message = apply_in(&dir, &[&id]).err().unwrap_or_default();
        assert!(message.contains("--auto-approve"), "message was {message}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(before, after, "a refused apply edited the corpus");
    }

    /// A plan whose FINGERPRINT still matches but whose span does not fit
    /// -- a hand-edited or corrupt plan. The fingerprint check passes, so
    /// the splice is the last thing standing between that plan and the
    /// wrong bytes. It refuses too.
    #[test]
    fn a_plan_with_an_impossible_span_is_refused_at_the_splice() {
        let dir = apply_project("bad-span");
        let path = forge_plan(&dir, 999);
        let message = apply_in(&dir, &[&path, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("no longer fits"), "message was {message}");
    }

    /// A plan whose fingerprint is CORRECT but whose span is not. Hand
    /// editing a plan, or a corrupt one, gets past V19 -- so the splice is
    /// the last thing standing between it and the wrong bytes.
    fn forge_plan(dir: &Path, line: usize) -> String {
        let corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let forged = format!(
            "format = 1\n\n[[fingerprint]]\nsrc = \"CLAUDE.md\"\ndigest = \"{}\"\n\n\
             [[steps]]\nid = \"forged1\"\nsrc = \"CLAUDE.md\"\nline_start = {line}\n\
             line_end = {line}\ntext = \"x\"\nlabel = \"M1\"\n\
             artifact = \".rekall/rules/x.sh\"\nwiring = \"w\"\n",
            plan::digest_of(&corpus)
        );
        let path = dir.join("forged.plan");
        let _ = std::fs::write(&path, forged);
        path.to_string_lossy().to_string()
    }

    #[test]
    fn apply_reports_what_it_did_on_stderr_in_json_mode() {
        let dir = apply_project("json-warnings");
        let id = rule_id(&dir);
        let warnings =
            apply_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.warnings)
                .unwrap_or_default();
        assert!(
            warnings.iter().any(|w| w.contains("wrote ")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("edited ")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_json_apply_exits_zero_and_prints_its_warnings() {
        let dir = apply_project("json-exit");
        let id = rule_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.extend(args(&["--auto-approve", "--format", "json"]));
        assert_eq!(perform(Action::Apply(flags), &env()), 0);
    }
    #[test]
    fn an_answer_is_read_from_any_reader() {
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        assert_eq!(
            read_answer(&mut input).ok(),
            Some(Consent::Answered("y\n".to_string()))
        );
    }

    #[test]
    fn an_answer_that_is_read_is_then_judged() {
        let mut yes = std::io::Cursor::new(b"yes\n".to_vec());
        let mut no = std::io::Cursor::new(b"\n".to_vec());
        assert!(read_answer(&mut yes).and_then(|c| approved(&c)).is_ok());
        assert!(read_answer(&mut no).and_then(|c| approved(&c)).is_err());
    }

    fn revert_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-revert").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n\n- background prose\n",
        );
        dir
    }

    fn revert_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        revert_command(&flags, &env())
    }

    /// Extract one statement and hand back its id and the corpus as it
    /// stood BEFORE the extraction -- which is the thing a revert has to
    /// reproduce.
    fn extracted(dir: &Path) -> (String, String) {
        extracted_from(dir, &dir.join("CLAUDE.md"))
    }

    /// The artifact an extracted id landed, read back from the ledger --
    /// the tests never hard-code the path, because the slug is derived
    /// from the statement's own words.
    fn artifact_of(dir: &Path, id: &str) -> String {
        ledger::load(&ledger::path_in(dir))
            .unwrap_or_default()
            .find(id)
            .map(|row| row.artifact.clone())
            .unwrap_or_default()
    }

    #[test]
    fn revert_is_dispatched() {
        assert_eq!(
            decide(&args(&["revert", "abc"])),
            Action::Revert(args(&["abc"]))
        );
    }

    /// V9: THE PROMISE. Extract, revert, and the file is the file that was
    /// there -- byte for byte. The ledger's verbatim text is what makes it
    /// a replay rather than a rewrite.
    #[test]
    fn revert_restores_the_original_bytes() {
        let dir = revert_project("bytes");
        let (id, before) = extracted(&dir);
        let result = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before);
    }

    /// V1, in the other direction. Restoring the source and leaving the
    /// artifact behind would leave two hand-maintained statements of one
    /// rule -- the copy V1 exists to forbid.
    #[test]
    fn revert_removes_the_artifact_and_the_ledger_row() {
        let dir = revert_project("artifact");
        let (id, _) = extracted(&dir);
        let artifact = artifact_of(&dir, &id);
        assert!(dir.join(&artifact).is_file(), "nothing was extracted");
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(
            !dir.join(&artifact).exists(),
            "the artifact survived: {artifact}"
        );
        let after = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert!(after.extracted.is_empty(), "{after:?}");
    }

    /// V20: off a tty and without the flag, revert refuses -- and the
    /// corpus is untouched. The confirm gate is one rule, not one per verb.
    #[test]
    fn revert_without_approval_refuses_and_changes_nothing() {
        let dir = revert_project("unattended");
        let (id, _) = extracted(&dir);
        let extracted_corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let message = revert_in(&dir, &[&id]).err().unwrap_or_default();
        assert!(message.contains("--auto-approve"), "message was {message}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(
            after, extracted_corpus,
            "a refused revert edited the corpus"
        );
    }

    /// V13: reverting twice is a no-op at exit 0. The statement is back in
    /// the corpus and its text rehashes to the same id, so "not in the
    /// ledger" means work already undone rather than an unknown id.
    #[test]
    fn reverting_twice_is_a_no_op() {
        let dir = revert_project("idempotent");
        let (id, before) = extracted(&dir);
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        let second = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(second.is_ok(), "{second:?}");
        let text = second.map(|out| out.text).unwrap_or_default();
        assert!(text.contains("nothing to revert"), "{text}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before, "a second revert changed the corpus");
    }

    #[test]
    fn an_id_in_neither_the_ledger_nor_the_corpus_is_an_error() {
        let dir = revert_project("unknown");
        let message = revert_in(&dir, &["zzzzzzz", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    /// A ledger holding two rows that share a prefix, written by hand
    /// because two REAL extractions get two hex ids that are not
    /// guaranteed to collide on any character.
    fn write_twin_rows(dir: &Path) {
        let _ = std::fs::create_dir_all(dir.join(ledger::DIR));
        let _ = std::fs::write(
            ledger::path_in(dir),
            "[[extracted]]\nid = \"aaa1111\"\nsrc = \"CLAUDE.md\"\n\
             line_start = 1\nline_end = 1\ntext = \"x\"\nartifact = \"a.sh\"\n\n\
             [[extracted]]\nid = \"aaa2222\"\nsrc = \"CLAUDE.md\"\n\
             line_start = 2\nline_end = 2\ntext = \"y\"\nartifact = \"b.sh\"\n",
        );
    }

    /// Section I: a prefix that matches two ids is a usage error, not a
    /// coin toss -- and it says WHICH two, so the fix is to type more
    /// characters rather than to guess.
    #[test]
    fn an_ambiguous_prefix_names_the_ids_it_matched() {
        let dir = revert_project("ambiguous");
        write_twin_rows(&dir);
        let message = revert_in(&dir, &["aaa", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("Use more characters"), "{message}");
        assert!(message.contains("aaa1111"), "{message}");
    }

    /// The pointer is the ADDRESS. If someone deleted it by hand there is
    /// no single place the text belongs, so revert refuses -- and names
    /// what to do instead rather than only what went wrong (V28).
    #[test]
    fn a_hand_edited_pointer_refuses_and_names_the_fix() {
        let dir = revert_project("no-pointer");
        let (id, _) = extracted(&dir);
        let _ = std::fs::write(dir.join("CLAUDE.md"), "# Rules\n\n- prose\n");
        let message = revert_in(&dir, &[&id, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no longer holds the pointer"),
            "message was {message}"
        );
        assert!(message.contains("rekall log"), "no fix named: {message}");
    }

    /// An artifact someone already deleted is REPORTED, not an error: the
    /// corpus still ends in the state the revert promised.
    #[test]
    fn an_artifact_that_is_already_gone_is_reported_not_fatal() {
        let dir = revert_project("gone");
        let (id, before) = extracted(&dir);
        let _ = std::fs::remove_file(dir.join(artifact_of(&dir, &id)));
        let text = revert_in(&dir, &[&id, "--auto-approve"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("was already gone"), "{text}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before);
    }

    /// An artifact that will NOT delete is an error, and the ledger row is
    /// kept on purpose: the extraction is not fully undone, and a ledger
    /// that said it was would hide the leftover from `check` too. A
    /// directory standing where the file should be is the portable way to
    /// make the removal fail.
    #[test]
    fn an_artifact_that_cannot_be_removed_is_an_error() {
        let dir = revert_project("stuck");
        let (id, _) = extracted(&dir);
        let artifact = dir.join(artifact_of(&dir, &id));
        let _ = std::fs::remove_file(&artifact);
        let _ = std::fs::create_dir_all(artifact.join("in-the-way"));
        let message = revert_in(&dir, &[&id, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("could not be removed"), "{message}");
        let after = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(after.extracted.len(), 1, "the row was dropped anyway");
    }

    #[test]
    fn revert_json_is_parseable() {
        let dir = revert_project("json");
        let (id, _) = extracted(&dir);
        let text =
            revert_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.text)
                .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }

    #[test]
    fn a_json_revert_reports_what_it_did_on_stderr() {
        let dir = revert_project("json-warnings");
        let (id, _) = extracted(&dir);
        let warnings =
            revert_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.warnings)
                .unwrap_or_default();
        assert!(
            warnings.iter().any(|w| w.contains("restored ")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("removed ")),
            "{warnings:?}"
        );
    }

    #[test]
    fn revert_without_an_id_says_what_to_run() {
        let dir = revert_project("no-input");
        let message = revert_in(&dir, &["--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("rekall log"), "message was {message}");
    }

    #[test]
    fn revert_takes_one_id_and_a_second_is_an_error() {
        assert!(parse_revert(&args(&["aaa", "bbb"])).is_err());
        assert!(parse_revert(&args(&["--nope"])).is_err());
        assert!(parse_revert(&args(&["--format"])).is_err());
    }

    #[test]
    fn a_successful_revert_exits_zero() {
        let dir = revert_project("exit-zero");
        let (id, _) = extracted(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.push("--auto-approve".to_string());
        assert_eq!(perform(Action::Revert(flags), &env()), 0);
    }

    #[test]
    fn a_failed_revert_exits_two() {
        assert_eq!(
            perform(
                Action::Revert(args(&["zzzzzzz", "--auto-approve"])),
                &env()
            ),
            USAGE_EXIT
        );
    }

    /// The revert of a corpus file in a NESTED directory. The artifact is
    /// resolved against the project root, and recovering that root from
    /// the source path is the step that would silently go wrong.
    #[test]
    fn a_nested_source_file_resolves_its_artifact_from_the_root() {
        let dir = nested_project("nested");
        let nested = dir.join("docs").join("AGENTS.md");
        let (id, before) = extracted_from(&dir, &nested);
        let artifact = artifact_of(&dir, &id);
        assert!(dir.join(&artifact).is_file(), "nothing was extracted");
        let result = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(
            std::fs::read_to_string(&nested).unwrap_or_default(),
            before
        );
        assert!(
            !dir.join(&artifact).exists(),
            "the artifact resolved against the wrong root: {artifact}"
        );
    }

    /// A corpus whose only source file is one directory down.
    fn nested_project(name: &str) -> PathBuf {
        let dir = revert_project(name);
        let _ = std::fs::remove_file(dir.join("CLAUDE.md"));
        let _ = std::fs::create_dir_all(dir.join("docs"));
        let _ = std::fs::write(
            dir.join("docs").join("AGENTS.md"),
            "# Rules\n\n- never commit to `main`\n\n- background prose\n",
        );
        dir
    }

    /// `extracted`, for a corpus file that is not `CLAUDE.md`.
    fn extracted_from(dir: &Path, src: &Path) -> (String, String) {
        let before = std::fs::read_to_string(src).unwrap_or_default();
        let id = rule_id(dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id.clone());
        flags.push("--auto-approve".to_string());
        let _ = apply_command(&flags, &env());
        (id, before)
    }

    /// It RAISES `runner_timeout_ms` far above the default. Tests built on
    /// this fixture assert what a rule SAID, and the bound is wall-clock
    /// (B5, V38): a loaded box turns a millisecond rule into a killed one,
    /// and the suite then reports a failure that never happened. MEASURED
    /// twice today, under a concurrent clippy run. That the bound WORKS is
    /// tested where it belongs, by a rule that really does hang.
    fn check_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-check").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n\n[triggers]\nrunner_timeout_ms = 60000\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n\n- background prose\n",
        );
        dir
    }

    fn check_in(dir: &Path, extra: &[&str]) -> Result<Checked, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        check_command(&flags, &env())
    }

    /// Replace the generated placeholder with a runner that actually
    /// checks something -- the state V2 asks for.
    fn write_real_runner(dir: &Path, id: &str) {
        let path = dir.join(artifact_of(dir, id));
        let _ = std::fs::write(
            &path,
            "#!/bin/sh\n# rekall:payload\n# - never commit to `main`\n# rekall:/payload\ngrep -q x f && exit 1\n",
        );
    }

    #[test]
    fn check_is_dispatched() {
        assert_eq!(decide(&args(&["check"])), Action::Check(Vec::new()));
    }

    /// A repo that has extracted nothing is CLEAN, and SILENT (V28). A
    /// gate that fails before the tool has been used is a gate nobody
    /// adopts, and one that chatters on success is one nobody reads.
    #[test]
    fn a_repo_with_no_extractions_is_clean_and_silent() {
        let dir = check_project("clean");
        let checked = check_in(&dir, &[]);
        assert_eq!(checked.as_ref().map(|c| c.drift).unwrap_or(9), 0);
        assert_eq!(
            checked.map(|c| c.output.text).unwrap_or_default(),
            "",
            "a clean gate said something"
        );
        assert_eq!(perform(Action::Check(dash_c(&dir)), &env()), 0);
    }

    fn dash_c(dir: &Path) -> Vec<String> {
        args(&["-C", &dir.to_string_lossy()])
    }

    /// V2: `apply` writes a runner that announces itself unimplemented, so
    /// a fresh extraction is drift until someone writes the check. That is
    /// the point -- a rule with no runner gates nothing.
    #[test]
    fn a_freshly_extracted_rule_has_no_runner_yet() {
        let dir = check_project("no-runner");
        let (_, _) = extracted(&dir);
        let checked = check_in(&dir, &[]);
        let text = checked.map(|c| c.output.text).unwrap_or_default();
        assert!(text.contains(check::NO_RUNNER), "{text}");
        assert_eq!(perform(Action::Check(dash_c(&dir)), &env()), DRIFT_EXIT);
    }

    #[test]
    fn a_rule_with_a_real_runner_passes() {
        let dir = check_project("runner");
        let (id, _) = extracted(&dir);
        write_real_runner(&dir, &id);
        assert_eq!(check_in(&dir, &[]).map(|c| c.drift).unwrap_or(9), 0);
    }

    /// V1: the statement pasted back beside the artifact it moved into.
    #[test]
    fn a_statement_put_back_by_hand_is_reported() {
        let dir = check_project("copy");
        let (id, before) = extracted(&dir);
        write_real_runner(&dir, &id);
        let pointer = apply::pointer_of(&id);
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            format!("{pointer}\n\n{before}"),
        );
        let text = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(text.contains(check::STATEMENT_RESTORED), "{text}");
    }

    #[test]
    fn a_deleted_artifact_is_reported() {
        let dir = check_project("gone");
        let (id, _) = extracted(&dir);
        let _ = std::fs::remove_file(dir.join(artifact_of(&dir, &id)));
        let text = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(text.contains(check::MISSING_ARTIFACT), "{text}");
    }

    /// An artifact sitting in a rules directory with no ledger row behind
    /// it. Nothing records where it came from, so nothing can undo it.
    #[test]
    fn an_unclaimed_artifact_is_an_orphan() {
        let dir = check_project("orphan");
        let _ = std::fs::create_dir_all(dir.join(".rekall").join("rules"));
        let _ = std::fs::write(
            dir.join(".rekall").join("rules").join("stray.sh"),
            "#!/bin/sh\nexit 0\n",
        );
        let text = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(text.contains(check::ORPHAN_ARTIFACT), "{text}");
        assert!(text.contains("stray.sh"), "{text}");
    }

    /// A `revert` leaves NOTHING for the gate to find. The two verbs agree
    /// about what a finished extraction looks like, in both directions.
    #[test]
    fn a_reverted_extraction_leaves_a_clean_gate() {
        let dir = check_project("reverted");
        let (id, _) = extracted(&dir);
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        assert_eq!(check_in(&dir, &[]).map(|c| c.drift).unwrap_or(9), 0);
    }

    #[test]
    fn check_json_is_parseable_and_carries_the_kind_as_a_field() {
        let dir = check_project("json");
        let _ = extracted(&dir);
        let text = check_in(&dir, &["--format", "json"])
            .map(|c| c.output.text)
            .unwrap_or_default();
        let parsed: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_default();
        let kind = parsed
            .get("drift")
            .and_then(|drift| drift.get(0))
            .and_then(|first| first.get("kind"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(kind, Some(check::NO_RUNNER), "{text}");
    }

    /// V17: a clean gate still emits valid JSON. An agent parsing an empty
    /// string would have to special-case success.
    #[test]
    fn a_clean_gate_still_emits_json() {
        let dir = check_project("json-clean");
        let text = check_in(&dir, &["--format", "json"])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).ok(),
            Some(serde_json::json!({"drift": []})),
            "{text}"
        );
    }

    #[test]
    fn check_takes_no_positionals_and_rejects_unknown_flags() {
        assert!(parse_check(&args(&["abc"])).is_err());
        assert!(parse_check(&args(&["--nope"])).is_err());
        assert!(parse_check(&args(&["--format"])).is_err());
    }

    /// A broken invocation exits 2, not 1. "You typed it wrong" and "your
    /// corpus is wrong" are different answers and the caller reacts to
    /// them differently.
    #[test]
    fn a_usage_error_exits_two_not_one() {
        assert_eq!(perform(Action::Check(args(&["--nope"])), &env()), 2);
    }

    #[test]
    fn a_corrupt_ledger_stops_the_gate_rather_than_passing_it() {
        let dir = check_project("corrupt");
        let _ = std::fs::create_dir_all(dir.join(ledger::DIR));
        let _ = std::fs::write(ledger::path_in(&dir), "not = = toml\n");
        assert!(check_in(&dir, &[]).is_err());
        assert_eq!(perform(Action::Check(dash_c(&dir)), &env()), USAGE_EXIT);
    }

    fn log_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        log_command(&flags, &env())
    }

    #[test]
    fn log_is_dispatched() {
        assert_eq!(
            decide(&args(&["log", "--dead"])),
            Action::Log(args(&["--dead"]))
        );
    }

    /// The whole row, from a real extraction rather than a hand-written
    /// ledger: span, class, fire count and the verbatim text.
    #[test]
    fn log_reports_the_span_the_class_and_the_original_text() {
        let dir = check_project("log-row");
        let (id, _) = extracted(&dir);
        let text = log_in(&dir, &[]).map(|o| o.text).unwrap_or_default();
        assert!(text.contains(&id), "{text}");
        assert!(text.contains("CLAUDE.md:3-3"), "{text}");
        assert!(text.contains("fires=0"), "{text}");
        assert!(text.contains("- never commit to `main`"), "{text}");
    }

    /// V11: `--dead` is the measurement that makes deletion arithmetic
    /// instead of nerve. A fresh extraction has never fired, so it is
    /// dead; one that has fired drops out.
    #[test]
    fn dead_lists_the_never_fired_and_drops_the_rest() {
        let dir = check_project("log-dead");
        let (id, _) = extracted(&dir);
        assert!(dead_text(&dir).contains(&id), "a fresh extraction is dead");
        record_a_firing(&dir, &id);
        assert_eq!(
            dead_text(&dir),
            "",
            "a fired artifact was still called dead"
        );
    }

    fn dead_text(dir: &Path) -> String {
        log_in(dir, &["--dead"]).map(|o| o.text).unwrap_or_default()
    }

    /// Count one firing straight into the ledger. `hook` will do this for
    /// real (T13); until then the counter is exercised where it lives.
    fn record_a_firing(dir: &Path, id: &str) {
        let path = ledger::path_in(dir);
        let mut held = ledger::load(&path).unwrap_or_default();
        held.fired(id);
        let _ = ledger::save(&path, &held);
    }

    #[test]
    fn an_empty_ledger_logs_nothing_and_exits_zero() {
        let dir = check_project("log-empty");
        assert_eq!(log_in(&dir, &[]).map(|o| o.text).unwrap_or_default(), "");
        assert_eq!(perform(Action::Log(dash_c(&dir)), &env()), 0);
    }

    /// A recent extraction is inside a wide window and outside a narrow
    /// one. The clock is real here, so the assertion is about which side
    /// of the floor `now` falls -- not about a fixed timestamp.
    #[test]
    fn since_bounds_the_window() {
        let dir = check_project("log-since");
        let (id, _) = extracted(&dir);
        let wide = log_in(&dir, &["--since", "2w"])
            .map(|o| o.text)
            .unwrap_or_default();
        assert!(wide.contains(&id), "{wide}");
    }

    #[test]
    fn a_since_without_a_unit_is_a_usage_error_that_names_the_forms() {
        let dir = check_project("log-bad-since");
        let said = log_in(&dir, &["--since", "7"]).err().unwrap_or_default();
        assert!(said.contains("7d"), "{said}");
        assert_eq!(
            perform(Action::Log(args(&["--since", "7"])), &env()),
            USAGE_EXIT
        );
    }

    #[test]
    fn log_json_is_parseable_and_carries_the_fire_count() {
        let dir = check_project("log-json");
        let _ = extracted(&dir);
        let text = log_in(&dir, &["--format", "json"])
            .map(|o| o.text)
            .unwrap_or_default();
        let parsed: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_default();
        let fires = parsed
            .get("entries")
            .and_then(|entries| entries.get(0))
            .and_then(|first| first.get("fires"));
        assert_eq!(fires, Some(&serde_json::json!(0)), "{text}");
    }

    #[test]
    fn an_unknown_log_flag_is_an_error() {
        assert!(parse_log(&args(&["--nope"])).is_err());
        assert!(parse_log(&args(&["--since"])).is_err());
        assert!(parse_log(&args(&["abc"])).is_err());
    }

    /// A reverted extraction leaves the ledger, so it leaves the log. The
    /// log is the record of what is extracted NOW, not a history of
    /// everything that ever was.
    #[test]
    fn a_reverted_extraction_leaves_the_log() {
        let dir = check_project("log-reverted");
        let (id, _) = extracted(&dir);
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        assert_eq!(log_in(&dir, &[]).map(|o| o.text).unwrap_or_default(), "");
    }

    /// A project with one extracted `S` skill whose blocks are filled.
    fn recall_project(name: &str, fire: &str, refuse: &str) -> PathBuf {
        let dir = check_project(name);
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- when editing Rust files, run clippy first\n",
        );
        let (id, _) = extracted(&dir);
        let path = dir.join(artifact_of(&dir, &id));
        let _ = std::fs::write(&path, skill_with(fire, refuse));
        dir
    }

    fn skill_with(fire: &str, refuse: &str) -> String {
        format!(
            "---\nname: s\ndescription: \"- never commit to `main`\"\n---\n\n<!-- rekall:payload -->\n- never commit to `main`\n\
             <!-- rekall:/payload -->\n\n{}\n\n```rekall\n{fire}\n```\n\n{}\n\n```rekall\n{refuse}\n```\n",
            apply::FIRES,
            apply::NOT_FIRES
        )
    }

    fn recall_in(dir: &Path, extra: &[&str]) -> String {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        recall_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }

    /// V22, end to end: a `[signals]` table nothing read would be a wish.
    /// This drives it from the CONFIG FILE through `scan` to a verdict --
    /// the same statement is `U` with the shipped defaults and `M` once
    /// the corpus names its own vocabulary.
    #[test]
    fn a_configured_weight_reaches_the_verdict() {
        let dir = check_project("weights");
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- release notes ship beside the tag\n",
        );
        assert!(
            scan_row(&dir).contains("  U  "),
            "the defaults should not classify this: {}",
            scan_row(&dir)
        );
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n\n[signals.weight]\n\"ship beside\" = 2\n",
        );
        assert!(scan_row(&dir).contains("  M"), "{}", scan_row(&dir));
    }

    /// The DEADBAND, from the same file. A corpus can ask for more
    /// evidence before it accepts a verdict.
    #[test]
    fn a_configured_deadband_withholds_a_weak_verdict() {
        let dir = check_project("deadband");
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- when editing `.rs`, never use unwrap\n",
        );
        assert!(scan_row(&dir).contains("  M"), "{}", scan_row(&dir));
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n\n[signals]\ndeadband = 1\n",
        );
        assert!(scan_row(&dir).contains("  U  "), "{}", scan_row(&dir));
    }

    fn scan_row(dir: &Path) -> String {
        scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }

    fn payload(dir: &Path, path: &str) -> String {
        format!(
            "{{\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"Edit\",\
             \"tool_input\":{{\"file_path\":\"{path}\"}},\"cwd\":\"{}\"}}",
            dir.to_string_lossy()
        )
    }

    fn hook_in(dir: &Path, path: &str) -> serde_json::Value {
        hook_command(&payload(dir, path), dir).unwrap_or_default()
    }

    #[test]
    fn hook_is_dispatched() {
        assert_eq!(decide(&args(&["hook"])), Action::Hook(Vec::new()));
    }

    /// V18, made literal. What `recall` PRINTS is what `hook` DECIDES,
    /// asserted by driving both from the same project and comparing --
    /// two matchers is the invisible defect, where each looks right alone
    /// and the divergence only shows in production.
    #[test]
    fn hook_decides_what_recall_prints() {
        let dir = recall_project("agree", "path = [\"**/*.rs\"]", "");
        let printed = recall_in(&dir, &["--tool", "Edit", "--path", "a.rs"]);
        let decided = hook_in(&dir, "a.rs");
        assert!(printed.starts_with("load"), "{printed}");
        assert!(decided.get("hookSpecificOutput").is_some(), "{decided}");

        let printed = recall_in(&dir, &["--tool", "Edit", "--path", "a.md"]);
        let decided = hook_in(&dir, "a.md");
        assert!(printed.starts_with("skip"), "{printed}");
        assert_eq!(decided, serde_json::json!({}), "{decided}");
    }

    /// The skill's RULE is what reaches the model. An id would tell it a
    /// rule exists without saying what the rule is; the whole file would
    /// tell it how to maintain a trigger it will never edit (V43).
    #[test]
    fn the_skill_text_is_what_gets_injected() {
        let dir = recall_project("inject", "path = [\"**/*.rs\"]", "");
        let out = hook_in(&dir, "a.rs");
        let context = out
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(context.contains("never commit"), "{context}");
    }

    /// V34: the counter, and V11 finally has an author. A skill that
    /// LOADS is a skill that fired.
    #[test]
    fn a_loaded_skill_is_counted_as_fired() {
        let dir = recall_project("counted", "path = [\"**/*.rs\"]", "");
        assert_eq!(fires_of(&dir), 0);
        let _ = hook_in(&dir, "a.rs");
        assert_eq!(fires_of(&dir), 1);
        let _ = hook_in(&dir, "a.rs");
        assert_eq!(fires_of(&dir), 2, "the second fire was lost");
    }

    /// A skill that did NOT load must not count. Otherwise `--dead`
    /// measures how often the hook ran, not how often the rule mattered.
    #[test]
    fn a_skill_that_did_not_load_is_not_counted() {
        let dir = recall_project("uncounted", "path = [\"**/*.rs\"]", "");
        let _ = hook_in(&dir, "notes.md");
        assert_eq!(fires_of(&dir), 0);
    }

    /// V34's hazard, exercised the way it would actually bite: one hook
    /// per tool call means concurrent writers. A read-modify-write of the
    /// ledger would lose counts here and only here.
    #[test]
    fn concurrent_hooks_do_not_lose_counts() {
        let dir = recall_project("concurrent", "path = [\"**/*.rs\"]", "");
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let at = dir.clone();
                scope.spawn(move || hook_in(&at, "a.rs"));
            }
        });
        assert_eq!(fires_of(&dir), 8, "a concurrent fire was lost");
    }

    /// V11 reads the folded number: `--dead` must stop calling an
    /// artifact dead once a hook has fired it.
    #[test]
    fn a_fired_skill_leaves_the_dead_list() {
        let dir = recall_project("dead", "path = [\"**/*.rs\"]", "");
        assert!(!dead_text(&dir).is_empty(), "should start dead");
        let _ = hook_in(&dir, "a.rs");
        assert_eq!(dead_text(&dir), "", "still dead after firing");
    }

    fn fires_of(dir: &Path) -> u64 {
        ledger::load(&ledger::path_in(dir))
            .unwrap_or_default()
            .extracted
            .first()
            .map_or(0, |row| row.fires)
    }

    /// Section I: the signal is in the JSON, NEVER the exit code. A
    /// harness reading a nonzero exit would call the tool broken on every
    /// tool use.
    #[test]
    fn a_broken_payload_still_exits_zero_with_valid_json() {
        let dir = check_project("hook-garbage");
        let out = hook_command("not json", &dir).unwrap_or_default();
        assert_eq!(out, serde_json::json!({}));
        assert_eq!(perform(Action::Hook(Vec::new()), &env()), 0);
    }

    #[test]
    fn hook_takes_no_flags() {
        assert_eq!(
            perform(Action::Hook(args(&["--format", "json"])), &env()),
            USAGE_EXIT
        );
    }

    /// A project with one extracted `M` rule whose runner is real and
    /// whose trigger fires on `*.rs`.
    ///
    fn rule_project(name: &str, body: &str) -> PathBuf {
        let dir = check_project(name);
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n",
        );
        let (id, _) = extracted(&dir);
        let path = dir.join(artifact_of(&dir, &id));
        let _ = std::fs::write(&path, body);
        let _ = apply_cli::make_runnable(&path);
        write_trigger(&dir, &id, "path = [\"**/*.rs\"]");
        dir
    }

    /// An `M` artifact carries its runner AND its trigger block: the
    /// script is the rule, the block is when it arrives early (V37).
    fn write_trigger(dir: &Path, id: &str, fire: &str) {
        let path = dir.join(artifact_of(dir, id));
        let held = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::write(
            &path,
            format!(
                "{held}\n# {}\n#\n# ```rekall\n# {fire}\n# ```\n#\n# {}\n#\n# ```rekall\n# ```\n",
                apply::FIRES,
                apply::NOT_FIRES
            ),
        );
    }

    /// V37 end to end: a rule with a trigger FIRES from the hook, and its
    /// own words reach the model.
    #[test]
    fn a_mechanical_rule_with_a_trigger_advises() {
        let dir = rule_project(
            "m-fires",
            "#!/bin/sh\necho 'do not commit to main' >&2\nexit 1\n",
        );
        let out = hook_in(&dir, "a.rs");
        assert_eq!(
            out.pointer("/hookSpecificOutput/additionalContext")
                .and_then(serde_json::Value::as_str),
            Some("do not commit to main"),
            "{out}"
        );
    }

    /// V38, THE ONE THAT MATTERS. A failing rule ADVISES; it must never
    /// deny the tool call. A wrong rule that blocks costs the user their
    /// work, and unwedging it means editing the corpus mid-task.
    #[test]
    fn a_failing_rule_never_denies_the_tool_call() {
        let dir = rule_project("m-advises", "#!/bin/sh\necho no >&2\nexit 1\n");
        let out = hook_in(&dir, "a.rs");
        assert!(
            out.pointer("/hookSpecificOutput/permissionDecision")
                .is_none(),
            "the hook tried to block a tool call: {out}"
        );
        assert_eq!(perform(Action::Hook(Vec::new()), &env()), 0);
    }

    /// A rule that looked and found nothing says NOTHING. Injecting
    /// "passed" on every tool call is the always-on cost this crate
    /// exists to remove.
    #[test]
    fn a_clean_rule_adds_no_context() {
        let dir = rule_project("m-clean", "#!/bin/sh\nexit 0\n");
        assert_eq!(hook_in(&dir, "a.rs"), serde_json::json!({}));
    }

    /// V38: bounded. A hung rule is killed and reported rather than
    /// stalling the harness on every call.
    #[test]
    fn a_hanging_rule_does_not_stall_the_hook() {
        // This one wants the bound to BITE, so it sets a SHORT one --
        // through the same config key, which is the point.
        let dir = rule_project("m-hang", "#!/bin/sh\nsleep 30\n");
        set_runner_timeout(&dir, 150);
        let started = std::time::Instant::now();
        let out = hook_in(&dir, "a.rs");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the hook was not bounded"
        );
        let said = out
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(said.contains("did not finish"), "{out}");
    }

    /// V2: a generated runner arrives EXECUTABLE. Otherwise the failure
    /// surfaces as "permission denied" at a tool call rather than as
    /// something `check` could have told you.
    #[test]
    fn a_generated_runner_is_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = check_project("m-mode");
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n",
        );
        let (id, _) = extracted(&dir);
        let mode = std::fs::metadata(dir.join(artifact_of(&dir, &id)))
            .map(|meta| meta.permissions().mode() & 0o111);
        assert_eq!(mode.ok(), Some(0o111), "the runner is not executable");
    }

    /// B4, at the level it was found: `check` and `recall` must agree
    /// about the same artifact. A generated `M` rule has no trigger block,
    /// `check` is silent about that (V37 makes it gate-only), and `recall`
    /// used to call it broken.
    #[test]
    fn check_and_recall_agree_about_a_generated_rule() {
        let dir = one_extracted_rule("agree-m");
        let drift = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(
            !drift.contains(check::BAD_TRIGGER_BLOCK)
                && !drift.contains(check::NO_TRIGGER),
            "check called the trigger broken: {drift}"
        );
        let said = recall_in(&dir, &["--tool", "Edit", "--path", "a.rs"]);
        assert!(said.contains("gates at commit only"), "{said}");
        assert!(!said.contains("could not be read"), "{said}");
    }

    /// THE PAYOFF of T45. The generated artifact now TELLS you the block
    /// exists, so filling it in is a local edit rather than a spec read.
    /// This walks that path: take the emitted block, put a real trigger in
    /// it, and the rule arrives at the tool call.
    #[test]
    fn filling_the_emitted_block_makes_the_rule_fire() {
        let dir = one_extracted_rule("emitted-block");
        assert_eq!(hook_in(&dir, "a.rs"), serde_json::json!({}));
        write_a_real_rule(&dir);
        let out = hook_in(&dir, "a.rs");
        assert_eq!(
            out.pointer("/hookSpecificOutput/additionalContext")
                .and_then(serde_json::Value::as_str),
            Some("main is protected"),
            "{out}"
        );
    }

    /// What a user actually does once the artifact tells them the block is
    /// there: name the trigger, and REPLACE the placeholder echo with a
    /// real check rather than adding alongside it.
    fn write_a_real_rule(dir: &Path) {
        let path = dir.join(artifact_of(dir, &extracted_id(dir)));
        let held = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::write(&path, real_rule(&held));
    }

    fn extracted_id(dir: &Path) -> String {
        ledger::load(&ledger::path_in(dir))
            .unwrap_or_default()
            .extracted
            .first()
            .map(|row| row.id.clone())
            .unwrap_or_default()
    }

    fn real_rule(held: &str) -> String {
        held.replacen("# tool = []", "# tool = [\"Edit\"]", 1)
            .lines()
            .map(|line| {
                if line.contains(apply::UNIMPLEMENTED) {
                    "echo 'main is protected' >&2"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `check` says NOTHING about an `M`s triggers, before or after the
    /// blocks are emitted. It wants the runner (V2) and nothing else.
    #[test]
    fn emitting_the_blocks_adds_no_drift() {
        let dir = one_extracted_rule("no-new-drift");
        let text = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(text.contains(check::NO_RUNNER), "{text}");
        assert!(!text.contains(check::BAD_TRIGGER_BLOCK), "{text}");
        assert!(!text.contains(check::NO_REFUSAL_CLAUSE), "{text}");
    }

    fn set_runner_timeout(dir: &Path, ms: u64) {
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            format!(
                "[sources]\nroots = [\".\"]\n\n[triggers]\nrunner_timeout_ms = {ms}\n"
            ),
        );
    }

    /// A project with one extracted `M` rule, left exactly as `apply`
    /// wrote it -- no trigger block, which is the state B4 misread.
    fn one_extracted_rule(name: &str) -> PathBuf {
        let dir = check_project(name);
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n",
        );
        let _ = extracted(&dir);
        dir
    }

    /// And a GATE-ONLY rule must not start firing as a side effect of
    /// being called legal. `loads` stays false, so the hook says nothing.
    #[test]
    fn a_gate_only_rule_still_does_not_fire() {
        let dir = one_extracted_rule("gate-only-quiet");
        assert_eq!(hook_in(&dir, "a.rs"), serde_json::json!({}));
    }

    #[test]
    fn recall_is_dispatched() {
        assert_eq!(
            decide(&args(&["recall", "editing"])),
            Action::Recall(args(&["editing"]))
        );
    }

    /// V3's reload rule, answered end to end: an extracted skill with a
    /// matching trigger LOADS here.
    #[test]
    fn a_matching_skill_loads() {
        let dir = recall_project("hit", "path = [\"**/*.rs\"]", "");
        let out = recall_in(&dir, &["--path", "src/main.rs"]);
        assert!(out.starts_with("load"), "{out}");
    }

    #[test]
    fn a_skill_whose_trigger_misses_is_reported_as_skipped() {
        let dir = recall_project("miss", "path = [\"**/*.py\"]", "");
        let out = recall_in(&dir, &["--path", "src/main.rs"]);
        assert!(out.starts_with("skip"), "{out}");
    }

    /// V29 through the verb: the refusal beats the match, and says so.
    #[test]
    fn the_do_not_fire_block_wins_and_is_named() {
        let dir = recall_project(
            "refuse",
            "path = [\"**/*.rs\"]",
            "path = [\"src/**\"]",
        );
        let out = recall_in(&dir, &["--path", "src/main.rs"]);
        assert!(out.starts_with("skip"), "{out}");
        assert!(out.contains("WINS"), "{out}");
    }

    #[test]
    fn the_tool_flag_reaches_the_matcher() {
        let dir = recall_project("tool", "tool = [\"Edit\"]", "");
        assert!(recall_in(&dir, &["--tool", "Edit"]).starts_with("load"));
        assert!(recall_in(&dir, &["--tool", "Bash"]).starts_with("skip"));
    }

    /// `--cwd` is the situation's directory, and it stands in for the path
    /// when no file is named -- which is the state someone is in when they
    /// ask what loads here before touching anything.
    #[test]
    fn the_cwd_flag_stands_in_for_an_unnamed_path() {
        let dir = recall_project("cwd", "path = [\"**/backend/**\"]", "");
        assert!(
            recall_in(&dir, &["--cwd", "srv/backend/api"]).starts_with("load")
        );
        assert!(recall_in(&dir, &["--cwd", "srv/web"]).starts_with("skip"));
    }

    /// A situation typed as bare words is ONE description, not a usage
    /// error. A shell splits it and refusing the second word would fail
    /// for a reason nobody could guess.
    #[test]
    fn a_multi_word_situation_is_joined_into_one_text() {
        let held = parse_recall(&args(&["editing", "a", "test"]));
        assert_eq!(held.map(|a| a.text).unwrap_or_default(), "editing a test");
    }

    #[test]
    fn a_word_trigger_matches_the_situation_text() {
        let dir = recall_project("word", "word = [\"clippy\"]", "");
        assert!(recall_in(&dir, &["run", "clippy", "now"]).starts_with("load"));
        assert!(recall_in(&dir, &["write", "docs"]).starts_with("skip"));
    }

    /// V7: report-only. Asking what loads must NOT count a firing -- V11's
    /// counter has to mean the artifact was loaded, or `--dead` measures
    /// curiosity instead of use.
    #[test]
    fn recall_counts_no_firing() {
        let dir = recall_project("no-fire", "path = [\"**/*.rs\"]", "");
        let _ = recall_in(&dir, &["--path", "src/main.rs"]);
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(held.extracted.first().map(|row| row.fires), Some(0));
    }

    #[test]
    fn recall_json_is_parseable_and_carries_the_verdict() {
        let dir = recall_project("json", "path = [\"**/*.rs\"]", "");
        let text =
            recall_in(&dir, &["--path", "src/main.rs", "--format", "json"]);
        let parsed: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_default();
        let loads = parsed
            .get("rows")
            .and_then(|rows| rows.get(0))
            .and_then(|first| first.get("loads"));
        assert_eq!(loads, Some(&serde_json::json!(true)), "{text}");
    }

    #[test]
    fn an_empty_ledger_recalls_nothing_and_exits_zero() {
        let dir = check_project("recall-empty");
        assert_eq!(recall_in(&dir, &["anything"]), "");
        assert_eq!(perform(Action::Recall(dash_c(&dir)), &env()), 0);
    }

    #[test]
    fn an_unknown_recall_flag_is_an_error() {
        assert!(parse_recall(&args(&["--nope"])).is_err());
        assert!(parse_recall(&args(&["--tool"])).is_err());
    }

    /// `reported` routes only the verbs that answer with an `Output`.
    /// The catch-all is unreachable through `perform`, which is why it is
    /// asserted HERE: an arm nothing can reach and nothing tests is an
    /// arm that silently rots into the wrong behaviour.
    #[test]
    fn a_verb_that_does_not_report_an_output_says_so() {
        let said = reported(Action::PrintVersion, &env()).err();
        assert!(
            said.is_some_and(|s| s.contains("does not report an Output")),
            "the catch-all changed shape"
        );
    }

    /// A reader that fails mid-line is an ERROR, not a silent yes.
    #[test]
    fn a_failing_reader_is_not_consent() {
        struct Broken;
        impl std::io::Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("no"))
            }
        }
        let mut input = std::io::BufReader::new(Broken);
        assert!(read_answer(&mut input).is_err());
    }
    /// V41. A `runner` names a GATE STEP, and only an `M` rule has one.
    /// Ignoring it would read, to whoever filled it in, exactly like
    /// honouring it -- so the plan is refused and the message says which
    /// field to clear.
    #[test]
    fn a_runner_on_a_skill_step_is_refused() {
        let dir = plan_project("runner-on-skill");
        let plan_path = dir.join("x.plan");
        let _ = std::fs::write(
            &plan_path,
            "format = 1\nfingerprint = []\n\n[[steps]]\nid = \"abc1234\"\nsrc = \"CLAUDE.md\"\nline_start = 1\nline_end = 1\ntext = \"- when editing `.rs`, prefer modules\"\nlabel = \"S1\"\nartifact = \".claude/skills/x/SKILL.md\"\nrunner = \"ascii\"\nwiring = \"\"\n",
        );
        let result =
            apply_in(&dir, &["--auto-approve", &plan_path.to_string_lossy()]);
        let message = result.err().unwrap_or_default();
        assert!(message.contains("runner"), "{message}");
        assert!(message.contains("GATE STEP"), "{message}");
    }
    /// V28: a config with no roots is a SETUP problem, and the message
    /// names the command that fixes it rather than reporting an empty
    /// corpus as if that were a normal answer.
    #[test]
    fn a_config_with_no_roots_names_the_command_that_fixes_it() {
        let dir = PathBuf::from("target").join("cli-check").join("no-roots");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ =
            std::fs::write(dir.join("rekall.toml"), "[sources]\nroots = []\n");
        let failed =
            scan_command(&args(&["-C", &dir.to_string_lossy()]), &env());
        let said = failed.err().unwrap_or_default();
        assert!(said.contains("rekall init"), "{said}");
    }
    /// A Bash payload, whose text lives in the COMMAND rather than in a
    /// prompt. This is the shape B7 was invisible in.
    fn command_payload(dir: &Path, command: &str) -> String {
        format!(
            "{{\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"Bash\",\
             \"tool_input\":{{\"command\":\"{command}\"}},\"cwd\":\"{}\"}}",
            dir.to_string_lossy()
        )
    }

    /// B7, and V18 where it actually broke. A `word` trigger is tested
    /// against the situation TEXT, and a tool call carries no prompt -- so
    /// reading `prompt` alone made every `word` trigger dead on the one
    /// event `hook` runs on, while `recall`, which takes the situation as
    /// an argument, said `load` for the same skill.
    ///
    /// ONE payload drives BOTH verbs here. Two tests that each build their
    /// own input is how the two paths drifted while both looked right.
    #[test]
    fn a_word_trigger_fires_on_what_the_tool_was_asked_to_run() {
        let dir = recall_project("word-command", "word = [\"cargo test\"]", "");
        let printed = recall_in(&dir, &["--tool", "Bash", "cargo test"]);
        let decided = hook_command(&command_payload(&dir, "cargo test"), &dir)
            .unwrap_or_default();
        assert!(printed.starts_with("load"), "{printed}");
        assert!(
            decided.get("hookSpecificOutput").is_some(),
            "recall said load and hook said nothing: {decided}"
        );
    }

    /// And the do-not-fire clause still wins over the command text.
    #[test]
    fn a_refusal_clause_wins_over_the_command_text() {
        let dir = recall_project(
            "word-refused",
            "word = [\"cargo test\"]",
            "word = [\"--list\"]",
        );
        let decided =
            hook_command(&command_payload(&dir, "cargo test --list"), &dir)
                .unwrap_or_default();
        assert_eq!(decided, serde_json::json!({}), "{decided}");
    }
    /// V43, at the place it costs. What reaches the model is the RULE, not
    /// the file that carries it -- MEASURED on this crate's own skill, 330
    /// tokens of file to say 46, paid at the fire point on every match.
    #[test]
    fn the_hook_injects_the_payload_and_not_the_scaffold() {
        let dir = recall_project("payload-only", "path = [\"**/*.rs\"]", "");
        let context = hook_in(&dir, "a.rs")
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(context.contains("never commit"), "{context}");
        assert!(!context.contains("Fires when"), "{context}");
        assert!(!context.contains("rekall:payload"), "{context}");
    }
}
