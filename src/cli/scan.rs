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
        other => return positional(out, other),
    }
    Ok(())
}

/// Anything that is not a flag is a PATH, and anything that looks like a
/// flag and is not one is still an error.
///
/// Told apart by the leading dash rather than by guessing: before this,
/// every non-flag argument was reported as `unknown flag`, which called a
/// path a flag in the one message a reader had to work from.
fn positional(out: &mut ScanArgs, arg: &str) -> Result<(), String> {
    match arg {
        other if other.starts_with('-') => {
            Err(format!("unknown flag `{other}`"))
        }
        // Section I's `scan [<path>...]`. A positional NARROWS the corpus
        // the config already names; it never widens it.
        path => {
            out.filter.paths.push(path.to_string());
            Ok(())
        }
    }
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
    unreached(&args, &outcome)?;
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

/// A named path the corpus never reached is a TYPO, not an empty result.
///
/// `--class M` matching nothing is a fact about the corpus. A path
/// matching nothing is a fact about the argument: the file is outside the
/// configured roots, or it is spelled wrong, and printing an empty table
/// would report either as "clean" (V26).
fn unreached(args: &ScanArgs, outcome: &scan::Outcome) -> Result<(), String> {
    let seen: Vec<&str> = outcome.report.rows.iter().map(file_of).collect();
    let missed: Vec<&str> = args
        .filter
        .paths
        .iter()
        .filter(|want| !covers(&seen, want))
        .map(String::as_str)
        .collect();
    if missed.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{NO_SUCH_PATH}`{}`. {SO_LOOK}",
        missed.join("`, `")
    ))
}

const NO_SUCH_PATH: &str = "no corpus file matches ";
const SO_LOOK: &str = "Check the spelling, or run `rekall scan --sources` \
                       to see what the configured roots actually reach";

/// The `file` half of a `file:line-line` row.
fn file_of(row: &scan::Row) -> &str {
    row.src
        .rsplit_once(':')
        .map_or(row.src.as_str(), |(f, _)| f)
}

/// A directory covers everything beneath it; a file covers itself.
fn covers(seen: &[&str], want: &str) -> bool {
    let want = want.trim_end_matches('/');
    seen.iter()
        .any(|f| *f == want || f.starts_with(&format!("{want}/")))
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::cli::testing::*;
    use crate::cli::{Action, Output, perform};

    use std::path::Path;

    fn run_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags =
            vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        flags.extend(extra.iter().map(|item| (*item).to_string()));
        scan_command(&flags, &env())
    }

    #[test]
    fn a_configured_project_scans_end_to_end() {
        let dir = project("end-to-end");
        let output = run_in(&dir, &[]).ok();
        assert!(
            output.as_ref().is_some_and(|out| out.text.contains("M1")),
            "output was {output:?}"
        );
        assert_eq!(output.map(|out| out.warnings.len()), Some(0));
    }

    /// Paths in the report are relative to the project, not to wherever the
    /// process happened to start. This is what keeps ids findable later.
    #[test]
    fn output_paths_are_project_relative() {
        let dir = project("relative");
        let text = text_of(&dir, &[]);
        assert!(text.contains("CLAUDE.md:1-1"), "text was {text}");
        assert!(
            !text.contains("target/test-project"),
            "the absolute temp path leaked into the report"
        );
    }

    #[test]
    fn json_output_parses_and_matches_the_human_row_count() {
        let dir = project("json");
        let human_rows = text_of(&dir, &[]).lines().count();
        assert_eq!(json_rows(&dir), human_rows);
        assert!(human_rows > 0, "the fixture must produce rows");
    }

    /// The rendered text, or an empty string if the scan failed. A failed
    /// scan then shows up as a failed ASSERTION on content rather than as
    /// a panic in a helper -- which keeps every test path executable.
    fn text_of(dir: &Path, extra: &[&str]) -> String {
        run_in(dir, extra).map(|out| out.text).unwrap_or_default()
    }

    fn json_rows(dir: &Path) -> usize {
        let text = text_of(dir, &["--format", "json"]);
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .as_ref()
            .and_then(|parsed| parsed.get("rows"))
            .and_then(|rows| rows.as_array())
            .map_or(0, std::vec::Vec::len)
    }

    #[test]
    fn the_sources_flag_adds_a_source_line() {
        let dir = project("sources");
        let plain = text_of(&dir, &[]);
        let with_sources = text_of(&dir, &["--sources"]);
        assert!(with_sources.lines().count() > plain.lines().count());
        assert!(with_sources.contains("project"));
    }

    #[test]
    fn filters_reach_through_the_command() {
        let dir = project("filter");
        let text = text_of(&dir, &["--class", "S"]);
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("S2"), "text was {text}");
    }

    #[test]
    fn an_unreadable_corpus_file_becomes_a_named_warning() {
        let dir = project("warned");
        let _ = std::fs::write(dir.join("bad.md"), [0xff_u8, 0xfe]);
        let warnings = run_in(&dir, &[])
            .map(|out| out.warnings)
            .unwrap_or_default();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings.first().is_some_and(|w| w.contains("bad.md")),
            "{warnings:?}"
        );
    }

    /// A broken config is an error, not an empty scan. Treating it as "no
    /// config" would scan a corpus the user never described.
    #[test]
    fn a_broken_project_config_stops_the_scan() {
        let dir = project("broken");
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = \"not a list\"\n",
        );
        assert!(run_in(&dir, &[]).is_err());
    }

    /// The binary's SUCCESS path: a real project, printed, exit 0. This is
    /// the arm every ordinary invocation takes, and it was the last part
    /// of the command path never executed by the suite.
    #[test]
    fn a_successful_scan_exits_zero() {
        let dir = project("exit-zero");
        let flags = vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        assert_eq!(perform(Action::Scan(flags), &env()), 0);
    }

    /// A corpus with an unreadable file still SUCCEEDS -- the warning is
    /// printed, the readable statements are reported, and the exit code
    /// stays 0. A partial corpus is not a failed run, but it must not be
    /// a silent one either.
    #[test]
    fn a_scan_with_warnings_still_exits_zero() {
        let dir = project("exit-zero-warned");
        let _ = std::fs::write(dir.join("bad.md"), [0xff_u8, 0xfe]);
        let flags = vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        assert_eq!(perform(Action::Scan(flags), &env()), 0);
    }

    /// Section I's `scan [<path>...]`, which the parser rejected as an
    /// unknown flag until now. A positional narrows the corpus the config
    /// already names; it never widens it.
    fn two_file_project(name: &str) -> PathBuf {
        let dir = project(name);
        let _ = std::fs::create_dir_all(dir.join("docs"));
        let _ = std::fs::write(
            dir.join("docs").join("AGENTS.md"),
            "- always run the linter before pushing\n",
        );
        dir
    }

    #[test]
    fn a_path_narrows_the_scan_to_that_file() {
        let dir = two_file_project("scan-one-path");
        let all = run_in(&dir, &[]).map(|o| o.text).unwrap_or_default();
        let one = run_in(&dir, &["CLAUDE.md"])
            .map(|o| o.text)
            .unwrap_or_default();
        assert!(all.contains("docs/AGENTS.md"), "{all}");
        assert!(!one.contains("docs/AGENTS.md"), "{one}");
        assert!(one.contains("CLAUDE.md"), "{one}");
    }

    /// A directory covers everything beneath it, which is what anybody
    /// typing a path at a shell means by it.
    #[test]
    fn a_directory_narrows_to_everything_under_it() {
        let dir = two_file_project("scan-dir-path");
        let text = run_in(&dir, &["docs"]).map(|o| o.text).unwrap_or_default();
        assert!(text.contains("docs/AGENTS.md"), "{text}");
        assert!(!text.contains("CLAUDE.md:"), "{text}");
    }

    /// A path the corpus never reached is a TYPO, not an empty result --
    /// printing an empty table would report a misspelling as "clean".
    #[test]
    fn a_path_outside_the_corpus_is_an_error_not_an_empty_table() {
        let dir = two_file_project("scan-no-such-path");
        let why = run_in(&dir, &["docs/NOPE.md"]).err().unwrap_or_default();
        assert!(why.contains("no corpus file matches"), "{why}");
        assert!(why.contains("--sources"), "{why}");
    }

    /// Something that looks like a flag and is not one stays an error. The
    /// message called a PATH a flag before this; it must not now call a
    /// flag a path.
    #[test]
    fn an_unknown_flag_is_still_an_unknown_flag() {
        let dir = two_file_project("scan-still-flags");
        let why = run_in(&dir, &["--nope"]).err().unwrap_or_default();
        assert!(why.contains("unknown flag"), "{why}");
    }

    /// Paths compose with the other filters rather than replacing them.
    #[test]
    fn a_path_and_a_class_filter_both_apply() {
        let dir = two_file_project("scan-path-and-class");
        let text = run_in(&dir, &["CLAUDE.md", "--class", "S"])
            .map(|o| o.text)
            .unwrap_or_default();
        assert!(!text.contains("docs/AGENTS.md"), "{text}");
        assert!(
            text.lines().all(|l| l.is_empty() || l.contains("  S")),
            "{text}"
        );
    }
}
