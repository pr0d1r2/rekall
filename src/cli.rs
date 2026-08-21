//! Argument parsing and dispatch.
//!
//! Hand-rolled rather than derived. The verb set is small and fixed by
//! section I, and the exit codes are part of the published contract -- 0 ok,
//! 1 drift, 2 usage -- so the mapping from argument to exit stays visible
//! here instead of inside a macro.

use crate::{config, init, scan, show};
use std::path::{Path, PathBuf};

pub const USAGE_EXIT: u8 = 2;

#[derive(Debug, PartialEq, Eq)]
pub enum Format {
    Human,
    Json,
}

/// An unknown `--format` is a USAGE error, never a silent fall back to
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

#[derive(Debug, Default)]
pub struct ScanArgs {
    pub filter: scan::Filter,
    pub sources: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

/// Parse `scan`'s flags.
///
/// A flag that takes a value and is given none is an error rather than a
/// default: `--top` with nothing after it is a truncated command line, and
/// guessing what was meant is how a scan silently reports the wrong slice.
pub fn parse_scan(args: &[String]) -> Result<ScanArgs, String> {
    let mut out = ScanArgs::default();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        apply_scan_flag(&mut out, flag, &mut rest)?;
    }
    Ok(out)
}

fn apply_scan_flag<'a>(
    out: &mut ScanArgs,
    flag: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match flag {
        "--sources" => out.sources = true,
        "--format" => {
            out.json = parse_format(&need(flag, rest)?)? == Format::Json
        }
        "--class" => out.filter.class = Some(need(flag, rest)?),
        "--sharpness" => {
            out.filter.sharpness = Some(parse_level(&need(flag, rest)?)?)
        }
        "--top" => out.filter.top = Some(parse_count(&need(flag, rest)?)?),
        "-C" => out.cwd = Some(PathBuf::from(need(flag, rest)?)),
        other => return Err(format!("unknown flag `{other}`")),
    }
    Ok(())
}

fn need<'a>(
    flag: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<String, String> {
    rest.next()
        .cloned()
        .ok_or_else(|| format!("`{flag}` needs a value"))
}

fn parse_level(raw: &str) -> Result<u8, String> {
    match raw {
        "1" => Ok(1),
        "2" => Ok(2),
        "3" => Ok(3),
        other => Err(format!("--sharpness must be 1, 2 or 3, got `{other}`")),
    }
}

fn parse_count(raw: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|_| format!("--top needs a whole number, got `{raw}`"))
}

/// Load both config scopes and report every root with the scope that named
/// it. The union lives in `config::merge`; this keeps the attribution so
/// `--sources` can show where each root came from.
pub fn resolve(cwd: &Path) -> Result<Resolved, String> {
    let user = load_optional(config::user_path().as_deref())?;
    let project = load_optional(config::find_project(cwd).as_deref())?;
    let merged = config::merge(user.clone(), project.clone());
    Ok(Resolved {
        roots: config::attribute(&user, &project),
        globs: merged.sources.globs.unwrap_or_default(),
    })
}

pub struct Resolved {
    pub roots: Vec<(String, config::Scope)>,
    pub globs: Vec<String>,
}

/// A config file that is absent is fine; one that is present and broken is
/// not. Treating a parse error as "no config" would run the scan against a
/// corpus the user never described.
fn load_optional(path: Option<&Path>) -> Result<config::Config, String> {
    let Some(path) = path else {
        return Ok(config::Config::default());
    };
    if !path.is_file() {
        return Ok(config::Config::default());
    }
    config::load_file(path).map_err(|error| error.to_string())
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
  revert  reverse one extraction, verbatim

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
        Some("scan") => Action::Scan(rest(args)),
        Some("init") => Action::Init(rest(args)),
        Some("show") => Action::Show(rest(args)),
        Some(verb) if VERBS.contains(&verb) => {
            Action::Unimplemented(verb.to_string())
        }
        Some(other) => Action::Unknown(other.to_string()),
    }
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
        Action::Scan(flags) => run_scan(&flags, env),
        Action::Init(flags) => run_init(&flags, env),
        Action::Show(flags) => run_show(&flags, env),
        Action::Unimplemented(verb) => say_unimplemented(&verb),
        Action::Unknown(other) => say_unknown(&other),
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

/// `init` flags. `--force` is separate from `--auto-approve` on purpose:
/// this writes its OWN config, never the corpus, so it is not the
/// destructive path V20 guards.
#[derive(Debug, Default)]
pub struct InitArgs {
    pub force: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_init(args: &[String]) -> Result<InitArgs, String> {
    let mut out = InitArgs::default();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        apply_init_flag(&mut out, flag, &mut rest)?;
    }
    Ok(out)
}

fn apply_init_flag<'a>(
    out: &mut InitArgs,
    flag: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match flag {
        "--force" => out.force = true,
        "--format" => {
            out.json = parse_format(&need(flag, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(flag, rest)?)),
        other => return Err(format!("unknown flag `{other}`")),
    }
    Ok(())
}

/// Run `init` end to end.
pub fn init_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_init(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let report = init::run(&base, env.home.as_deref(), args.force)
        .map_err(|error| error.to_string())?;
    Ok(Output {
        text: render_init(&report, args.json)?,
        warnings: Vec::new(),
    })
}

fn render_init(report: &init::Report, json: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string_pretty(report)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(init::render_human(report))
}

fn run_init(flags: &[String], env: &Env) -> u8 {
    match init_command(flags, env) {
        Ok(output) => {
            print!("{}", output.text);
            0
        }
        Err(message) => {
            eprintln!("rekall: {message}");
            USAGE_EXIT
        }
    }
}

/// `show` takes ONE id (or an unambiguous prefix of one) plus the usual
/// format and directory flags.
#[derive(Debug, Default)]
pub struct ShowArgs {
    pub id: Option<String>,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_show(args: &[String]) -> Result<ShowArgs, String> {
    let mut out = ShowArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_show_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_show_arg<'a>(
    out: &mut ShowArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        id => return set_id(out, id),
    }
    Ok(())
}

/// A second positional is a MISTAKE, not a second lookup. `show a b` most
/// likely means the shell split something, and quietly showing only `a`
/// would answer a question nobody asked.
fn set_id(out: &mut ShowArgs, id: &str) -> Result<(), String> {
    if out.id.is_some() {
        return Err(format!("`show` takes one id, got a second: `{id}`"));
    }
    out.id = Some(id.to_string());
    Ok(())
}

/// Run `show` end to end.
pub fn show_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_show(flags)?;
    let id = args.id.clone().ok_or_else(|| NO_ID.to_string())?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let resolved = resolve(&base)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    render_lookup(&find(&resolved, &base, env, &id)?, &id, args.json)
}

pub const NO_ID: &str = "`show` needs an id -- copy one from `rekall scan`";

fn find(
    resolved: &Resolved,
    base: &Path,
    env: &Env,
    id: &str,
) -> Result<show::Lookup, String> {
    let at = scan::Corpus {
        roots: &resolved.roots,
        globs: &resolved.globs,
        home: env.home.as_deref(),
        base,
    };
    show::lookup(&at, id).map_err(|error| error.to_string())
}

fn render_lookup(
    found: &show::Lookup,
    id: &str,
    json: bool,
) -> Result<Output, String> {
    match found {
        show::Lookup::Unique(found) => Ok(Output {
            text: render_found(found, json)?,
            warnings: Vec::new(),
        }),
        show::Lookup::Ambiguous(ids) => Err(format!(
            "`{id}` matches {} statements: {}. Use more characters.",
            ids.len(),
            ids.join(", ")
        )),
        show::Lookup::Missing => Err(format!("no statement matches `{id}`")),
    }
}

fn render_found(found: &show::Found, json: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string_pretty(found)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(show::render_human(found))
}

fn run_show(flags: &[String], env: &Env) -> u8 {
    match show_command(flags, env) {
        Ok(output) => {
            print!("{}", output.text);
            0
        }
        Err(message) => {
            eprintln!("rekall: {message}");
            USAGE_EXIT
        }
    }
}

fn run_scan(flags: &[String], env: &Env) -> u8 {
    match scan_command(flags, env) {
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

/// What a scan produced for a caller to print.
#[derive(Debug)]
pub struct Output {
    pub text: String,
    /// Files that could not be read. NAMED, never silently dropped:
    /// reporting a smaller corpus as if it were the whole one is the skip
    /// V26 forbids.
    pub warnings: Vec<String>,
}

/// Run `scan` end to end: parse flags, resolve config, inventory, render.
pub fn scan_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_scan(flags)?;
    let cwd = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let resolved = resolve(&cwd)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    let outcome = inventory(&resolved, &args, &cwd, env.home.as_deref())?;
    Ok(Output {
        text: render(&outcome, &args)?,
        warnings: warnings(&outcome),
    })
}

pub const NO_SOURCES: &str = "no corpus roots configured. \
Run `rekall init` to detect them, or add [sources].roots to rekall.toml";

fn inventory(
    resolved: &Resolved,
    args: &ScanArgs,
    base: &Path,
    home: Option<&str>,
) -> Result<scan::Outcome, String> {
    let corpus = scan::Corpus {
        roots: &resolved.roots,
        globs: &resolved.globs,
        home,
        base,
    };
    scan::run(&corpus, &args.filter).map_err(|error| error.to_string())
}

fn render(outcome: &scan::Outcome, args: &ScanArgs) -> Result<String, String> {
    if args.json {
        return serde_json::to_string_pretty(&outcome.report)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(scan::render_human(&outcome.report, args.sources))
}

fn warnings(outcome: &scan::Outcome) -> Vec<String> {
    outcome
        .unreadable
        .iter()
        .map(|path| format!("could not read {} -- skipped", path.display()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// An absent config is fine; a broken one is not. Treating a parse
    /// error as "no config" would scan a corpus the user never described.
    #[test]
    fn an_absent_config_is_not_an_error() {
        assert!(load_optional(None).is_ok());
        assert!(
            load_optional(Some(Path::new("definitely/not/here.toml"))).is_ok()
        );
    }

    #[test]
    fn a_broken_config_is_an_error() {
        assert!(load_optional(Some(Path::new("Cargo.lock"))).is_err());
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

    #[test]
    fn warnings_are_empty_when_everything_was_readable() {
        assert!(warnings(&empty_outcome()).is_empty());
    }

    #[test]
    fn an_unreadable_file_is_named_in_the_warnings() {
        let outcome = scan::Outcome {
            report: scan::Report {
                rows: Vec::new(),
                sources: Vec::new(),
            },
            unreadable: vec![PathBuf::from("/secret/notes.md")],
        };
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
    const IMPLEMENTED: [&str; 3] = ["scan", "init", "show"];

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
}
