//! `rekall` -- turn always-on agent prose into tangibles.
//!
//! A THIN CALLER, deliberately. Everything the binary decides lives in
//! `rekall::cli`, where a test can assert on values instead of on captured
//! stdout -- so the only thing that cannot be exercised by the suite is the
//! three lines below.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    rekall::cli::dispatch(&args)
}
