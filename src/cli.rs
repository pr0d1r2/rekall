//! Argument parsing and dispatch.
//!
//! Hand-rolled rather than derived. The verb set is small and fixed by
//! section I, and the exit codes are part of the published contract -- 0 ok,
//! 1 drift, 2 usage -- so the mapping from argument to exit stays visible
//! here instead of inside a macro.

use crate::{config, scan};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
#[must_use]
pub fn decide(args: &[String]) -> Action {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => Action::PrintUsage { code: 0 },
        Some("-V" | "--version") => Action::PrintVersion,
        Some("scan") => {
            Action::Scan(args.get(1..).unwrap_or_default().to_vec())
        }
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
#[must_use]
pub fn perform(action: Action) -> u8 {
    match action {
        Action::PrintUsage { code } => show_usage(code),
        Action::PrintVersion => show_version(),
        Action::Scan(flags) => run_scan(&flags),
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

/// The whole binary, in one line of side effect.
pub fn dispatch(args: &[String]) -> ExitCode {
    ExitCode::from(perform(decide(args)))
}

fn run_scan(flags: &[String]) -> u8 {
    match scan_command(flags) {
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
pub struct Output {
    pub text: String,
    /// Files that could not be read. NAMED, never silently dropped:
    /// reporting a smaller corpus as if it were the whole one is the skip
    /// V26 forbids.
    pub warnings: Vec<String>,
}

/// Run `scan` end to end: parse flags, resolve config, inventory, render.
pub fn scan_command(flags: &[String]) -> Result<Output, String> {
    let args = parse_scan(flags)?;
    let cwd = working_dir(args.cwd.as_deref())?;
    let resolved = resolve(&cwd)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    let outcome = inventory(&resolved, &args, &cwd)?;
    Ok(Output {
        text: render(&outcome, &args)?,
        warnings: warnings(&outcome),
    })
}

pub const NO_SOURCES: &str = "no corpus roots configured. \
Run `rekall init` to detect them, or add [sources].roots to rekall.toml";

fn working_dir(requested: Option<&Path>) -> Result<PathBuf, String> {
    match requested {
        Some(path) => Ok(path.to_path_buf()),
        None => std::env::current_dir()
            .map_err(|error| format!("no working directory: {error}")),
    }
}

fn inventory(
    resolved: &Resolved,
    args: &ScanArgs,
    base: &Path,
) -> Result<scan::Outcome, String> {
    let home = std::env::var("HOME").ok();
    let corpus = scan::Corpus {
        roots: &resolved.roots,
        globs: &resolved.globs,
        home: home.as_deref(),
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
        let Some(parsed) = parsed.ok() else {
            unreachable!("these flags parse")
        };
        assert_eq!(parsed.filter.class.as_deref(), Some("M"));
        assert_eq!(parsed.filter.sharpness, Some(2));
        assert_eq!(parsed.filter.top, Some(5));
    }

    #[test]
    fn sources_and_json_are_flags_without_values() {
        let parsed = parse_scan(&args(&["--sources", "--format", "json"]));
        let Some(parsed) = parsed.ok() else {
            unreachable!("these flags parse")
        };
        assert!(parsed.sources && parsed.json);
    }

    #[test]
    fn dash_c_sets_the_working_directory() {
        let parsed = parse_scan(&args(&["-C", "/tmp/x"]));
        let Some(parsed) = parsed.ok() else {
            unreachable!("these flags parse")
        };
        assert_eq!(parsed.cwd.as_deref(), Some(Path::new("/tmp/x")));
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
        let outcome = scan_command(&args(&["-C", "/tmp"]));
        let Some(message) = outcome.err() else {
            unreachable!("/tmp has no rekall.toml")
        };
        assert!(message.contains("rekall init"), "message was {message}");
    }

    #[test]
    fn an_unknown_flag_reaches_the_caller_as_an_error() {
        assert!(scan_command(&args(&["--nope"])).is_err());
    }

    #[test]
    fn warnings_are_empty_when_everything_was_readable() {
        let outcome = scan::Outcome {
            report: scan::Report {
                rows: Vec::new(),
                sources: Vec::new(),
            },
            unreadable: Vec::new(),
        };
        assert!(warnings(&outcome).is_empty());
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

    #[test]
    fn json_output_is_parseable_and_carries_the_report_anatomy() {
        let outcome = scan::Outcome {
            report: scan::Report {
                rows: Vec::new(),
                sources: Vec::new(),
            },
            unreadable: Vec::new(),
        };
        let json_args = ScanArgs {
            json: true,
            ..ScanArgs::default()
        };
        let Ok(text) = render(&outcome, &json_args) else {
            unreachable!("an empty report serializes")
        };
        assert!(text.contains("\"rows\"") && text.contains("\"sources\""));
    }

    #[test]
    fn human_output_is_not_json() {
        let outcome = scan::Outcome {
            report: scan::Report {
                rows: Vec::new(),
                sources: Vec::new(),
            },
            unreadable: Vec::new(),
        };
        let Ok(text) = render(&outcome, &ScanArgs::default()) else {
            unreachable!("an empty report renders")
        };
        assert!(!text.contains("\"rows\""));
    }

    #[test]
    fn working_dir_prefers_the_requested_path() {
        assert_eq!(
            working_dir(Some(Path::new("/tmp/x"))).ok(),
            Some(PathBuf::from("/tmp/x"))
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
    #[test]
    fn a_specified_verb_is_unimplemented_not_unknown() {
        for verb in VERBS {
            if verb == "scan" {
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
        assert_eq!(perform(Action::PrintUsage { code: 0 }), 0);
    }

    #[test]
    fn version_exits_zero() {
        assert_eq!(perform(Action::PrintVersion), 0);
    }

    /// Both are usage errors -- section I fixes the exit set, and neither
    /// an unknown verb nor an unimplemented one is drift.
    #[test]
    fn unknown_and_unimplemented_both_exit_two() {
        assert_eq!(
            perform(Action::Unimplemented("plan".to_string())),
            USAGE_EXIT
        );
        assert_eq!(perform(Action::Unknown("scna".to_string())), USAGE_EXIT);
        assert_eq!(USAGE_EXIT, 2);
    }

    #[test]
    fn usage_shown_after_an_unknown_verb_still_reports_failure() {
        assert_eq!(perform(Action::Unknown("nope".to_string())), 2);
    }

    #[test]
    fn a_scan_that_cannot_run_exits_two() {
        assert_eq!(perform(Action::Scan(args(&["--nope"]))), USAGE_EXIT);
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
        scan_command(&flags)
    }

    #[test]
    fn a_configured_project_scans_end_to_end() {
        let dir = project("end-to-end");
        let Ok(output) = run_in(&dir, &[]) else {
            unreachable!("a configured project scans")
        };
        assert!(output.text.contains("M1"), "text was {}", output.text);
        assert!(output.warnings.is_empty());
    }

    /// Paths in the report are relative to the project, not to wherever the
    /// process happened to start. This is what keeps ids findable later.
    #[test]
    fn output_paths_are_project_relative() {
        let dir = project("relative");
        let Ok(output) = run_in(&dir, &[]) else {
            unreachable!("scans")
        };
        assert!(
            output.text.contains("CLAUDE.md:1-1"),
            "text was {}",
            output.text
        );
        assert!(!output.text.contains("target/test-project"));
    }

    #[test]
    fn json_output_parses_and_matches_the_human_row_count() {
        let dir = project("json");
        let Ok(human) = run_in(&dir, &[]) else {
            unreachable!("scans")
        };
        let rows = json_rows(&dir);
        assert_eq!(rows, human.text.lines().count());
    }

    fn json_rows(dir: &Path) -> usize {
        let Ok(json) = run_in(dir, &["--format", "json"]) else {
            unreachable!("scans")
        };
        let parsed: serde_json::Value = match serde_json::from_str(&json.text) {
            Ok(value) => value,
            Err(error) => unreachable!("json output must parse: {error}"),
        };
        parsed
            .get("rows")
            .and_then(|rows| rows.as_array())
            .map_or(0, std::vec::Vec::len)
    }

    #[test]
    fn the_sources_flag_adds_a_source_line() {
        let dir = project("sources");
        let Ok(plain) = run_in(&dir, &[]) else {
            unreachable!("scans")
        };
        let Ok(with_sources) = run_in(&dir, &["--sources"]) else {
            unreachable!("scans")
        };
        assert!(with_sources.text.lines().count() > plain.text.lines().count());
        assert!(with_sources.text.contains("project"));
    }

    #[test]
    fn filters_reach_through_the_command() {
        let dir = project("filter");
        let Ok(output) = run_in(&dir, &["--class", "S"]) else {
            unreachable!("scans")
        };
        assert_eq!(output.text.lines().count(), 1);
        assert!(output.text.contains("S2"));
    }

    #[test]
    fn an_unreadable_corpus_file_becomes_a_named_warning() {
        let dir = project("warned");
        let _ = std::fs::write(dir.join("bad.md"), [0xff_u8, 0xfe]);
        let Ok(output) = run_in(&dir, &[]) else {
            unreachable!("scans")
        };
        assert_eq!(output.warnings.len(), 1, "{:?}", output.warnings);
        assert!(
            output
                .warnings
                .first()
                .is_some_and(|w| w.contains("bad.md")),
            "{:?}",
            output.warnings
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
        assert_eq!(perform(Action::Scan(flags)), 0);
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
        assert_eq!(perform(Action::Scan(flags)), 0);
    }
}
