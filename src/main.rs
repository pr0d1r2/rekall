//! `rekall` -- turn always-on agent prose into tangibles.
//!
//! Scaffold. Every verb SPEC.md section I defines is RECOGNIZED here and
//! says it is not implemented. A verb the binary has never heard of is a
//! typo; a verb it knows but cannot yet perform is a backlog row, and the
//! MESSAGE tells those apart.
//!
//! The EXIT CODE does not, and deliberately so: section I fixes the set at
//! 0 ok, 1 drift, 2 usage, and neither case is drift. Adding a fourth code
//! to make the scaffold more expressive would change a published contract
//! for a state that stops existing once the verbs land.
//!
//! Recognizing every verb from the first commit keeps the usage text and
//! that section in one place, and makes the gap loud rather than silent
//! (V26).

use rekall::cli;
use std::process::ExitCode;

/// Every verb SPEC.md section I defines, in the order it lists them.
const VERBS: [&str; 11] = [
    "init", "scan", "show", "plan", "apply", "check", "recall", "hook", "log",
    "catch", "revert",
];

const USAGE: &str = "\
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => usage(ExitCode::SUCCESS),
        Some("-V" | "--version") => version(),
        Some("scan") => scan(args.get(1..).unwrap_or_default()),
        Some(verb) if VERBS.contains(&verb) => unimplemented_verb(verb),
        Some(other) => unknown_verb(other),
    }
}

fn usage(code: ExitCode) -> ExitCode {
    print!("{USAGE}");
    code
}

fn version() -> ExitCode {
    println!("rekall {}", env!("CARGO_PKG_VERSION"));
    ExitCode::SUCCESS
}

/// A known verb with no implementation. Exit 2, the usage code: the caller
/// asked for something this build cannot do, and saying so beats a zero
/// exit that reports success for work that never happened.
fn unimplemented_verb(verb: &str) -> ExitCode {
    eprintln!(
        "rekall: `{verb}` is specified but not implemented in this build"
    );
    ExitCode::from(2)
}

fn unknown_verb(other: &str) -> ExitCode {
    eprintln!("rekall: unknown verb `{other}`\n");
    usage(ExitCode::from(2))
}

/// `rekall scan` -- the CPU core.
///
/// The binary is a THIN CALLER: it turns a result into an exit code and
/// prints. Everything it decides lives in `cli`, where a test can assert on
/// values instead of on captured stdout.
fn scan(flags: &[String]) -> ExitCode {
    match cli::scan_command(flags) {
        Ok(output) => {
            for warning in &output.warnings {
                eprintln!("rekall: {warning}");
            }
            print!("{}", output.text);
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("rekall: {message}");
            ExitCode::from(cli::USAGE_EXIT)
        }
    }
}
