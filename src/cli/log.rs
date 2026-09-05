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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::testing::*;
    use crate::cli::{Action, USAGE_EXIT, decide, perform};
    use crate::ledger;
    use std::path::Path;

    #[test]
    fn log_is_dispatched() {
        assert_eq!(
            decide(&args(&["log", "--dead"])),
            Action::Log(args(&["--dead"]))
        );
    }

    /// The whole row, from a real extraction rather than a hand-written
    /// ledger: span, class, fire count and the verbatim text.
    #[test]
    fn log_reports_the_span_the_class_and_the_original_text() {
        let dir = check_project("log-row");
        let (id, _) = extracted(&dir);
        let text = log_in(&dir, &[]).map(|o| o.text).unwrap_or_default();
        assert!(text.contains(&id), "{text}");
        assert!(text.contains("CLAUDE.md:3-3"), "{text}");
        assert!(text.contains("fires=0"), "{text}");
        assert!(text.contains("- never commit to `main`"), "{text}");
    }

    /// V11: `--dead` is the measurement that makes deletion arithmetic
    /// instead of nerve. A fresh extraction has never fired, so it is
    /// dead; one that has fired drops out.
    #[test]
    fn dead_lists_the_never_fired_and_drops_the_rest() {
        let dir = check_project("log-dead");
        let (id, _) = extracted(&dir);
        assert!(dead_text(&dir).contains(&id), "a fresh extraction is dead");
        record_a_firing(&dir, &id);
        assert_eq!(
            dead_text(&dir),
            "",
            "a fired artifact was still called dead"
        );
    }

    /// Count one firing straight into the ledger. `hook` will do this for
    /// real (T13); until then the counter is exercised where it lives.
    fn record_a_firing(dir: &Path, id: &str) {
        let path = ledger::path_in(dir);
        let mut held = ledger::load(&path).unwrap_or_default();
        held.fired(id);
        let _ = ledger::save(&path, &held);
    }

    #[test]
    fn an_empty_ledger_logs_nothing_and_exits_zero() {
        let dir = check_project("log-empty");
        assert_eq!(log_in(&dir, &[]).map(|o| o.text).unwrap_or_default(), "");
        assert_eq!(perform(Action::Log(dash_c(&dir)), &env()), 0);
    }

    /// A recent extraction is inside a wide window and outside a narrow
    /// one. The clock is real here, so the assertion is about which side
    /// of the floor `now` falls -- not about a fixed timestamp.
    #[test]
    fn since_bounds_the_window() {
        let dir = check_project("log-since");
        let (id, _) = extracted(&dir);
        let wide = log_in(&dir, &["--since", "2w"])
            .map(|o| o.text)
            .unwrap_or_default();
        assert!(wide.contains(&id), "{wide}");
    }

    #[test]
    fn a_since_without_a_unit_is_a_usage_error_that_names_the_forms() {
        let dir = check_project("log-bad-since");
        let said = log_in(&dir, &["--since", "7"]).err().unwrap_or_default();
        assert!(said.contains("7d"), "{said}");
        assert_eq!(
            perform(Action::Log(args(&["--since", "7"])), &env()),
            USAGE_EXIT
        );
    }

    #[test]
    fn log_json_is_parseable_and_carries_the_fire_count() {
        let dir = check_project("log-json");
        let _ = extracted(&dir);
        let text = log_in(&dir, &["--format", "json"])
            .map(|o| o.text)
            .unwrap_or_default();
        let parsed: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_default();
        let fires = parsed
            .get("entries")
            .and_then(|entries| entries.get(0))
            .and_then(|first| first.get("fires"));
        assert_eq!(fires, Some(&serde_json::json!(0)), "{text}");
    }

    #[test]
    fn an_unknown_log_flag_is_an_error() {
        assert!(parse_log(&args(&["--nope"])).is_err());
        assert!(parse_log(&args(&["--since"])).is_err());
        assert!(parse_log(&args(&["abc"])).is_err());
    }

    /// A reverted extraction leaves the ledger, so it leaves the log. The
    /// log is the record of what is extracted NOW, not a history of
    /// everything that ever was.
    #[test]
    fn a_reverted_extraction_leaves_the_log() {
        let dir = check_project("log-reverted");
        let (id, _) = extracted(&dir);
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        assert_eq!(log_in(&dir, &[]).map(|o| o.text).unwrap_or_default(), "");
    }
}
