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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check;
    use crate::cli::scan::scan_command;
    use crate::cli::testing::*;
    use crate::cli::{Action, perform};
    use crate::ledger;

    use std::path::Path;

    /// V22, end to end: a `[signals]` table nothing read would be a wish.
    /// This drives it from the CONFIG FILE through `scan` to a verdict --
    /// the same statement is `U` with the shipped defaults and `M` once
    /// the corpus names its own vocabulary.
    #[test]
    fn a_configured_weight_reaches_the_verdict() {
        let dir = check_project("weights");
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- release notes ship beside the tag\n",
        );
        assert!(
            scan_row(&dir).contains("  U  "),
            "the defaults should not classify this: {}",
            scan_row(&dir)
        );
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n\n[signals.weight]\n\"ship beside\" = 2\n",
        );
        assert!(scan_row(&dir).contains("  M"), "{}", scan_row(&dir));
    }

    /// The DEADBAND, from the same file. A corpus can ask for more
    /// evidence before it accepts a verdict.
    #[test]
    fn a_configured_deadband_withholds_a_weak_verdict() {
        let dir = check_project("deadband");
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- when editing `.rs`, never use unwrap\n",
        );
        assert!(scan_row(&dir).contains("  M"), "{}", scan_row(&dir));
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n\n[signals]\ndeadband = 1\n",
        );
        assert!(scan_row(&dir).contains("  U  "), "{}", scan_row(&dir));
    }

    fn scan_row(dir: &Path) -> String {
        scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }
    /// B4, at the level it was found: `check` and `recall` must agree
    /// about the same artifact. A generated `M` rule has no trigger block,
    /// `check` is silent about that (V37 makes it gate-only), and `recall`
    /// used to call it broken.
    #[test]
    fn check_and_recall_agree_about_a_generated_rule() {
        let dir = one_extracted_rule("agree-m");
        let drift = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(
            !drift.contains(check::BAD_TRIGGER_BLOCK)
                && !drift.contains(check::NO_TRIGGER),
            "check called the trigger broken: {drift}"
        );
        let said = recall_in(&dir, &["--tool", "Edit", "--path", "a.rs"]);
        assert!(said.contains("gates at commit only"), "{said}");
        assert!(!said.contains("could not be read"), "{said}");
    }

    /// V3's reload rule, answered end to end: an extracted skill with a
    /// matching trigger LOADS here.
    #[test]
    fn a_matching_skill_loads() {
        let dir = recall_project("hit", "path = [\"**/*.rs\"]", "");
        let out = recall_in(&dir, &["--path", "src/main.rs"]);
        assert!(out.starts_with("load"), "{out}");
    }

    #[test]
    fn a_skill_whose_trigger_misses_is_reported_as_skipped() {
        let dir = recall_project("miss", "path = [\"**/*.py\"]", "");
        let out = recall_in(&dir, &["--path", "src/main.rs"]);
        assert!(out.starts_with("skip"), "{out}");
    }

    /// V29 through the verb: the refusal beats the match, and says so.
    #[test]
    fn the_do_not_fire_block_wins_and_is_named() {
        let dir = recall_project(
            "refuse",
            "path = [\"**/*.rs\"]",
            "path = [\"src/**\"]",
        );
        let out = recall_in(&dir, &["--path", "src/main.rs"]);
        assert!(out.starts_with("skip"), "{out}");
        assert!(out.contains("WINS"), "{out}");
    }

    #[test]
    fn the_tool_flag_reaches_the_matcher() {
        let dir = recall_project("tool", "tool = [\"Edit\"]", "");
        assert!(recall_in(&dir, &["--tool", "Edit"]).starts_with("load"));
        assert!(recall_in(&dir, &["--tool", "Bash"]).starts_with("skip"));
    }

    /// `--cwd` is the situation's directory, and it stands in for the path
    /// when no file is named -- which is the state someone is in when they
    /// ask what loads here before touching anything.
    #[test]
    fn the_cwd_flag_stands_in_for_an_unnamed_path() {
        let dir = recall_project("cwd", "path = [\"**/backend/**\"]", "");
        assert!(
            recall_in(&dir, &["--cwd", "srv/backend/api"]).starts_with("load")
        );
        assert!(recall_in(&dir, &["--cwd", "srv/web"]).starts_with("skip"));
    }

    /// A situation typed as bare words is ONE description, not a usage
    /// error. A shell splits it and refusing the second word would fail
    /// for a reason nobody could guess.
    #[test]
    fn a_multi_word_situation_is_joined_into_one_text() {
        let held = parse_recall(&args(&["editing", "a", "test"]));
        assert_eq!(held.map(|a| a.text).unwrap_or_default(), "editing a test");
    }

    #[test]
    fn a_word_trigger_matches_the_situation_text() {
        let dir = recall_project("word", "word = [\"clippy\"]", "");
        assert!(recall_in(&dir, &["run", "clippy", "now"]).starts_with("load"));
        assert!(recall_in(&dir, &["write", "docs"]).starts_with("skip"));
    }

    /// V7: report-only. Asking what loads must NOT count a firing -- V11's
    /// counter has to mean the artifact was loaded, or `--dead` measures
    /// curiosity instead of use.
    #[test]
    fn recall_counts_no_firing() {
        let dir = recall_project("no-fire", "path = [\"**/*.rs\"]", "");
        let _ = recall_in(&dir, &["--path", "src/main.rs"]);
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(held.extracted.first().map(|row| row.fires), Some(0));
    }

    #[test]
    fn recall_json_is_parseable_and_carries_the_verdict() {
        let dir = recall_project("json", "path = [\"**/*.rs\"]", "");
        let text =
            recall_in(&dir, &["--path", "src/main.rs", "--format", "json"]);
        let parsed: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_default();
        let loads = parsed
            .get("rows")
            .and_then(|rows| rows.get(0))
            .and_then(|first| first.get("loads"));
        assert_eq!(loads, Some(&serde_json::json!(true)), "{text}");
    }

    #[test]
    fn an_empty_ledger_recalls_nothing_and_exits_zero() {
        let dir = check_project("recall-empty");
        assert_eq!(recall_in(&dir, &["anything"]), "");
        assert_eq!(perform(Action::Recall(dash_c(&dir)), &env()), 0);
    }

    #[test]
    fn an_unknown_recall_flag_is_an_error() {
        assert!(parse_recall(&args(&["--nope"])).is_err());
        assert!(parse_recall(&args(&["--tool"])).is_err());
    }
}
