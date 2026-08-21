//! `rekall` -- turn always-on agent prose into tangibles.
//!
//! The process EDGE, and nothing else. It reads the two process globals the
//! library refuses to reach for -- the working directory and `HOME` -- and
//! hands them in, then turns a `u8` into an `ExitCode`.
//!
//! Everything that decides anything lives in `rekall::cli`, where a test can
//! assert on values. This file is excluded from coverage by
//! `.config/nextest`-style configuration in `hk.pkl`, and the exclusion is
//! only honest because there is nothing here to test: a binary's `fn main`
//! cannot be called by a unit test, so code kept here would be permanently
//! unverifiable rather than merely unverified.

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
        home: std::env::var("HOME").ok(),
    };
    ExitCode::from(cli::perform(cli::decide(&args), &env))
}
