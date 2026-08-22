use super::{
    Env, Format, Output, load_corpus, need, one, parse_format, report,
};
use crate::{apply, ledger, plan, scan, statement};
use std::path::{Path, PathBuf};
/// `apply` takes ids OR a plan file, plus `--auto-approve`.
#[derive(Debug, Default)]
pub struct ApplyArgs {
    pub ids: Vec<String>,
    /// A positional that names an EXISTING FILE is read as a plan; anything
    /// else is an id. Section I writes the two as alternatives, and an id is
    /// seven hex characters, so the collision is a file literally named
    /// `95bae35` -- at which point reading it as a plan and failing to parse
    /// is a better outcome than silently treating a plan path as an id.
    pub plan_file: Option<PathBuf>,
    pub auto_approve: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_apply(args: &[String]) -> Result<ApplyArgs, String> {
    let mut out = ApplyArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_apply_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_apply_arg<'a>(
    out: &mut ApplyArgs,
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
        positional if Path::new(positional).is_file() => {
            out.plan_file = Some(PathBuf::from(positional));
        }
        id => out.ids.push(id.to_string()),
    }
    Ok(())
}

/// How consent for a mutating run was obtained.
///
/// An enum rather than two booleans, because `approved(true, false)` at a
/// call site says nothing about which is which -- the two-bool limit in
/// clippy.toml exists for exactly this, and the three states here are a
/// KIND rather than a pair of flags.
#[derive(Debug, PartialEq, Eq)]
pub enum Consent {
    /// `--auto-approve` was passed.
    Flag,
    /// A terminal was present and the user answered.
    Answered(String),
    /// No terminal and no flag. Silence is NOT consent: prompting into a
    /// pipe hangs a CI job until someone kills it, and proceeding without
    /// asking makes the DESTRUCTIVE path the quiet one (V20).
    Unattended,
}

/// Whether this run may mutate.
pub fn approved(consent: &Consent) -> Result<(), String> {
    match consent {
        Consent::Flag => Ok(()),
        Consent::Unattended => Err(NEEDS_APPROVAL.to_string()),
        Consent::Answered(answer) => match answer.trim() {
            "y" | "Y" | "yes" => Ok(()),
            _ => Err("cancelled".to_string()),
        },
    }
}

pub const NEEDS_APPROVAL: &str = "this would edit your corpus and stdin is not a terminal. \
Re-run with --auto-approve if that is what you want";

/// Run `apply` end to end.
///
/// Order matters and is the point: resolve the plan, CHECK IT IS FRESH
/// (V19), name every file (V7), ask (V20), and only then write.
pub fn apply_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_apply(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let loaded = load_corpus(&base, env)?;
    let path = ledger::path_in(&base);
    let held = ledger::load(&path).map_err(|error| error.to_string())?;
    let requested = resolve_plan(&args, &loaded, &held)?;
    refuse_if_stale(&requested.plan, &loaded)?;
    let outcome = staged_outcome(&requested, &held);
    let staged = Staged {
        held,
        outcome,
        path,
    };
    commit(&args, &requested.plan, &base, staged)
}

fn staged_outcome(
    requested: &Requested,
    held: &ledger::Ledger,
) -> apply::Outcome {
    let mut outcome = apply::preview(&requested.plan.steps, held);
    outcome.skipped.extend(requested.already.clone());
    outcome
}

/// A plan, plus the requested ids that were ALREADY extracted.
///
/// Those come back together because an already-extracted id has no
/// statement left in the corpus to plan from -- `apply` replaced it with a
/// pointer -- so looking it up fails, and failing is exactly what V13 says
/// must not happen.
struct Requested {
    plan: plan::Plan,
    already: Vec<String>,
}

struct Staged {
    held: ledger::Ledger,
    outcome: apply::Outcome,
    path: PathBuf,
}

/// Either the plan file the user handed us, or one built from ids.
fn resolve_plan(
    args: &ApplyArgs,
    loaded: &scan::Loaded,
    held: &ledger::Ledger,
) -> Result<Requested, String> {
    if let Some(path) = args.plan_file.as_deref() {
        return from_file(path);
    }
    if args.ids.is_empty() {
        return Err(NO_APPLY_INPUT.to_string());
    }
    let split = split_requested(&args.ids, loaded, held)?;
    let plan = plan::build(&split.chosen, &loaded.sources, &loaded.weights)
        .map_err(|error| error.to_string())?;
    Ok(Requested {
        plan,
        already: split.already,
    })
}

fn from_file(path: &Path) -> Result<Requested, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let plan = toml::from_str(&text).map_err(|error| error.to_string())?;
    Ok(Requested {
        plan,
        already: Vec::new(),
    })
}

/// Sort requested ids into "still in the corpus" and "already extracted".
///
/// An id the corpus does not hold but the LEDGER does is not an error: it
/// is work already done. `apply` replaced that statement with a pointer,
/// so there is nothing left to look up -- and V13 makes re-applying a
/// no-op at exit 0, not an "unknown id" at exit 2.
fn split_requested(
    ids: &[String],
    loaded: &scan::Loaded,
    held: &ledger::Ledger,
) -> Result<Split, String> {
    let mut out = Split::default();
    for id in ids {
        match one(&loaded.statements, id) {
            Ok(found) => out.chosen.push(found),
            Err(message) => out.already.push(known_or_fail(id, held, message)?),
        }
    }
    Ok(out)
}

#[derive(Default)]
struct Split {
    chosen: Vec<statement::Statement>,
    already: Vec<String>,
}

fn known_or_fail(
    id: &str,
    held: &ledger::Ledger,
    message: String,
) -> Result<String, String> {
    match held.find(id) {
        Some(row) => Ok(row.id.clone()),
        None => Err(message),
    }
}

pub const NO_APPLY_INPUT: &str =
    "`apply` needs ids or a plan file -- run `rekall plan` first";

/// V19. A plan whose sources moved would delete the wrong lines, so it is
/// REFUSED rather than adjusted. Re-planning is cheap and report-only.
fn refuse_if_stale(
    built: &plan::Plan,
    loaded: &scan::Loaded,
) -> Result<(), String> {
    let current: Vec<(String, Option<String>)> = loaded
        .sources
        .iter()
        .map(|(src, text)| (src.clone(), Some(text.clone())))
        .collect();
    let stale = plan::staleness(built, &current);
    if stale.is_empty() {
        return Ok(());
    }
    let reasons: Vec<String> = stale.iter().map(ToString::to_string).collect();
    Err(format!("refusing a stale plan: {}", reasons.join("; ")))
}

fn commit(
    args: &ApplyArgs,
    built: &plan::Plan,
    base: &Path,
    mut staged: Staged,
) -> Result<Output, String> {
    let pending = apply::pending(&built.steps, &staged.held);
    if pending.is_empty() {
        return report(
            &staged.outcome,
            args.json,
            vec!["nothing to do".to_string()],
            render_apply_human,
        );
    }
    let named = render_apply_human(&staged.outcome, &[]);
    approved(&consent_for(args.auto_approve, &named)?)?;
    let done = perform_apply(&pending, base, &mut staged)?;
    report(&staged.outcome, args.json, done, render_apply_human)
}

fn perform_apply(
    pending: &[plan::Step],
    base: &Path,
    staged: &mut Staged,
) -> Result<Vec<String>, String> {
    let mut done = write_all(pending, base)?;
    for step in pending {
        staged.held.record(apply::row_for(step, now()));
    }
    ledger::save(&staged.path, &staged.held)
        .map_err(|error| error.to_string())?;
    done.push(format!("recorded {} extraction(s)", pending.len()));
    Ok(done)
}

/// Unix seconds. Zero if the clock is unreadable -- a missing timestamp is
/// a worse ledger row than an old one, and neither is worth failing over.
pub(super) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// V7: name every file BEFORE asking, so the question is informed.
///
/// Takes the rendered names rather than an outcome, because V20 is ONE
/// rule and not one per verb: `apply` and `revert` both delete from the
/// user's private memory, and a second confirm gate written for the second
/// verb is a second place for the rule to be slightly wrong.
pub(super) fn consent_for(
    auto_approve: bool,
    named: &str,
) -> Result<Consent, String> {
    if auto_approve {
        return Ok(Consent::Flag);
    }
    eprint!("{named}");
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(Consent::Unattended);
    }
    ask_at_terminal()
}

/// Read an answer from anywhere.
///
/// Takes the reader rather than reaching for stdin, so the parsing of a
/// consent answer -- the part with a decision in it -- is testable without
/// a terminal. What is left needing one is the two lines below.
pub fn read_answer(
    input: &mut impl std::io::BufRead,
) -> Result<Consent, String> {
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    Ok(Consent::Answered(answer))
}

fn ask_at_terminal() -> Result<Consent, String> {
    eprint!("apply these changes? [y/N] ");
    read_answer(&mut std::io::stdin().lock())
}

fn write_all(
    pending: &[plan::Step],
    base: &Path,
) -> Result<Vec<String>, String> {
    let mut done = Vec::new();
    for step in pending {
        write_artifact(base, step)?;
        done.push(format!("wrote {}", step.artifact));
    }
    edit_sources(pending, base, &mut done)?;
    Ok(done)
}

fn write_artifact(base: &Path, step: &plan::Step) -> Result<(), String> {
    let path = base.join(&step.artifact);
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(&path, apply::artifact_text(step))
        .map_err(|error| error.to_string())?;
    if step.label.starts_with('M') {
        make_runnable(&path)?;
    }
    Ok(())
}

/// An `M` artifact arrives EXECUTABLE. A runner nothing can execute is a
/// rule with no runner, which V2 says gates nothing -- and the failure
/// would surface as "permission denied" at a tool call rather than as
/// something `check` could tell you about.
pub(super) fn make_runnable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| error.to_string())
}

/// One rewrite per source file, with every span for that file applied
/// together and bottom-up. Rewriting per STEP would shift the spans of the
/// steps that follow it.
fn edit_sources(
    pending: &[plan::Step],
    base: &Path,
    done: &mut Vec<String>,
) -> Result<(), String> {
    for src in sources_touched(pending) {
        let steps: Vec<plan::Step> =
            pending.iter().filter(|s| s.src == src).cloned().collect();
        let path = base.join(&src);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())?;
        let edited = apply::splice_all(&text, &steps).ok_or_else(|| {
            format!("{src}: a span no longer fits -- re-run `rekall plan`")
        })?;
        std::fs::write(&path, edited).map_err(|error| error.to_string())?;
        done.push(format!("edited {src}"));
    }
    Ok(())
}

fn sources_touched(pending: &[plan::Step]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for step in pending {
        if !out.contains(&step.src) {
            out.push(step.src.clone());
        }
    }
    out
}

#[must_use]
pub fn render_apply_human(outcome: &apply::Outcome, done: &[String]) -> String {
    let mut out = String::new();
    for path in &outcome.writes {
        out.push_str(&format!("write   {path}\n"));
    }
    for path in &outcome.edits {
        out.push_str(&format!("edit    {path}\n"));
    }
    for id in &outcome.skipped {
        out.push_str(&format!("skip    {id} (already extracted)\n"));
    }
    for line in done {
        out.push_str(&format!("done    {line}\n"));
    }
    out
}
