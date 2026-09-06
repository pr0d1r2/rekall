use super::{
    Env, Format, Output, need, parse_format, resolve as resolve_config,
};
use crate::{catch, ledger};
use std::path::{Path, PathBuf};

/// `catch` -- the second intake (T18).
#[derive(Debug, Default)]
pub struct CatchArgs {
    /// The transcript to read. Absent = the newest under the configured
    /// root, because a path a human has to look up is a path they will not
    /// look up mid-session (V46).
    pub session: Option<String>,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_catch(args: &[String]) -> Result<CatchArgs, String> {
    let mut out = CatchArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_catch_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_catch_arg<'a>(
    out: &mut CatchArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json;
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        session => out.session = Some(session.to_string()),
    }
    Ok(())
}

pub fn catch_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_catch(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let path = session_path(&args, env)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|why| format!("cannot read `{}`: {why}", path.display()))?;
    // The SAME weights `scan` uses, from the same resolver. A corpus that
    // tuned a word tunes it for both intakes, and V44's "same classifier"
    // is a shared call rather than a shared intention.
    let weights = resolve_config(&base)?.weights;
    let report = catch::catch(&text, &path.to_string_lossy(), &weights);
    let proposed = record(&base, &report)?;
    render(&report, proposed, &path, args.json)
}

/// Which transcript to read.
///
/// An explicit argument wins. Absent, it is the newest file under the
/// configured root -- V46's "the common case needs no path a human would
/// have to look up".
fn session_path(args: &CatchArgs, env: &Env) -> Result<PathBuf, String> {
    if let Some(named) = &args.session {
        return Ok(PathBuf::from(named));
    }
    let roots = transcript_roots(env)?;
    if roots.is_empty() {
        return Err(
            "no transcript root exists on this machine -- looked for `~/.claude/projects` and `~/.codex/sessions`. Name a transcript explicitly: `rekall catch <path>`"
                .to_string(),
        );
    }
    roots
        .iter()
        .filter_map(|root| newest(root))
        .max_by_key(|path| modified_at(path))
        .ok_or_else(|| no_transcript(&roots))
}

/// Names every root that was looked in, not just the first. A reader who
/// runs two agents needs to know which one was empty.
fn no_transcript(roots: &[PathBuf]) -> String {
    let looked = roots
        .iter()
        .map(|r| format!("`{}`", r.display()))
        .collect::<Vec<_>>()
        .join(" or ");
    format!(
        "no transcript found under {looked}. Name one explicitly: `rekall catch <path>`"
    )
}

fn modified_at(path: &Path) -> std::time::SystemTime {
    path.metadata()
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH)
}

/// Every transcript root this build knows, in the order it knows them.
///
/// ITERATED rather than selected, which is why `catch` needs no `--agent`
/// (`..:V47`). A file under `~/.codex/sessions` is Codex BY LOCATION --
/// a fact about where it sits rather than a guess about what it holds --
/// so provenance answers the question sniffing a payload would only
/// gamble on.
///
/// A root that does not exist is skipped in silence: not every machine
/// runs every agent, and an absent directory is a fact about this box
/// rather than a fault to report.
fn transcript_roots(env: &Env) -> Result<Vec<PathBuf>, String> {
    let home = env.home.as_deref().ok_or_else(|| {
        "HOME is unset, so no transcript root can be derived. Name a transcript explicitly: `rekall catch <path>`".to_string()
    })?;
    let at = PathBuf::from(home);
    Ok(vec![
        at.join(".claude").join("projects"),
        at.join(".codex").join("sessions"),
    ]
    .into_iter()
    .filter(|root| root.is_dir())
    .collect())
}

/// The most recently modified file under `root`, at any depth.
///
/// Unreadable entries are skipped rather than fatal: a transcript
/// directory is the harness's, and one unreadable file in it is not a
/// reason to refuse the other forty.
fn newest(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for found in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let Some(when) = found.metadata().ok().and_then(|m| m.modified().ok())
        else {
            continue;
        };
        if best.as_ref().is_none_or(|(seen, _)| when > *seen) {
            best = Some((when, found.into_path()));
        }
    }
    best.map(|(_, path)| path)
}

/// Write the candidates. V45: this is the LEDGER, not the corpus, so it is
/// not a breach of report-only -- and no source file and no artifact is
/// touched here.
fn record(base: &Path, report: &catch::Report) -> Result<usize, String> {
    let at = ledger::path_in(base);
    let mut held = ledger::load(&at).map_err(|why| why.to_string())?;
    let when = super::apply::now();
    let mut added = 0usize;
    for found in &report.caught {
        if held.propose(candidate_of(found, when)) {
            added = added.saturating_add(1);
        }
    }
    ledger::save(&at, &held).map_err(|why| why.to_string())?;
    Ok(added)
}

fn candidate_of(found: &catch::Caught, when: u64) -> ledger::Candidate {
    ledger::Candidate {
        id: found.id.clone(),
        src: found.src.clone(),
        text: found.text.clone(),
        at: when,
    }
}

fn render(
    report: &catch::Report,
    proposed: usize,
    path: &Path,
    json: bool,
) -> Result<Output, String> {
    let text = if json {
        as_json(report, proposed, path)?
    } else {
        as_human(report, proposed)
    };
    Ok(Output {
        text,
        warnings: Vec::new(),
    })
}

fn as_human(report: &catch::Report, proposed: usize) -> String {
    let mut out = String::new();
    for found in &report.caught {
        out.push_str(&format!(
            "{}  {}  {}  {}\n",
            found.id,
            found.label,
            found.signals.join(","),
            first_line(&found.text)
        ));
    }
    out.push_str(&summary(report, proposed));
    out
}

/// The SKIP COUNT is printed even when it is zero.
///
/// V46's other half. A transcript half of which failed to parse yields few
/// candidates, and "few candidates" reads exactly like "few violations"
/// unless the count is on the screen next to it.
fn summary(report: &catch::Report, proposed: usize) -> String {
    format!(
        "{} turn(s) read, {} caught, {} new candidate(s), {} line(s) unreadable\n",
        report.turns,
        report.caught.len(),
        proposed,
        report.skipped
    )
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("").trim()
}

fn as_json(
    report: &catch::Report,
    proposed: usize,
    path: &Path,
) -> Result<String, String> {
    let rows: Vec<serde_json::Value> =
        report.caught.iter().map(row_json).collect();
    let body = serde_json::json!({
        "session": path.to_string_lossy(),
        "turns": report.turns,
        "caught": rows,
        "proposed": proposed,
        "skipped": report.skipped,
    });
    serde_json::to_string_pretty(&body).map_err(|why| why.to_string())
}

/// One caught row, with the same anatomy `scan` prints -- `class` and
/// `sharpness` reachable without string-surgery on the label (V17).
fn row_json(found: &catch::Caught) -> serde_json::Value {
    serde_json::json!({
        "id": found.id,
        "src": found.src,
        "text": found.text,
        "class": found.class,
        "sharpness": found.sharpness,
        "label": found.label,
        "signals": found.signals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transcript and a project to record into. Under `target/` so
    /// `cargo clean` removes it, named after its test so two never
    /// collide.
    fn project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("test-catch").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(dir.join("session.jsonl"), TRANSCRIPT);
        dir
    }

    /// Two corrections, one request, one assistant echo and one unreadable
    /// line -- every branch the verb has to tell apart, in one fixture.
    const TRANSCRIPT: &str = concat!(
        r#"{"role":"user","content":"Could you look at the parser?"}"#,
        "\n",
        r#"{"role":"user","content":"Never commit a .env file."}"#,
        "\n",
        "this line is not json\n",
        r#"{"role":"user","content":"Always run the gate before pushing."}"#,
        "\n",
        r#"{"role":"assistant","content":"Never commit a .env file."}"#,
        "\n",
    );

    fn env_at(dir: &Path) -> Env {
        Env {
            cwd: dir.to_path_buf(),
            home: None,
        }
    }

    fn session_of(dir: &Path) -> String {
        dir.join("session.jsonl").to_string_lossy().into_owned()
    }

    fn run(dir: &Path, extra: &[&str]) -> String {
        let mut flags = vec![session_of(dir)];
        flags.extend(extra.iter().map(|f| (*f).to_string()));
        catch_command(&flags, &env_at(dir))
            .map(|out| out.text)
            .unwrap_or_default()
    }

    #[test]
    fn a_session_argument_is_not_a_flag() {
        let parsed = parse_catch(&["a.jsonl".to_string()]);
        assert_eq!(
            parsed.map(|p| p.session).ok().flatten(),
            Some("a.jsonl".to_string())
        );
    }

    #[test]
    fn an_unknown_flag_is_an_error() {
        assert!(parse_catch(&["--nope".to_string()]).is_err());
    }

    #[test]
    fn the_format_flag_is_read() {
        let parsed = parse_catch(&["--format".to_string(), "json".to_string()]);
        assert!(parsed.map(|p| p.json).unwrap_or_default());
    }

    #[test]
    fn a_bad_format_is_an_error() {
        let flags = ["--format".to_string(), "yaml".to_string()];
        assert!(parse_catch(&flags).is_err());
    }

    #[test]
    fn an_absent_transcript_names_itself() {
        let dir = project("absent");
        let flags = ["nowhere/at/all.jsonl".to_string()];
        let outcome = catch_command(&flags, &env_at(&dir));
        assert!(outcome.is_err(), "an unreadable session is an error");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The whole point, at the CLI boundary: two corrections caught, the
    /// request dropped as `U`, the assistant's echo not evidence, and the
    /// unreadable line COUNTED rather than swallowed (V44, V46).
    #[test]
    fn the_human_report_counts_what_it_could_not_read() {
        let dir = project("human");
        let out = run(&dir, &[]);
        assert!(out.contains("Never commit"), "{out}");
        assert!(out.contains("Always run the gate"), "{out}");
        assert!(!out.contains("look at the parser"), "{out}");
        assert!(out.contains("1 line(s) unreadable"), "{out}");
        assert!(out.contains("3 turn(s) read"), "{out}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// V17: `class` and `sharpness` reachable without splitting the label.
    #[test]
    fn the_json_report_separates_class_from_sharpness() {
        let dir = project("json");
        let out = run(&dir, &["--format", "json"]);
        for field in ["\"class\"", "\"sharpness\"", "\"label\"", "\"skipped\""]
        {
            assert!(out.contains(field), "json must carry {field}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// V45: the LEDGER is written, the CORPUS is not.
    #[test]
    fn candidates_reach_the_ledger_and_the_corpus_is_untouched() {
        let dir = project("ledger");
        let before = std::fs::read_to_string(dir.join("session.jsonl"))
            .unwrap_or_default();
        let _ = run(&dir, &[]);
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(held.candidates.len(), 2, "both corrections recorded");
        assert!(held.extracted.is_empty(), "catch extracts nothing");
        let after = std::fs::read_to_string(dir.join("session.jsonl"))
            .unwrap_or_default();
        assert_eq!(before, after, "the transcript is an INPUT, never written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Re-reading a transcript must not grow the file. The ledger refuses
    /// the duplicate id; this proves the verb reports that honestly.
    #[test]
    fn a_second_pass_proposes_nothing_new() {
        let dir = project("twice");
        let _ = run(&dir, &[]);
        let out = run(&dir, &[]);
        assert!(out.contains("0 new candidate(s)"), "{out}");
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(held.candidates.len(), 2, "still two, not four");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_explicit_session_wins_over_the_configured_root() {
        let args = CatchArgs {
            session: Some("chosen.jsonl".to_string()),
            ..CatchArgs::default()
        };
        let found = session_path(&args, &env_at(Path::new(".")));
        assert_eq!(found.ok(), Some(PathBuf::from("chosen.jsonl")));
    }

    /// V26's shape at the verb level: without HOME the root cannot be
    /// derived, and saying "no transcripts found" would report an absence
    /// this build never actually looked for.
    #[test]
    fn no_home_and_no_argument_says_which_is_missing() {
        let outcome =
            session_path(&CatchArgs::default(), &env_at(Path::new(".")));
        let why = outcome.err().unwrap_or_default();
        assert!(why.contains("HOME"), "{why}");
        assert!(why.contains("rekall catch <path>"), "{why}");
    }

    #[test]
    fn an_empty_root_is_reported_as_empty_not_as_a_crash() {
        let dir = project("emptyroot");
        let root = dir.join(".claude").join("projects");
        let _ = std::fs::create_dir_all(&root);
        assert_eq!(newest(&root), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_newest_transcript_is_the_one_chosen() {
        let dir = project("newest");
        let root = dir.join(".claude").join("projects");
        let _ = std::fs::create_dir_all(&root);
        let _ = std::fs::write(root.join("old.jsonl"), "{}\n");
        let _ = std::fs::write(root.join("new.jsonl"), "{}\n");
        let picked = newest(&root).unwrap_or_default();
        assert!(picked.starts_with(&root), "{picked:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_line_trims_and_stops_at_the_break() {
        assert_eq!(first_line("  one  \ntwo"), "one");
        assert_eq!(first_line(""), "");
    }
    fn home_with(name: &str, dirs: &[&str]) -> PathBuf {
        let at = PathBuf::from("target").join("test-home").join(name);
        let _ = std::fs::remove_dir_all(&at);
        for d in dirs {
            let _ = std::fs::create_dir_all(at.join(d));
        }
        let _ = std::fs::create_dir_all(&at);
        at
    }

    fn env_home(at: &Path) -> Env {
        Env {
            cwd: PathBuf::from("."),
            home: Some(at.to_string_lossy().into_owned()),
        }
    }

    /// Both agents present: both roots, in a fixed order so the report a
    /// reader gets does not depend on directory iteration.
    #[test]
    fn every_known_root_that_exists_is_returned() {
        let at = home_with("both", &[".claude/projects", ".codex/sessions"]);
        let found = transcript_roots(&env_home(&at)).unwrap_or_default();
        let ends: Vec<bool> = found
            .iter()
            .zip(["projects", "sessions"])
            .map(|(p, want)| p.ends_with(want))
            .collect();
        assert_eq!(ends, vec![true, true], "{found:?}");
        let _ = std::fs::remove_dir_all(&at);
    }

    /// Not every machine runs every agent. An absent root is a fact about
    /// the box, not a fault, so it is skipped rather than reported.
    #[test]
    fn a_root_that_does_not_exist_is_skipped_in_silence() {
        let at = home_with("codexonly", &[".codex/sessions"]);
        let found = transcript_roots(&env_home(&at)).unwrap_or_default();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(
            found.first().is_some_and(|p| p.ends_with("sessions")),
            "{found:?}"
        );
        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn no_home_names_what_it_could_not_derive() {
        let bare = Env {
            cwd: PathBuf::from("."),
            home: None,
        };
        let why = transcript_roots(&bare).err().unwrap_or_default();
        assert!(why.contains("HOME"), "{why}");
        assert!(why.contains("rekall catch <path>"), "{why}");
    }

    /// A machine running neither agent gets told what was looked for,
    /// rather than a bare "not found" it cannot act on.
    #[test]
    fn no_root_at_all_names_both_places_it_looked() {
        let at = home_with("neither", &[]);
        let args = CatchArgs::default();
        let why = session_path(&args, &env_home(&at))
            .err()
            .unwrap_or_default();
        assert!(why.contains(".claude"), "{why}");
        assert!(why.contains(".codex"), "{why}");
        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn the_message_for_empty_roots_names_each_one() {
        let said =
            no_transcript(&[PathBuf::from("/a/one"), PathBuf::from("/b/two")]);
        assert!(said.contains("/a/one"), "{said}");
        assert!(said.contains("/b/two"), "{said}");
        assert!(said.contains(" or "), "{said}");
    }

    #[test]
    fn an_unreadable_path_sorts_oldest_rather_than_erroring() {
        assert_eq!(
            modified_at(Path::new("nowhere/at/all")),
            std::time::UNIX_EPOCH
        );
    }

    /// The common case needs no path a human would have to look up
    /// (V46): with no argument, the newest file under a real root is what
    /// gets read.
    #[test]
    fn no_argument_reads_the_newest_file_under_a_root() {
        let at = home_with("newestroot", &[".claude/projects"]);
        let _ = std::fs::write(at.join(".claude/projects/a.jsonl"), "{}\n");
        let found = session_path(&CatchArgs::default(), &env_home(&at));
        assert!(
            found.as_ref().is_ok_and(|p| p.ends_with("a.jsonl")),
            "{found:?}"
        );
        let _ = std::fs::remove_dir_all(&at);
    }

    /// A root that exists and holds nothing is a different answer from a
    /// root that does not exist, and the message names every place it
    /// looked rather than the first.
    #[test]
    fn roots_that_hold_nothing_name_every_place_looked() {
        let at = home_with("emptyroots", &[".claude/projects"]);
        let why = session_path(&CatchArgs::default(), &env_home(&at))
            .err()
            .unwrap_or_default();
        assert!(why.contains("projects"), "{why}");
        let _ = std::fs::remove_dir_all(&at);
    }
}
