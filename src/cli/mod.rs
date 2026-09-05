//! Argument parsing and dispatch.
//!
//! Hand-rolled rather than derived. The verb set is small and fixed by
//! section I, and the exit codes are part of the published contract -- 0 ok,
//! 1 drift, 2 usage -- so the mapping from argument to exit stays visible
//! here instead of inside a macro.

// The crate modules are reached by FULL PATH inside the verb modules,
// because `mod apply;` below would otherwise shadow `crate::apply`.
pub use env::{Env, HOME_OVERRIDE, corpus_home};

use crate::{statement, tokens};
use std::path::Path;

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
mod env;
mod hook;
mod init;
mod issue;
mod log;
mod plan;
mod publish;
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

#[derive(Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// PROSE is the default because a person is the default reader (V17).
    #[default]
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
pub const VERBS: [&str; 12] = [
    "init", "scan", "show", "plan", "apply", "check", "recall", "hook", "log",
    "catch", "revert", "issue",
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
  issue   hand a proven skill to the loop that tends it

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
    Issue(Vec<String>),
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
const IMPLEMENTS: [(&str, Make); 12] = [
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
    ("issue", Action::Issue),
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
pub(super) fn reported(action: Action, env: &Env) -> Result<Output, String> {
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
        Action::Issue(flags) => issue::issue_command(&flags, env),
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

/// The shared test fixtures, declared HERE beside the tests rather than
/// with the verb modules above. The module-size gate counts lines BEFORE
/// the first `#[cfg(test)]`, so a test-only declaration at the top of the
/// file would truncate that count and hide however much code followed it.
#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// V63: the override wins, so a session can be pointed at a scratch
    /// corpus instead of somebody's real agent memory.
    #[test]
    fn the_override_replaces_home() {
        assert_eq!(
            corpus_home(Some("/tmp/fake".into()), Some("/home/u".into())),
            Some("/tmp/fake".to_string())
        );
    }

    /// EMPTY IS UNSET. `config::non_empty_var` learned this from
    /// `XDG_CONFIG_HOME=""` resolving a user config to `/rekall/...`; here an
    /// empty override would resolve `~/x` to `/x`.
    #[test]
    fn an_empty_override_falls_back_to_home() {
        assert_eq!(
            corpus_home(Some(String::new()), Some("/home/u".into())),
            Some("/home/u".to_string())
        );
    }

    #[test]
    fn no_override_leaves_home_alone() {
        assert_eq!(
            corpus_home(None, Some("/home/u".into())),
            Some("/home/u".to_string())
        );
        assert_eq!(corpus_home(None, None), None);
    }

    /// An override with NO home behind it still stands: pointing at a
    /// scratch tree must not need a real home to exist.
    #[test]
    fn the_override_works_without_a_home() {
        assert_eq!(
            corpus_home(Some("/tmp/fake".into()), None),
            Some("/tmp/fake".to_string())
        );
    }
    // The CRATE modules, named explicitly. `use super::*` now also pulls in
    // the cli submodules of the same name, and an explicit import beats a
    // glob -- so this is what keeps `plan::Plan` meaning the domain type.

    use super::scan::{render, set_row_tokens, warnings};
    use crate::scan;

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
    const IMPLEMENTED: [&str; 12] = [
        "scan", "init", "show", "plan", "apply", "revert", "check", "log",
        "recall", "hook", "catch", "issue",
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
}
