use super::{
    Env, Format, NO_SOURCES, Output, Resolved, need, parse_format, resolve,
    spread,
};
use crate::{scan, tokens};
use std::path::{Path, PathBuf};

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

/// Run `scan` end to end: parse flags, resolve config, inventory, render.
pub fn scan_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_scan(flags)?;
    let cwd = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let resolved = resolve(&cwd)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    let mut outcome = inventory(&resolved, &args, &cwd, env.home.as_deref())?;
    let skipped = fill_tokens(&mut outcome);
    let mut warnings = warnings(&outcome);
    warnings.extend(skipped);
    Ok(Output {
        text: render(&outcome, &args)?,
        warnings,
    })
}

/// Fill the tokens column, or NAME why it is empty (V8, V26).
///
/// ONE call for the whole report rather than one per row. The sibling
/// pays a tokenizer-table load per PROCESS, which is the entire cost, so
/// the per-row shape is the same work multiplied by the number of
/// statements -- see `tokens` for the measurement that settled it.
///
/// Returns the warnings to print rather than failing: a box without
/// `itok` gets a report with an empty column and a line saying why, which
/// is what V26 asks of an optional sibling.
pub(super) fn fill_tokens(outcome: &mut scan::Outcome) -> Vec<String> {
    let counted = tokens::count_all(&outcome.texts);
    spread(&mut outcome.report.rows, counted, set_row_tokens)
}

/// Named rather than a closure at each call site, so the one line that
/// assigns the column is the SAME line in production and in the test that
/// covers it -- an inline closure passed to the skip path is a body
/// nothing ever runs.
pub(super) fn set_row_tokens(row: &mut scan::Row, count: Option<u32>) {
    row.tokens = count;
}

pub(super) fn inventory(
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
        weights: &resolved.weights,
    };
    scan::run(&corpus, &args.filter).map_err(|error| error.to_string())
}

pub(super) fn render(
    outcome: &scan::Outcome,
    args: &ScanArgs,
) -> Result<String, String> {
    if args.json {
        return serde_json::to_string_pretty(&outcome.report)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(scan::render_human(&outcome.report, args.sources))
}

pub(super) fn warnings(outcome: &scan::Outcome) -> Vec<String> {
    outcome
        .unreadable
        .iter()
        .map(|path| format!("could not read {} -- skipped", path.display()))
        .collect()
}
