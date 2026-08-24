use super::apply::now;
use super::{Env, Format, Output, need, net_reclaim, parse_format};
use crate::{ledger, log};
use std::path::PathBuf;
/// `log` reads the ledger. Report-only, and the one verb whose whole job
/// is to make a rule's uselessness measurable rather than suspected.
#[derive(Debug, Default)]
pub struct LogArgs {
    pub dead: bool,
    pub since: Option<String>,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_log(args: &[String]) -> Result<LogArgs, String> {
    let mut out = LogArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_log_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_log_arg<'a>(
    out: &mut LogArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--dead" => out.dead = true,
        "--since" => out.since = Some(need(arg, rest)?),
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json;
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other => return Err(format!("unknown flag `{other}`")),
    }
    Ok(())
}

/// Run `log` end to end.
pub fn log_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_log(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let held =
        ledger::load(&ledger::path_in(&base)).map_err(|e| e.to_string())?;
    let mut report = log::report(&held, log_filter(&args)?);
    let skipped = fill_log_tokens(&mut report);
    let mut out = render_log(&report, args.json)?;
    out.warnings = skipped;
    Ok(out)
}

/// The tokens RECLAIMED by each extraction, from the verbatim text the
/// ledger kept (V9). One call for the whole log, same as `scan`.
fn fill_log_tokens(report: &mut log::Report) -> Vec<String> {
    let ids: Vec<String> = report
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect();
    let texts: Vec<String> = report
        .entries
        .iter()
        .map(|entry| entry.text.clone())
        .collect();
    for (entry, net) in report.entries.iter_mut().zip(net_reclaim(&ids, &texts))
    {
        entry.reclaimed = net;
    }
    Vec::new()
}

/// The duration is resolved against the clock HERE, at the edge, so
/// `log`'s own filtering stays a pure function of two numbers.
fn log_filter(args: &LogArgs) -> Result<log::Filter, String> {
    let since = match args.since.as_deref() {
        Some(raw) => Some(log::floor(now(), log::duration(raw)?)),
        None => None,
    };
    Ok(log::Filter {
        dead: args.dead,
        since,
    })
}

fn render_log(report: &log::Report, json: bool) -> Result<Output, String> {
    let text = if json {
        serde_json::to_string_pretty(report)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string())?
    } else {
        log::render_human(report)
    };
    Ok(Output {
        text,
        warnings: Vec::new(),
    })
}
