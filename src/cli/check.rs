use super::{Env, Format, Output, need, parse_format};
use crate::{check, corpus, ledger, scan};
use std::path::{Path, PathBuf};
/// `check` takes no ids. It audits EVERYTHING the ledger claims, because a
/// gate you can point at a subset is a gate that reports green for the
/// part you did not point it at.
#[derive(Debug, Default)]
pub struct CheckArgs {
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_check(args: &[String]) -> Result<CheckArgs, String> {
    let mut out = CheckArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_check_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_check_arg<'a>(
    out: &mut CheckArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json;
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other => return Err(format!("unknown flag `{other}`")),
    }
    Ok(())
}

/// A finished audit, with the exit code it earned.
///
/// The count travels WITH the output because drift is not an error: the
/// command ran correctly and the answer is bad news. Folding it into
/// `Err` would make a working gate indistinguishable from a broken one.
pub struct Checked {
    pub output: Output,
    pub drift: usize,
}

/// Run `check` end to end.
///
/// CPU-only, and it reads nothing but the ledger and the files the ledger
/// names (V6). No key, no network, so it runs in `hk` on every commit and
/// in a CI that can reach nothing.
pub fn check_command(flags: &[String], env: &Env) -> Result<Checked, String> {
    let args = parse_check(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let held =
        ledger::load(&ledger::path_in(&base)).map_err(|e| e.to_string())?;
    let bytes = read_rows(&base, &held);
    let seen = zip_rows(&held, &bytes);
    let on_disk = artifacts_on_disk(&base, env)?;
    render_check(&check::audit(&seen, &on_disk), args.json)
}

/// The bytes each row is judged against, read once.
///
/// A file that cannot be read comes back as `None` rather than an error.
/// It IS a finding -- a ledger naming a file nobody can open is exactly
/// the drift this gate exists for -- and aborting on the first one would
/// report a single problem where there may be five.
struct Bytes {
    source: Option<String>,
    artifact: Option<String>,
}

fn read_rows(base: &Path, held: &ledger::Ledger) -> Vec<Bytes> {
    held.extracted
        .iter()
        .map(|row| Bytes {
            source: std::fs::read_to_string(base.join(&row.src)).ok(),
            artifact: std::fs::read_to_string(base.join(&row.artifact)).ok(),
        })
        .collect()
}

fn zip_rows<'a>(
    held: &'a ledger::Ledger,
    bytes: &'a [Bytes],
) -> Vec<check::Seen<'a>> {
    held.extracted
        .iter()
        .zip(bytes)
        .map(|(row, read)| check::Seen {
            row,
            source: read.source.as_deref(),
            artifact: read.artifact.as_deref(),
        })
        .collect()
}

/// Where `plan` materializes artifacts, and therefore the only places an
/// ORPHAN can be found.
///
/// Two named roots rather than a walk of the project: everything outside
/// them is somebody else's file, and a gate that called every unclaimed
/// file in the repo an orphan would be unusable on its first run.
const ARTIFACT_ROOTS: [&str; 2] = [".rekall/rules", ".claude/skills"];
const ARTIFACT_GLOBS: [&str; 2] = ["**/*.sh", "**/SKILL.md"];

fn artifacts_on_disk(base: &Path, env: &Env) -> Result<Vec<String>, String> {
    let roots: Vec<String> =
        ARTIFACT_ROOTS.iter().map(|r| (*r).to_string()).collect();
    let globs: Vec<String> =
        ARTIFACT_GLOBS.iter().map(|g| (*g).to_string()).collect();
    let walked = corpus::files(&roots, &globs, env.home.as_deref(), base)
        .map_err(|error| error.to_string())?;
    Ok(walked
        .files
        .iter()
        .map(|path| scan::stable_name(path, base, env.home.as_deref()))
        .collect())
}

/// V17: both formats, same anatomy. V28: human output is SILENT when
/// clean -- output that always appears is output nobody reads.
fn render_check(found: &[check::Drift], json: bool) -> Result<Checked, String> {
    let text = if json {
        serde_json::to_string_pretty(&Report { drift: found })
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string())?
    } else {
        check::render_human(found)
    };
    Ok(Checked {
        output: Output {
            text,
            warnings: Vec::new(),
        },
        drift: found.len(),
    })
}

/// The JSON envelope. An OBJECT rather than a bare array, so a later
/// field -- a count, a summary -- can be added without changing the type
/// every consumer already parses.
#[derive(serde::Serialize)]
struct Report<'a> {
    drift: &'a [check::Drift],
}
