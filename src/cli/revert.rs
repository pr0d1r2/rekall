use super::apply::{approved, consent_for};
use super::{
    Env, Format, Output, load_corpus, need, one, parse_format, report,
};
use crate::{ledger, plan, revert};
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
fn remove_artifact(
    row: &ledger::Extracted,
    source: &Path,
) -> Result<String, String> {
    let path = base_of(source, &row.src).join(&row.artifact);
    if !path.exists() {
        return Ok(format!("{} was already gone", row.artifact));
    }
    std::fs::remove_file(&path)
        .map(|()| format!("removed {}", row.artifact))
        .map_err(|cause| {
            format!(
                "{} is back in {}, but {} could not be removed: {cause}. \
                 Delete it by hand -- the ledger row is kept so `rekall check` \
                 still reports the leftover",
                row.id, row.src, row.artifact
            )
        })
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
