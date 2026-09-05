use super::apply::{approved, consent_for};
use super::{
    Env, Format, Output, load_corpus, need, one, parse_format, report,
};
use crate::{apply, ledger, plan, revert};
use std::path::{Path, PathBuf};
/// `revert` takes ONE id plus `--auto-approve` and the usual flags.
///
/// One, because section I writes it `revert <id>` and because reverting is
/// the deliberate undoing of a single decision. A batch revert would need
/// its own answer to what happens when the third of five cannot be found,
/// and that answer is better as five commands.
#[derive(Debug, Default)]
pub struct RevertArgs {
    pub id: Option<String>,
    pub auto_approve: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_revert(args: &[String]) -> Result<RevertArgs, String> {
    let mut out = RevertArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_revert_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_revert_arg<'a>(
    out: &mut RevertArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--auto-approve" => out.auto_approve = true,
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        id => return set_revert_id(out, id),
    }
    Ok(())
}

/// A second positional is a MISTAKE, not a second revert -- the same
/// reasoning `show` uses, and it matters more here: quietly reverting only
/// the first of two named ids would leave the second extraction in place
/// while the command reported success.
fn set_revert_id(out: &mut RevertArgs, id: &str) -> Result<(), String> {
    if out.id.is_some() {
        return Err(format!("`revert` takes one id, got a second: `{id}`"));
    }
    out.id = Some(id.to_string());
    Ok(())
}

pub const NO_REVERT_INPUT: &str =
    "`revert` needs an id -- copy one from `rekall log`";

/// Run `revert` end to end.
///
/// Order is the point, as it is in `apply`: find the row, compute the
/// restored bytes and REFUSE if the pointer is gone, name both files
/// (V7), ask (V20), and only then write.
pub fn revert_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_revert(flags)?;
    let id = args.id.clone().ok_or_else(|| NO_REVERT_INPUT.to_string())?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let mut held = Held::open(&base)?;
    let Some(row) = chosen_row(&held.ledger, &id, &base, env)? else {
        return nothing_to_revert(&id, args.json);
    };
    undo(&row, &base, &args, &mut held)
}

/// V13: nothing left to undo is a no-op at exit 0, and says so.
fn nothing_to_revert(id: &str, json: bool) -> Result<Output, String> {
    report(
        &revert::nothing_to_do(id),
        json,
        vec!["nothing to do".to_string()],
        render_revert_human,
    )
}

/// Compute, name, ask, write -- in that order (V7, V20).
fn undo(
    row: &ledger::Extracted,
    base: &Path,
    args: &RevertArgs,
    held: &mut Held,
) -> Result<Output, String> {
    let restored = restored_text(row, base)?;
    let outcome = revert::preview(row);
    let named = render_revert_human(&outcome, &[]);
    approved(&consent_for(args.auto_approve, &named)?)?;
    let done = perform_revert(row, &restored, held)?;
    report(&outcome, args.json, done, render_revert_human)
}

/// The ledger and where it lives, carried together so writing it back does
/// not need the path threaded through every call beneath it.
struct Held {
    ledger: ledger::Ledger,
    path: PathBuf,
}

impl Held {
    fn open(base: &Path) -> Result<Self, String> {
        let path = ledger::path_in(base);
        let ledger = ledger::load(&path).map_err(|error| error.to_string())?;
        Ok(Self { ledger, path })
    }
}

/// Which ledger row to undo, or `None` when there is nothing left to undo.
///
/// A prefix the ledger does not hold is NOT automatically an error. If the
/// statement is back in the corpus, this id has already been reverted --
/// and its text rehashes to the same id, which is what makes the check
/// possible at all. V13 makes that a no-op at exit 0, the mirror of
/// re-applying an id the ledger already holds. An id in NEITHER place is
/// still an error.
fn chosen_row(
    held: &ledger::Ledger,
    id: &str,
    base: &Path,
    env: &Env,
) -> Result<Option<ledger::Extracted>, String> {
    match held.matching(id).as_slice() {
        [only] => Ok(Some((*only).clone())),
        [] => already_reverted(id, base, env),
        many => Err(plan::Error::Ambiguous(
            id.to_string(),
            many.iter().map(|row| row.id.clone()).collect(),
        )
        .to_string()),
    }
}

fn already_reverted(
    id: &str,
    base: &Path,
    env: &Env,
) -> Result<Option<ledger::Extracted>, String> {
    let loaded = load_corpus(base, env)?;
    one(&loaded.statements, id).map(|_| None)
}

/// The bytes to put back, computed BEFORE anything is asked or written.
///
/// Refusing before the prompt matters twice over: asking someone to
/// approve an edit that cannot happen wastes the one deliberate act V20
/// exists to require, and it fixes the ORDER -- the source is restored
/// before the artifact is removed, so a failure never leaves the artifact
/// deleted and the statement still absent, which would lose the rule
/// outright.
struct Restored {
    path: PathBuf,
    text: String,
}

fn restored_text(
    row: &ledger::Extracted,
    base: &Path,
) -> Result<Restored, String> {
    let path = base.join(&row.src);
    let text =
        std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let text =
        revert::unsplice(&text, row).map_err(|fault| fault.to_string())?;
    Ok(Restored { path, text })
}

fn perform_revert(
    row: &ledger::Extracted,
    restored: &Restored,
    held: &mut Held,
) -> Result<Vec<String>, String> {
    std::fs::write(&restored.path, &restored.text)
        .map_err(|error| error.to_string())?;
    let mut done = vec![format!("restored {}", row.src)];
    done.push(remove_artifact(row, &restored.path)?);
    held.ledger.take(&row.id);
    ledger::save(&held.path, &held.ledger)
        .map_err(|error| error.to_string())?;
    done.push(format!("dropped the ledger row for {}", row.id));
    Ok(done)
}

/// The other half of the move (V1).
///
/// An artifact that is ALREADY GONE is reported, not an error: the corpus
/// still ends in the state the revert promised. An artifact that will not
/// delete IS an error, and the ledger row is left in place on purpose --
/// the extraction is not fully undone, and a ledger that said it was would
/// hide the leftover from `check` as well as from the user.
/// Remove the host's symlink before the file it points at.
///
/// `apply` may publish a skill by linking it into `.claude/skills/` (V48).
/// Deleting only the target would leave a DANGLING link that the host still
/// indexes and that `revert` claimed to have undone -- a leftover reported
/// by nothing, which is the state this verb exists to prevent.
///
/// Best effort and silent: a link nobody created is not an error, and a
/// link that cannot be removed must not stop the corpus being restored.
fn unpublish(base: &Path, artifact: &str) {
    let Some(slug) = apply::skill_slug(artifact) else {
        return;
    };
    let link = base.join(".claude").join("skills").join(slug);
    if link.symlink_metadata().is_ok() {
        let _ = std::fs::remove_file(&link);
    }
}

fn remove_artifact(
    row: &ledger::Extracted,
    source: &Path,
) -> Result<String, String> {
    let base = base_of(source, &row.src);
    let path = base.join(&row.artifact);
    unpublish(&base, &row.artifact);
    if !path.exists() {
        return Ok(format!("{} was already gone", row.artifact));
    }
    std::fs::remove_file(&path)
        .map(|()| format!("removed {}", row.artifact))
        .map_err(|cause| stuck(row, &cause.to_string()))
}

fn stuck(row: &ledger::Extracted, cause: &str) -> String {
    format!(
        "{} is back in {}, but {} could not be removed: {cause}. \
         Delete it by hand -- the ledger row is kept so `rekall check` \
         still reports the leftover",
        row.id, row.src, row.artifact
    )
}

/// The project root, recovered from the source path that was just written.
///
/// Derived rather than passed, so the artifact and the source it pairs
/// with can never be resolved against two different roots.
fn base_of(source: &Path, src: &str) -> PathBuf {
    let mut base = source.to_path_buf();
    for _ in Path::new(src).components() {
        base.pop();
    }
    base
}

#[must_use]
pub fn render_revert_human(
    outcome: &revert::Outcome,
    done: &[String],
) -> String {
    let mut out = String::new();
    if outcome.already {
        out.push_str(&format!("skip    {} (nothing to revert)\n", outcome.id));
    }
    if !outcome.restores.is_empty() {
        out.push_str(&format!("restore {}\n", outcome.restores));
        out.push_str(&format!("remove  {}\n", outcome.removes));
    }
    for line in done {
        out.push_str(&format!("done    {line}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::testing::*;
    use crate::cli::{Action, USAGE_EXIT, decide, perform};
    use crate::ledger;
    use std::path::Path;

    #[test]
    fn revert_is_dispatched() {
        assert_eq!(
            decide(&args(&["revert", "abc"])),
            Action::Revert(args(&["abc"]))
        );
    }

    /// V9: THE PROMISE. Extract, revert, and the file is the file that was
    /// there -- byte for byte. The ledger's verbatim text is what makes it
    /// a replay rather than a rewrite.
    #[test]
    fn revert_restores_the_original_bytes() {
        let dir = revert_project("bytes");
        let (id, before) = extracted(&dir);
        let result = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before);
    }

    /// V1, in the other direction. Restoring the source and leaving the
    /// artifact behind would leave two hand-maintained statements of one
    /// rule -- the copy V1 exists to forbid.
    #[test]
    fn revert_removes_the_artifact_and_the_ledger_row() {
        let dir = revert_project("artifact");
        let (id, _) = extracted(&dir);
        let artifact = artifact_of(&dir, &id);
        assert!(dir.join(&artifact).is_file(), "nothing was extracted");
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(
            !dir.join(&artifact).exists(),
            "the artifact survived: {artifact}"
        );
        let after = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert!(after.extracted.is_empty(), "{after:?}");
    }

    /// V20: off a tty and without the flag, revert refuses -- and the
    /// corpus is untouched. The confirm gate is one rule, not one per verb.
    #[test]
    fn revert_without_approval_refuses_and_changes_nothing() {
        let dir = revert_project("unattended");
        let (id, _) = extracted(&dir);
        let extracted_corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let message = revert_in(&dir, &[&id]).err().unwrap_or_default();
        assert!(message.contains("--auto-approve"), "message was {message}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(
            after, extracted_corpus,
            "a refused revert edited the corpus"
        );
    }

    /// V13: reverting twice is a no-op at exit 0. The statement is back in
    /// the corpus and its text rehashes to the same id, so "not in the
    /// ledger" means work already undone rather than an unknown id.
    #[test]
    fn reverting_twice_is_a_no_op() {
        let dir = revert_project("idempotent");
        let (id, before) = extracted(&dir);
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        let second = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(second.is_ok(), "{second:?}");
        let text = second.map(|out| out.text).unwrap_or_default();
        assert!(text.contains("nothing to revert"), "{text}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before, "a second revert changed the corpus");
    }

    #[test]
    fn an_id_in_neither_the_ledger_nor_the_corpus_is_an_error() {
        let dir = revert_project("unknown");
        let message = revert_in(&dir, &["zzzzzzz", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    /// A ledger holding two rows that share a prefix, written by hand
    /// because two REAL extractions get two hex ids that are not
    /// guaranteed to collide on any character.
    fn write_twin_rows(dir: &Path) {
        let _ = std::fs::create_dir_all(dir.join(ledger::DIR));
        let _ = std::fs::write(
            ledger::path_in(dir),
            "[[extracted]]\nid = \"aaa1111\"\nsrc = \"CLAUDE.md\"\n\
             line_start = 1\nline_end = 1\ntext = \"x\"\nartifact = \"a.sh\"\n\n\
             [[extracted]]\nid = \"aaa2222\"\nsrc = \"CLAUDE.md\"\n\
             line_start = 2\nline_end = 2\ntext = \"y\"\nartifact = \"b.sh\"\n",
        );
    }

    /// Section I: a prefix that matches two ids is a usage error, not a
    /// coin toss -- and it says WHICH two, so the fix is to type more
    /// characters rather than to guess.
    #[test]
    fn an_ambiguous_prefix_names_the_ids_it_matched() {
        let dir = revert_project("ambiguous");
        write_twin_rows(&dir);
        let message = revert_in(&dir, &["aaa", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("Use more characters"), "{message}");
        assert!(message.contains("aaa1111"), "{message}");
    }

    /// The pointer is the ADDRESS. If someone deleted it by hand there is
    /// no single place the text belongs, so revert refuses -- and names
    /// what to do instead rather than only what went wrong (V28).
    #[test]
    fn a_hand_edited_pointer_refuses_and_names_the_fix() {
        let dir = revert_project("no-pointer");
        let (id, _) = extracted(&dir);
        let _ = std::fs::write(dir.join("CLAUDE.md"), "# Rules\n\n- prose\n");
        let message = revert_in(&dir, &[&id, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no longer holds the pointer"),
            "message was {message}"
        );
        assert!(message.contains("rekall log"), "no fix named: {message}");
    }

    /// An artifact someone already deleted is REPORTED, not an error: the
    /// corpus still ends in the state the revert promised.
    #[test]
    fn an_artifact_that_is_already_gone_is_reported_not_fatal() {
        let dir = revert_project("gone");
        let (id, before) = extracted(&dir);
        let _ = std::fs::remove_file(dir.join(artifact_of(&dir, &id)));
        let text = revert_in(&dir, &[&id, "--auto-approve"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("was already gone"), "{text}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before);
    }

    /// An artifact that will NOT delete is an error, and the ledger row is
    /// kept on purpose: the extraction is not fully undone, and a ledger
    /// that said it was would hide the leftover from `check` too. A
    /// directory standing where the file should be is the portable way to
    /// make the removal fail.
    #[test]
    fn an_artifact_that_cannot_be_removed_is_an_error() {
        let dir = revert_project("stuck");
        let (id, _) = extracted(&dir);
        let artifact = dir.join(artifact_of(&dir, &id));
        let _ = std::fs::remove_file(&artifact);
        let _ = std::fs::create_dir_all(artifact.join("in-the-way"));
        let message = revert_in(&dir, &[&id, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("could not be removed"), "{message}");
        let after = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(after.extracted.len(), 1, "the row was dropped anyway");
    }

    #[test]
    fn revert_json_is_parseable() {
        let dir = revert_project("json");
        let (id, _) = extracted(&dir);
        let text =
            revert_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.text)
                .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }

    #[test]
    fn a_json_revert_reports_what_it_did_on_stderr() {
        let dir = revert_project("json-warnings");
        let (id, _) = extracted(&dir);
        let warnings =
            revert_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.warnings)
                .unwrap_or_default();
        assert!(
            warnings.iter().any(|w| w.contains("restored ")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("removed ")),
            "{warnings:?}"
        );
    }

    #[test]
    fn revert_without_an_id_says_what_to_run() {
        let dir = revert_project("no-input");
        let message = revert_in(&dir, &["--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("rekall log"), "message was {message}");
    }

    #[test]
    fn revert_takes_one_id_and_a_second_is_an_error() {
        assert!(parse_revert(&args(&["aaa", "bbb"])).is_err());
        assert!(parse_revert(&args(&["--nope"])).is_err());
        assert!(parse_revert(&args(&["--format"])).is_err());
    }

    #[test]
    fn a_successful_revert_exits_zero() {
        let dir = revert_project("exit-zero");
        let (id, _) = extracted(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.push("--auto-approve".to_string());
        assert_eq!(perform(Action::Revert(flags), &env()), 0);
    }

    #[test]
    fn a_failed_revert_exits_two() {
        assert_eq!(
            perform(
                Action::Revert(args(&["zzzzzzz", "--auto-approve"])),
                &env()
            ),
            USAGE_EXIT
        );
    }

    /// The revert of a corpus file in a NESTED directory. The artifact is
    /// resolved against the project root, and recovering that root from
    /// the source path is the step that would silently go wrong.
    #[test]
    fn a_nested_source_file_resolves_its_artifact_from_the_root() {
        let dir = nested_project("nested");
        let nested = dir.join("docs").join("AGENTS.md");
        let (id, before) = extracted_from(&dir, &nested);
        let artifact = artifact_of(&dir, &id);
        assert!(dir.join(&artifact).is_file(), "nothing was extracted");
        let result = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(
            std::fs::read_to_string(&nested).unwrap_or_default(),
            before
        );
        assert!(
            !dir.join(&artifact).exists(),
            "the artifact resolved against the wrong root: {artifact}"
        );
    }
}
