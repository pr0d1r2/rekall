use super::{Env, Format, Output, need, parse_format};
use crate::{ledger, recall, trigger};
use std::path::{Path, PathBuf};
/// `recall` takes the SITUATION: a free-text description plus whatever
/// exact facts the caller has.
#[derive(Debug, Default)]
pub struct RecallArgs {
    pub text: String,
    pub tool: Option<String>,
    pub path: Option<String>,
    /// Where the WORK is. Section I gives this verb a `--cwd` distinct
    /// from every other verb's `-C`, which says where the PROJECT is --
    /// only the first is a fact a trigger can turn on.
    pub cwd: Option<String>,
    pub base: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_recall(args: &[String]) -> Result<RecallArgs, String> {
    let mut out = RecallArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_recall_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_recall_arg<'a>(
    out: &mut RecallArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--tool" => out.tool = Some(need(arg, rest)?),
        "--path" => out.path = Some(need(arg, rest)?),
        "--cwd" => out.cwd = Some(need(arg, rest)?),
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json;
        }
        "-C" => out.base = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        // Every remaining positional joins the situation TEXT. A shell
        // splits an unquoted description into words, and refusing the
        // second one would make `rekall recall editing a test` a usage
        // error for no reason a caller could guess.
        word => out.text = join(&out.text, word),
    }
    Ok(())
}

fn join(held: &str, word: &str) -> String {
    if held.is_empty() {
        return word.to_string();
    }
    format!("{held} {word}")
}

/// Run `recall` end to end. Report-only, and it counts NO firing -- V11's
/// counter must mean the artifact was loaded, not asked about.
pub fn recall_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_recall(flags)?;
    let base = args.base.clone().unwrap_or_else(|| env.cwd.clone());
    let held =
        ledger::load(&ledger::path_in(&base)).map_err(|e| e.to_string())?;
    let texts = read_artifacts(&base, &held);
    let candidates = zip_candidates(&held, &texts);
    let report = recall::decide(&candidates, &situation(&args));
    render_recall(&report, args.json)
}

fn situation(args: &RecallArgs) -> trigger::Situation {
    trigger::Situation {
        tool: args.tool.clone(),
        path: args.path.clone(),
        cwd: args.cwd.clone(),
        text: args.text.clone(),
    }
}

pub(super) fn read_artifacts(
    base: &Path,
    held: &ledger::Ledger,
) -> Vec<Option<String>> {
    held.extracted
        .iter()
        .map(|row| std::fs::read_to_string(base.join(&row.artifact)).ok())
        .collect()
}

pub(super) fn zip_candidates<'a>(
    held: &'a ledger::Ledger,
    texts: &'a [Option<String>],
) -> Vec<recall::Candidate<'a>> {
    held.extracted
        .iter()
        .zip(texts)
        .map(|(row, text)| recall::Candidate {
            row,
            artifact: text.as_deref(),
        })
        .collect()
}

fn render_recall(
    report: &recall::Report,
    json: bool,
) -> Result<Output, String> {
    let text = if json {
        serde_json::to_string_pretty(report)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string())?
    } else {
        recall::render_human(report)
    };
    Ok(Output {
        text,
        warnings: Vec::new(),
    })
}
