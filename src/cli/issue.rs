use super::apply::{approved, consent_for, publish_artifact};
use super::{Env, Format, Output, need, parse_format, report};
use crate::{apply, issue, ledger};
use std::path::{Path, PathBuf};

/// `issue` takes ids, or `--all`, plus where to put the copy.
///
/// MANY ids, unlike `revert`. Reverting is undoing one decision and a
/// batch of them needs an answer to what happens when the third of five
/// fails. Issuing is the opposite shape: a registry adopts a SET, and
/// handing it one skill per invocation is the ceremony that stops people
/// doing it at all.
#[derive(Debug, Default)]
pub struct IssueArgs {
    pub ids: Vec<String>,
    pub all: bool,
    pub to: Option<PathBuf>,
    pub retire: bool,
    pub auto_approve: bool,
    pub cwd: Option<PathBuf>,
    /// Held as the parsed VALUE and not as a `json` flag, so this struct
    /// stays under the four-bool limit that catches exactly this: a
    /// parameter list where every argument is `true` or `false` and the
    /// call site says which is which by counting.
    pub format: Format,
}

pub fn parse_issue(args: &[String]) -> Result<IssueArgs, String> {
    let mut out = IssueArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_issue_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_issue_arg<'a>(
    out: &mut IssueArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--all" => out.all = true,
        "--retire" => out.retire = true,
        "--auto-approve" => out.auto_approve = true,
        "--to" => out.to = Some(PathBuf::from(need(arg, rest)?)),
        "--format" => {
            out.format = parse_format(&need(arg, rest)?)?;
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        id => out.ids.push(id.to_string()),
    }
    Ok(())
}

pub const NO_ISSUE_INPUT: &str = "`issue` needs an id, or `--all` for every extracted skill -- \
     copy one from `rekall log`";

/// Run `issue` end to end.
///
/// Same order as `apply` and `revert`, for the same reasons: choose the
/// rows, compute what would happen, NAME every file (V7), ask (V20), and
/// only then write. This verb writes OUTSIDE the repository and, on the
/// retire path, deletes the local copy of a rule -- both are the kind of
/// act V20 wants somebody to have said yes to.
pub fn issue_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_issue(flags)?;
    if args.ids.is_empty() && !args.all {
        return Err(NO_ISSUE_INPUT.to_string());
    }
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let mut store = Store::open(base)?;
    let rows = chosen(&store.ledger, &args)?;
    let planned = preview(&rows, &store.base, &args);
    ask_then_write(&planned, &rows, &args, &mut store)
}

/// The ledger, where it lives, and the root everything resolves against.
///
/// Carried together because every step below needs all three, and threading
/// them one by one is what pushed these functions past the argument limit.
struct Store {
    ledger: ledger::Ledger,
    path: PathBuf,
    base: PathBuf,
}

impl Store {
    fn open(base: PathBuf) -> Result<Self, String> {
        let path = ledger::path_in(&base);
        let ledger = ledger::load(&path).map_err(|error| error.to_string())?;
        Ok(Self { ledger, path, base })
    }
}

/// Which rows to act on.
///
/// `--all` means every EXTRACTED SKILL, and an `M` row named explicitly is
/// a REFUSAL rather than a silent skip: someone who typed a rule's id
/// meant something by it, and dropping it from a batch of five would
/// report success over work that did not happen.
fn chosen(
    held: &ledger::Ledger,
    args: &IssueArgs,
) -> Result<Vec<ledger::Extracted>, String> {
    if args.all {
        return Ok(held
            .extracted
            .iter()
            .filter(|row| row.label.starts_with('S'))
            .cloned()
            .collect());
    }
    args.ids.iter().map(|id| one_row(held, id)).collect()
}

fn one_row(
    held: &ledger::Ledger,
    id: &str,
) -> Result<ledger::Extracted, String> {
    let row = held
        .find(id)
        .cloned()
        .ok_or_else(|| format!("no extraction matches `{id}`"))?;
    if row.label.starts_with('S') {
        return Ok(row);
    }
    Err(issue::Fault::NotASkill {
        id: row.id,
        label: row.label,
    }
    .to_string())
}

/// What every chosen row would do, computed before anything is written.
fn preview(
    rows: &[ledger::Extracted],
    base: &Path,
    args: &IssueArgs,
) -> issue::Report {
    issue::Report {
        rows: rows
            .iter()
            .map(|row| one_preview(row, base, args))
            .collect(),
    }
}

fn one_preview(
    row: &ledger::Extracted,
    base: &Path,
    args: &IssueArgs,
) -> issue::Outcome {
    let stands = base.join(&row.artifact).exists();
    let mut out = issue::Outcome {
        id: row.id.clone(),
        repairs: repair_needed(row, base),
        links: link_needed(row, base),
        ..issue::Outcome::default()
    };
    fill_move(&mut out, row, args, stands);
    out.already = is_noop(&out);
    out
}

/// The two halves that MOVE something, as opposed to repairing what is
/// already here.
fn fill_move(
    out: &mut issue::Outcome,
    row: &ledger::Extracted,
    args: &IssueArgs,
    stands: bool,
) {
    if let Some(dir) = args.to.as_ref() {
        out.issues = named(&destination(dir, &row.artifact));
    }
    if args.retire && stands {
        out.retires = row.artifact.clone();
    }
}

fn is_noop(out: &issue::Outcome) -> bool {
    out.issues.is_empty()
        && out.repairs.is_empty()
        && out.links.is_empty()
        && out.retires.is_empty()
}

/// The head repair this row needs, or nothing.
///
/// An artifact that is GONE needs none: a retired row has no local copy by
/// design (`src/issue:V54`), and reporting a repair for a file that is not
/// there would be inventing work.
fn repair_needed(row: &ledger::Extracted, base: &Path) -> String {
    let path = base.join(&row.artifact);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return String::new();
    };
    if issue::guarded(&text) == text {
        return String::new();
    }
    row.artifact.clone()
}

/// The host link this row is missing, or nothing.
///
/// Reported PROJECT-RELATIVE, like every other path this crate prints. An
/// absolute one leaks the machine's layout into `--format json`, which is
/// written into CI artifacts.
fn link_needed(row: &ledger::Extracted, base: &Path) -> String {
    let hosts = base.join(".claude").join("skills");
    let Some(slug) = apply::artifact::skill_slug(&row.artifact) else {
        return String::new();
    };
    if !hosts.is_dir() || hosts.join(&slug).symlink_metadata().is_ok() {
        return String::new();
    }
    format!(".claude/skills/{slug}")
}

/// Where a portable copy of this artifact lands under a destination.
///
/// `<dir>/<slug>/SKILL.md`, and nothing more is assumed. This crate knows
/// NOTHING about a registry's layout (`src/issue:V54`), so it writes the
/// skill as a self-contained directory and leaves adoption to the
/// registry -- which is the only side that knows where its own sets live.
fn destination(dir: &Path, artifact: &str) -> PathBuf {
    let slug = apply::artifact::skill_slug(artifact)
        .unwrap_or_else(|| "rekall-skill".into());
    dir.join(slug).join("SKILL.md")
}

fn named(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Name it, ask, then do it (V7, V20).
fn ask_then_write(
    planned: &issue::Report,
    rows: &[ledger::Extracted],
    args: &IssueArgs,
    store: &mut Store,
) -> Result<Output, String> {
    let json = matches!(args.format, Format::Json);
    if planned.rows.iter().all(|row| row.already) {
        return report(planned, json, vec![], render_issue_human);
    }
    let named = render_issue_human(planned, &[]);
    approved(&consent_for(args.auto_approve, &named)?)?;
    let done = perform(rows, args, store)?;
    ledger::save(&store.path, &store.ledger)
        .map_err(|error| error.to_string())?;
    report(planned, json, done, render_issue_human)
}

fn perform(
    rows: &[ledger::Extracted],
    args: &IssueArgs,
    store: &mut Store,
) -> Result<Vec<String>, String> {
    let mut done = Vec::new();
    for row in rows {
        one_row_performed(row, args, store, &mut done)?;
    }
    Ok(done)
}

/// The ORDER is the invariant, and it is the order `src/issue:V54` argues
/// for.
///
/// Repair and link first, so what goes out is the artifact in the state
/// this crate says it should be in. Then write the portable copy. Then,
/// and only if asked, retire the local one -- so a failure anywhere above
/// leaves the rule still enforced HERE, which is the property the whole
/// staged move exists to keep.
fn one_row_performed(
    row: &ledger::Extracted,
    args: &IssueArgs,
    store: &mut Store,
    done: &mut Vec<String>,
) -> Result<(), String> {
    repair(row, &store.base, done)?;
    relink(row, &store.base, done)?;
    if let Some(dir) = args.to.as_ref() {
        write_copy(row, &store.base, dir, done)?;
        store.ledger.issue(&row.id, &named(dir));
    }
    if args.retire {
        retire(row, store, done)?;
    }
    Ok(())
}

fn repair(
    row: &ledger::Extracted,
    base: &Path,
    done: &mut Vec<String>,
) -> Result<(), String> {
    let path = base.join(&row.artifact);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let fixed = issue::guarded(&text);
    if fixed == text {
        return Ok(());
    }
    std::fs::write(&path, fixed).map_err(|error| error.to_string())?;
    done.push(format!("guarded the head of {}", row.artifact));
    Ok(())
}

fn relink(
    row: &ledger::Extracted,
    base: &Path,
    done: &mut Vec<String>,
) -> Result<(), String> {
    let missing = link_needed(row, base);
    if missing.is_empty() {
        return Ok(());
    }
    publish_artifact(base, &row.label, &row.artifact)?;
    done.push(format!("linked {missing}"));
    Ok(())
}

/// The portable copy, written VERBATIM.
///
/// Byte for byte what stands here, guard included. `issue` is a move and
/// not an authoring step: rewriting the file on the way out would mean
/// this crate deciding how the artifact should read in a repository it
/// knows nothing about. The guard's consequence there is REPORTED instead
/// -- see the note the human rendering prints.
fn write_copy(
    row: &ledger::Extracted,
    base: &Path,
    dir: &Path,
    done: &mut Vec<String>,
) -> Result<(), String> {
    let from = base.join(&row.artifact);
    let text = std::fs::read_to_string(&from).map_err(|why| {
        format!("cannot read {} to issue it: {why}", row.artifact)
    })?;
    let to = destination(dir, &row.artifact);
    let parent = to.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|why| why.to_string())?;
    std::fs::write(&to, text).map_err(|why| why.to_string())?;
    done.push(format!("issued {} to {}", row.id, named(&to)));
    Ok(())
}

/// Drop the local copy, keep the row.
///
/// The row STAYS, with its `issued_to` intact. That is what makes a
/// retired extraction readable afterwards -- what was extracted, from
/// where, and which registry tends it now -- and what stops `check`
/// reporting the end state as a missing artifact.
fn retire(
    row: &ledger::Extracted,
    store: &Store,
    done: &mut Vec<String>,
) -> Result<(), String> {
    let issued = store
        .ledger
        .find(&row.id)
        .is_some_and(|held| !held.issued_to.is_empty());
    if !issued {
        return Err(issue::Fault::NotIssued { id: row.id.clone() }.to_string());
    }
    unlink(&store.base, &row.artifact);
    drop_artifact(&store.base.join(&row.artifact), &row.artifact, done)
}

fn drop_artifact(
    path: &Path,
    named: &str,
    done: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(path).map_err(|why| why.to_string())?;
    done.push(format!("retired {named}"));
    Ok(())
}

/// The host's link goes before the file it points at, for the reason
/// `revert` states: a dangling link is a leftover nothing reports.
fn unlink(base: &Path, artifact: &str) {
    let Some(slug) = apply::artifact::skill_slug(artifact) else {
        return;
    };
    let link = base.join(".claude").join("skills").join(slug);
    if link.symlink_metadata().is_ok() {
        let _ = std::fs::remove_file(&link);
    }
}

#[must_use]
pub fn render_issue_human(report: &issue::Report, done: &[String]) -> String {
    let mut out = String::new();
    for row in &report.rows {
        out.push_str(&one_rendered(row));
    }
    for line in done {
        out.push_str(&format!("done    {line}\n"));
    }
    if report.rows.iter().any(|row| !row.issues.is_empty()) {
        out.push_str(ADOPTION_NOTE);
    }
    out
}

/// What the registry has to decide, said HERE rather than guessed at.
///
/// The copy carries `disable-model-invocation: true`, which is right in a
/// repository where `rekall hook` delivers the skill and wrong in one
/// where nothing does -- there it would arrive switched off. This crate
/// cannot see which kind the destination is, so it says so instead of
/// picking (`src/issue:V54`).
pub const ADOPTION_NOTE: &str = "\
note    the copy keeps `disable-model-invocation: true` -- right where \
`rekall hook` delivers the skill, wrong where nothing does. The registry \
decides; this end cannot see which it is.\n";

fn one_rendered(row: &issue::Outcome) -> String {
    if row.already {
        return format!("skip    {} (nothing to issue)\n", row.id);
    }
    let mut out = String::new();
    for (verb, what) in [
        ("guard", &row.repairs),
        ("link", &row.links),
        ("issue", &row.issues),
        ("retire", &row.retires),
    ] {
        if !what.is_empty() {
            out.push_str(&format!("{verb:<7} {what}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::testing::*;
    use crate::cli::{Action, decide, perform};

    /// A project with one extracted SKILL, its head left in the state the
    /// old template wrote -- no guard. That is `B10` on disk.
    fn issue_project(name: &str) -> (PathBuf, String) {
        let dir = scaffold(name);
        let (id, _) = extracted(&dir);
        let path = dir.join(artifact_of(&dir, &id));
        let _ = std::fs::write(&path, unguarded());
        (dir, id)
    }

    fn scaffold(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-issue").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- when editing Rust files, run clippy first\n",
        );
        dir
    }

    /// A skill in the state every artifact this crate wrote before V52
    /// was in: filled triggers, and a head that does NOT disable the
    /// host's own loading. That is B10 on disk.
    fn unguarded() -> String {
        skill_with("tool = [\"Edit\"]", "word = []")
            .replace("disable-model-invocation: true\n", "")
    }

    fn issue_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        flags.push("--auto-approve".to_string());
        issue_command(&flags, &env())
    }

    fn row_of(dir: &Path, id: &str) -> ledger::Extracted {
        ledger::load(&ledger::path_in(dir))
            .unwrap_or_default()
            .find(id)
            .cloned()
            .unwrap_or_default()
    }

    /// Dispatch is not the same as REACHING the verb: `perform` is the
    /// arm that actually calls it, and a table row nothing routes through
    /// is a verb the binary knows about and cannot run.
    #[test]
    fn issue_runs_through_perform() {
        let dir = scaffold("perform");
        let mut flags = dash_c(&dir);
        flags.push("--all".to_string());
        flags.push("--auto-approve".to_string());
        assert_eq!(perform(Action::Issue(flags), &env()), 0);
    }

    #[test]
    fn issue_is_dispatched() {
        assert_eq!(
            decide(&args(&["issue", "abc"])),
            Action::Issue(args(&["abc"]))
        );
    }

    /// V54, the whole point: after `issue` the copy is THERE and the
    /// local artifact is STILL HERE. Removing it now would leave the rule
    /// enforced by nothing until the registry materializes it back.
    #[test]
    fn issuing_writes_the_copy_and_leaves_the_local_one_standing() {
        let (dir, id) = issue_project("zero-downtime");
        let out = dir.join("registry");
        let _ = issue_in(&dir, &[&id, "--to", &out.to_string_lossy()]);
        let slug = apply::artifact::skill_slug(&artifact_of(&dir, &id))
            .unwrap_or_default();
        assert!(out.join(&slug).join("SKILL.md").is_file(), "copy written");
        assert!(
            dir.join(artifact_of(&dir, &id)).is_file(),
            "the local artifact still stands"
        );
    }

    /// The row is what makes two copies a TRANSITION and not the
    /// duplication V1 forbids, so the destination has to be recorded.
    #[test]
    fn the_row_records_where_it_went() {
        let (dir, id) = issue_project("records");
        let out = dir.join("registry");
        let _ = issue_in(&dir, &[&id, "--to", &out.to_string_lossy()]);
        assert_eq!(row_of(&dir, &id).issued_to, out.to_string_lossy());
    }

    /// What goes out is what stands here, byte for byte. `issue` is a
    /// move, not an authoring step.
    #[test]
    fn the_copy_is_verbatim() {
        let (dir, id) = issue_project("verbatim");
        let out = dir.join("registry");
        let _ = issue_in(&dir, &[&id, "--to", &out.to_string_lossy()]);
        let artifact = artifact_of(&dir, &id);
        let slug = apply::artifact::skill_slug(&artifact).unwrap_or_default();
        assert_eq!(
            std::fs::read_to_string(out.join(slug).join("SKILL.md")).ok(),
            std::fs::read_to_string(dir.join(&artifact)).ok()
        );
    }

    /// T66 is a repair of ONE LINE. The triggers a human filled in are
    /// the only part of the artifact this crate did not write, and
    /// regenerating the file would discard exactly them.
    #[test]
    fn reissuing_guards_the_head_and_keeps_the_triggers() {
        let (dir, id) = issue_project("guard");
        let path = dir.join(artifact_of(&dir, &id));
        let _ = issue_in(&dir, &[&id]);
        let after = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(issue::has_guard(&after), "the guard is in the head");
        assert!(after.contains("tool = [\"Edit\"]"), "triggers survive");
    }

    /// V13's shape: a second run has nothing left to do, says so, and
    /// does not fail.
    #[test]
    fn a_second_reissue_is_a_no_op_that_says_so() {
        let (dir, id) = issue_project("noop");
        let _ = issue_in(&dir, &[&id]);
        let text = issue_in(&dir, &[&id]).map(|o| o.text).unwrap_or_default();
        assert!(text.contains("nothing to issue"), "{text}");
    }

    /// Retiring before issuing would delete the only copy of an enforced
    /// rule and leave it nowhere. It is a refusal, and it names the fix.
    #[test]
    fn retiring_a_row_that_was_never_issued_is_refused() {
        let (dir, id) = issue_project("premature");
        let why = issue_in(&dir, &[&id, "--retire"]).err().unwrap_or_default();
        assert!(why.contains("never issued"), "{why}");
        assert!(dir.join(artifact_of(&dir, &id)).is_file(), "still here");
    }

    /// And once it HAS been issued, retiring drops the local copy and
    /// KEEPS the row -- which is what stops `check` reading the end state
    /// as a missing artifact.
    #[test]
    fn retiring_drops_the_artifact_and_keeps_the_row() {
        let (dir, id) = issue_project("retire");
        let out = dir.join("registry");
        let _ = issue_in(&dir, &[&id, "--to", &out.to_string_lossy()]);
        let artifact = artifact_of(&dir, &id);
        let _ = issue_in(&dir, &[&id, "--retire"]);
        assert!(!dir.join(&artifact).exists(), "the local copy is gone");
        let row = row_of(&dir, &id);
        assert_eq!(row.artifact, artifact, "the row still names it");
        assert!(!row.issued_to.is_empty(), "and still says where it went");
    }

    /// An `M` row named explicitly is a REFUSAL, not a silent skip: a
    /// rule's artifact is a script the gate runs by path, and moving it
    /// out breaks the wiring that makes it a rule.
    #[test]
    fn a_mechanical_rule_cannot_be_issued() {
        let dir = one_extracted_rule("issue-rule");
        let id = rule_id(&dir);
        let why = issue_in(&dir, &[&id]).err().unwrap_or_default();
        assert!(why.contains("mechanical rule"), "{why}");
    }

    /// No id and no `--all` is a USAGE error naming both ways out, not an
    /// invocation that quietly does nothing.
    #[test]
    fn issue_with_nothing_named_says_what_to_pass() {
        let why = issue_command(&args(&[]), &env()).err().unwrap_or_default();
        assert!(why.contains("--all"), "{why}");
    }

    /// The registry has to decide what the guard means at its end, and
    /// this end cannot see which kind of repo it is. So it SAYS so.
    #[test]
    fn issuing_prints_what_the_registry_has_to_decide() {
        let (dir, id) = issue_project("note");
        let out = dir.join("registry");
        let text = issue_in(&dir, &[&id, "--to", &out.to_string_lossy()])
            .map(|o| o.text)
            .unwrap_or_default();
        assert!(text.contains("The registry decides"), "{text}");
    }

    /// `--all` takes every extracted SKILL, so a corpus that has proved
    /// several does not need the ids typed out one at a time.
    #[test]
    fn all_takes_every_skill() {
        let (dir, id) = issue_project("all");
        let text = issue_in(&dir, &["--all"])
            .map(|o| o.text)
            .unwrap_or_default();
        assert!(text.contains(&artifact_of(&dir, &id)), "{text}");
    }

    /// V17: the JSON says the same thing the prose does, and an unknown
    /// format is a usage error rather than a silent fall back.
    #[test]
    fn issue_speaks_json_too() {
        let (dir, id) = issue_project("json");
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(&[&id, "--format", "json", "--auto-approve"]));
        let text = issue_command(&flags, &env())
            .map(|o| o.text)
            .unwrap_or_default();
        assert!(text.contains("\"repairs\""), "{text}");
        flags.pop();
        assert!(parse_issue(&args(&["--format", "yaml"])).is_err());
    }

    #[test]
    fn an_unknown_issue_flag_is_an_error() {
        assert!(parse_issue(&args(&["--nope"])).is_err());
    }

    /// The link half of a reissue: a checkout whose host directory exists
    /// but whose link was never made gets one, and a second run does not
    /// report it again.
    #[test]
    fn a_missing_host_link_is_republished_once() {
        let (dir, id) = issue_project("relink");
        let hosts = dir.join(".claude").join("skills");
        let _ = std::fs::create_dir_all(&hosts);
        let text = issue_in(&dir, &[&id]).map(|o| o.text).unwrap_or_default();
        assert!(text.contains("linked"), "{text}");
        let slug = apply::artifact::skill_slug(&artifact_of(&dir, &id))
            .unwrap_or_default();
        assert!(hosts.join(&slug).symlink_metadata().is_ok(), "link made");
        let again = issue_in(&dir, &[&id]).map(|o| o.text).unwrap_or_default();
        assert!(again.contains("nothing to issue"), "{again}");
    }

    /// Paths this crate PRINTS are project-relative. MEASURED otherwise:
    /// the link line reported an absolute host path, the only absolute
    /// path in any report, and `--format json` goes into CI artifacts.
    #[test]
    fn the_link_line_is_project_relative() {
        let (dir, id) = issue_project("relative-link");
        let _ = std::fs::create_dir_all(dir.join(".claude").join("skills"));
        let text = issue_in(&dir, &[&id]).map(|o| o.text).unwrap_or_default();
        assert!(text.contains(".claude/skills/"), "{text}");
        assert!(!text.contains("/target/cli-issue"), "{text}");
    }

    /// And retiring takes the link with the file, for the reason `revert`
    /// states: a dangling link is a leftover nothing reports.
    #[test]
    fn retiring_removes_the_host_link_too() {
        let (dir, id) = issue_project("unlink");
        let hosts = dir.join(".claude").join("skills");
        let _ = std::fs::create_dir_all(&hosts);
        let out = dir.join("registry");
        let _ = issue_in(&dir, &[&id, "--to", &out.to_string_lossy()]);
        let slug = apply::artifact::skill_slug(&artifact_of(&dir, &id))
            .unwrap_or_default();
        let _ = issue_in(&dir, &[&id, "--retire"]);
        assert!(hosts.join(&slug).symlink_metadata().is_err(), "link gone");
    }

    /// An artifact that is not there needs no repair and cannot be
    /// issued. The first is silence; the second names the file.
    #[test]
    fn an_absent_artifact_is_not_repaired_and_will_not_copy() {
        let (dir, id) = issue_project("absent");
        let artifact = artifact_of(&dir, &id);
        let _ = std::fs::remove_file(dir.join(&artifact));
        let row = row_of(&dir, &id);
        assert_eq!(repair_needed(&row, &dir), "");
        let why = issue_in(&dir, &[&id, "--to", "target/nowhere"])
            .err()
            .unwrap_or_default();
        assert!(why.contains("cannot read"), "{why}");
    }

    /// An artifact path with no directory of its own has no slug, so
    /// there is nothing to link and nothing to unlink -- and neither
    /// path may panic reaching for one.
    #[test]
    fn an_artifact_with_no_directory_has_no_link() {
        let dir = scaffold("slugless");
        let mut row = row_of(&dir, "nothing");
        row.artifact = "SKILL.md".to_string();
        assert_eq!(link_needed(&row, &dir), "");
        unlink(&dir, &row.artifact);
    }

    /// Retiring an artifact somebody already deleted is not an error:
    /// the row ends in the state the retirement promised.
    #[test]
    fn retiring_a_file_that_is_already_gone_is_quiet() {
        let mut done = Vec::new();
        let path = PathBuf::from("target").join("cli-issue").join("no-such");
        assert!(drop_artifact(&path, "x", &mut done).is_ok());
        assert!(done.is_empty());
    }
}
