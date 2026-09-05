//! `rekall` -- turn always-on agent prose into tangibles.
//!
//! The process EDGE, and nothing else. It reads the two process globals the
//! library refuses to reach for -- the working directory, `HOME`, and the
//! `REKALL_HOME` that overrides it (`V63`) -- and
//! hands them in, then turns a `u8` into an `ExitCode`.
//!
//! Everything that decides anything lives in `rekall::cli`, where a test can
//! assert on values. These lines are NOT excluded from coverage, and
//! `.coverage` says why: an exclusion would remove the only pressure keeping
//! this file thin, which is what let it reach 37 lines once. A binary's
//! `fn main` cannot be called by a unit test, so every line kept here is
//! permanently unverifiable -- and the gap staying visible in the number is
//! the cost of putting anything else here.

use rekall::cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Ok(cwd) = std::env::current_dir() else {
        eprintln!("rekall: cannot read the working directory");
        return ExitCode::from(cli::USAGE_EXIT);
    };
    let env = cli::Env {
        cwd,
        home: cli::corpus_home(
            std::env::var(cli::HOME_OVERRIDE).ok(),
            std::env::var("HOME").ok(),
        ),
    };
    ExitCode::from(cli::perform(cli::decide(&args), &env))
}
