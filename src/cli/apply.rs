use super::{
    Env, Format, Output, load_corpus, need, one, parse_format, report,
};
use crate::{apply, hook, ledger, plan, scan, statement};
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
    let requested = resolve_plan(&args, &loaded, &held, hook::wired(&base))?;
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
    delivered: bool,
) -> Result<Requested, String> {
    if let Some(path) = args.plan_file.as_deref() {
        return from_file(path, delivered);
    }
    if args.ids.is_empty() {
        return Err(NO_APPLY_INPUT.to_string());
    }
    let split = split_requested(&args.ids, loaded, held)?;
    let plan =
        plan::build(&split.chosen, &loaded.sources, &loaded.weights, delivered)
            .map_err(|error| error.to_string())?;
    Ok(Requested {
        plan,
        already: split.already,
    })
}

/// A `runner` names an existing GATE STEP, and only an `M` rule has one.
///
/// Refusing beats ignoring: a field silently dropped reads, to whoever
/// filled it in, exactly like a field that was honoured (V41, V28).
fn refuse_a_runner_on_a_skill(plan: &plan::Plan) -> Result<(), String> {
    for step in &plan.steps {
        if step.runner.is_empty() || step.label.starts_with('M') {
            continue;
        }
        return Err(format!(
            "step {} is {} and carries runner `{}`. A runner names a GATE STEP, which only an `M` rule has. Clear the field, or take the statement back through `rekall plan` if the class is wrong.",
            step.id, step.label, step.runner
        ));
    }
    Ok(())
}

fn from_file(path: &Path, delivered: bool) -> Result<Requested, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut plan: plan::Plan =
        toml::from_str(&text).map_err(|error| error.to_string())?;
    plan::retire_stale_wiring(&mut plan, delivered);
    refuse_a_runner_on_a_skill(&plan)?;
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
    publish(base, step)
}

/// Link the artifact into a host's own skills directory where one exists.
///
/// A SYMLINK and not a copy: Claude Code follows a `<skill-name>` link,
/// reads the target, and loads the skill once however many paths reach it
/// (`.:R17`). So the canonical file stays single under `.rekall/` -- V1
/// holds, because a link is not a copy and cannot drift from what it
/// points at.
///
/// Only where the host directory ALREADY exists. Creating `.claude/` in a
/// repo that has none would be this crate deciding which agent someone
/// runs, which is exactly what V47 refuses to guess at.
fn publish(base: &Path, step: &plan::Step) -> Result<(), String> {
    publish_artifact(base, &step.label, &step.artifact)
}

/// The same link, from a LEDGER ROW rather than a plan step.
///
/// `issue` republishes rows that already exist (`src/issue:T65`), and a
/// second implementation of "where does the host link go" is how the two
/// paths end up disagreeing about it.
pub(super) fn publish_artifact(
    base: &Path,
    label: &str,
    artifact: &str,
) -> Result<(), String> {
    if !label.starts_with('S') {
        return Ok(());
    }
    let Some(slug) = apply::skill_slug(artifact) else {
        return Ok(());
    };
    let hosts = base.join(".claude").join("skills");
    if !hosts.is_dir() {
        return Ok(());
    }
    link_skill(
        &hosts.join(&slug),
        &base.join(".rekall").join("skills").join(&slug),
    )
}

fn link_skill(link: &Path, target: &Path) -> Result<(), String> {
    if link.exists() || link.symlink_metadata().is_ok() {
        return Ok(());
    }
    let rel = Path::new("..").join("..").join(".rekall").join("skills");
    let _ = target;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(rel.join(name_of(link)), link)
            .map_err(|why| format!("cannot link `{}`: {why}", link.display()))
    }
    #[cfg(not(unix))]
    {
        Ok(())
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::plan::plan_command;
    use crate::cli::testing::*;
    use crate::cli::{Action, USAGE_EXIT, decide, perform};
    use crate::{ledger, plan};
    use std::path::{Path, PathBuf};

    fn apply_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-apply").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        // A GENEROUS runner bound, and B5 is exactly why. This suite runs
        // ~490 tests in parallel, many spawning `itok`, which is enough
        // contention to blow a 2s WALL-CLOCK limit on a script whose body
        // is `echo`. That is not a test artifact -- it is the production
        // failure in miniature, so the tests answer it the way a user on a
        // loaded box would: by setting the knob.
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n\n[triggers]\nrunner_timeout_ms = 30000\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n\n- background prose\n",
        );
        dir
    }

    #[test]
    fn apply_is_dispatched() {
        assert_eq!(
            decide(&args(&["apply", "abc"])),
            Action::Apply(args(&["abc"]))
        );
    }

    /// V20: off a tty, silence is not consent.
    #[test]
    fn without_a_terminal_and_without_the_flag_apply_refuses() {
        assert_eq!(
            approved(&Consent::Unattended),
            Err(NEEDS_APPROVAL.to_string())
        );
        assert!(NEEDS_APPROVAL.contains("--auto-approve"));
    }

    #[test]
    fn the_flag_is_consent_and_a_no_is_not() {
        assert!(approved(&Consent::Flag).is_ok());
        assert!(approved(&Consent::Answered("y\n".to_string())).is_ok());
        assert!(approved(&Consent::Answered("yes".to_string())).is_ok());
        assert!(approved(&Consent::Answered("n".to_string())).is_err());
        assert!(approved(&Consent::Answered(String::new())).is_err());
    }

    /// Bare Enter means NO. The prompt reads `[y/N]`, and a destructive
    /// default that triggers on a stray keypress is not a confirmation.
    #[test]
    fn an_empty_answer_cancels() {
        assert_eq!(
            approved(&Consent::Answered("\n".to_string())),
            Err("cancelled".to_string())
        );
    }

    #[test]
    fn apply_without_ids_or_a_plan_says_what_to_run() {
        let dir = apply_project("no-input");
        let message = apply_in(&dir, &["--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("rekall plan"), "message was {message}");
    }

    /// V1: the span is DELETED and a pointer left. Both halves asserted --
    /// a copy would leave the context cost unpaid.
    #[test]
    fn apply_moves_the_statement_and_leaves_a_pointer() {
        let dir = apply_project("moves");
        let id = rule_id(&dir);
        assert!(apply_in(&dir, &[&id, "--auto-approve"]).is_ok());
        let corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert!(
            !corpus.contains("never commit"),
            "source survived: {corpus}"
        );
        assert!(corpus.contains("<!-- rekall"), "no pointer: {corpus}");
        assert!(
            corpus.contains("- background prose"),
            "unrelated prose lost: {corpus}"
        );
    }

    #[test]
    fn apply_writes_the_artifact_and_records_the_ledger() {
        let dir = apply_project("artifact");
        let id = rule_id(&dir);
        assert!(apply_in(&dir, &[&id, "--auto-approve"]).is_ok());
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        let artifact = held
            .find(&id)
            .map(|row| row.artifact.clone())
            .unwrap_or_default();
        assert!(
            dir.join(&artifact).is_file(),
            "artifact missing: {artifact}"
        );
        assert_eq!(held.extracted.len(), 1);
    }

    /// V9: the ledger keeps the ORIGINAL TEXT, which is what makes `revert`
    /// a replay rather than a rewrite.
    #[test]
    fn the_ledger_keeps_the_original_text_verbatim() {
        let dir = apply_project("verbatim");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(
            held.find(&id).map(|row| row.text.clone()),
            Some("- never commit to `main`".to_string())
        );
    }

    /// V13: applying an id that is already extracted is a NO-OP at exit 0.
    /// Its statement is gone from the corpus -- `apply` replaced it with a
    /// pointer -- so the lookup that would fail must consult the ledger.
    #[test]
    fn re_applying_an_extracted_id_is_a_no_op() {
        let dir = apply_project("idempotent");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let before =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let second = apply_in(&dir, &[&id, "--auto-approve"]);
        assert!(second.is_ok(), "{second:?}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(before, after, "a second apply changed the corpus");
    }

    #[test]
    fn re_applying_exits_zero_and_says_it_skipped() {
        let dir = apply_project("idempotent-exit");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let text = apply_in(&dir, &[&id, "--auto-approve"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("already extracted"), "{text}");
    }

    #[test]
    fn an_id_in_neither_the_corpus_nor_the_ledger_is_still_an_error() {
        let dir = apply_project("unknown");
        let message = apply_in(&dir, &["zzzzzzz", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    /// V19: the whole reason the fingerprint exists.
    #[test]
    fn a_plan_applied_after_the_corpus_moved_is_refused() {
        let dir = apply_project("stale");
        let saved = save_plan(&dir);
        shift_corpus(&dir);
        let message = apply_in(&dir, &[&saved, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("refusing a stale plan"),
            "message was {message}"
        );
    }

    fn save_plan(dir: &Path) -> String {
        let id = rule_id(dir);
        let _ = plan_command(
            &args(&["-C", &dir.to_string_lossy(), &id, "--out", "p.plan"]),
            &env(),
        );
        dir.join("p.plan").to_string_lossy().to_string()
    }

    /// Insert lines ABOVE every span, which is the edit that moves them.
    fn shift_corpus(dir: &Path) {
        let path = dir.join("CLAUDE.md");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::write(&path, format!("# shifted\n\n{text}"));
    }

    #[test]
    fn a_fresh_plan_file_applies() {
        let dir = apply_project("from-plan");
        let id = rule_id(&dir);
        let _ = plan_command(
            &args(&["-C", &dir.to_string_lossy(), &id, "--out", "p.plan"]),
            &env(),
        );
        let plan_path = dir.join("p.plan");
        let result =
            apply_in(&dir, &[&plan_path.to_string_lossy(), "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn an_unknown_apply_flag_is_an_error() {
        assert!(parse_apply(&args(&["--nope"])).is_err());
        assert!(parse_apply(&args(&["--format"])).is_err());
    }

    #[test]
    fn a_successful_apply_exits_zero() {
        let dir = apply_project("exit-zero");
        let id = rule_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.push("--auto-approve".to_string());
        assert_eq!(perform(Action::Apply(flags), &env()), 0);
    }

    #[test]
    fn a_failed_apply_exits_two() {
        assert_eq!(
            perform(
                Action::Apply(args(&["zzzzzzz", "--auto-approve"])),
                &env()
            ),
            USAGE_EXIT
        );
    }

    #[test]
    fn apply_json_is_parseable() {
        let dir = apply_project("json");
        let id = rule_id(&dir);
        let text = apply_in(&dir, &[&id, "--auto-approve", "--format", "json"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }

    /// Without the flag and without a terminal, apply REFUSES rather than
    /// proceeding. Tests are never a tty, so this is the real path CI takes.
    #[test]
    fn apply_without_approval_refuses_and_changes_nothing() {
        let dir = apply_project("unattended");
        let id = rule_id(&dir);
        let before =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let message = apply_in(&dir, &[&id]).err().unwrap_or_default();
        assert!(message.contains("--auto-approve"), "message was {message}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(before, after, "a refused apply edited the corpus");
    }

    /// A plan whose FINGERPRINT still matches but whose span does not fit
    /// -- a hand-edited or corrupt plan. The fingerprint check passes, so
    /// the splice is the last thing standing between that plan and the
    /// wrong bytes. It refuses too.
    #[test]
    fn a_plan_with_an_impossible_span_is_refused_at_the_splice() {
        let dir = apply_project("bad-span");
        let path = forge_plan(&dir, 999);
        let message = apply_in(&dir, &[&path, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("no longer fits"), "message was {message}");
    }

    /// A plan whose fingerprint is CORRECT but whose span is not. Hand
    /// editing a plan, or a corrupt one, gets past V19 -- so the splice is
    /// the last thing standing between it and the wrong bytes.
    fn forge_plan(dir: &Path, line: usize) -> String {
        let corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let forged = format!(
            "format = 1\n\n[[fingerprint]]\nsrc = \"CLAUDE.md\"\ndigest = \"{}\"\n\n\
             [[steps]]\nid = \"forged1\"\nsrc = \"CLAUDE.md\"\nline_start = {line}\n\
             line_end = {line}\ntext = \"x\"\nlabel = \"M1\"\n\
             artifact = \".rekall/rules/x.sh\"\nwiring = \"w\"\n",
            plan::digest_of(&corpus)
        );
        let path = dir.join("forged.plan");
        let _ = std::fs::write(&path, forged);
        path.to_string_lossy().to_string()
    }

    #[test]
    fn apply_reports_what_it_did_on_stderr_in_json_mode() {
        let dir = apply_project("json-warnings");
        let id = rule_id(&dir);
        let warnings =
            apply_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.warnings)
                .unwrap_or_default();
        assert!(
            warnings.iter().any(|w| w.contains("wrote ")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("edited ")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_json_apply_exits_zero_and_prints_its_warnings() {
        let dir = apply_project("json-exit");
        let id = rule_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.extend(args(&["--auto-approve", "--format", "json"]));
        assert_eq!(perform(Action::Apply(flags), &env()), 0);
    }

    #[test]
    fn an_answer_is_read_from_any_reader() {
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        assert_eq!(
            read_answer(&mut input).ok(),
            Some(Consent::Answered("y\n".to_string()))
        );
    }

    #[test]
    fn an_answer_that_is_read_is_then_judged() {
        let mut yes = std::io::Cursor::new(b"yes\n".to_vec());
        let mut no = std::io::Cursor::new(b"\n".to_vec());
        assert!(read_answer(&mut yes).and_then(|c| approved(&c)).is_ok());
        assert!(read_answer(&mut no).and_then(|c| approved(&c)).is_err());
    }
    fn skill_step(slug: &str) -> plan::Step {
        plan::Step {
            id: "abc1234".to_string(),
            src: "CLAUDE.md".to_string(),
            line_start: 1,
            line_end: 1,
            text: "- when editing `.rs`, prefer modules".to_string(),
            label: "S1".to_string(),
            artifact: format!(".rekall/skills/{slug}/SKILL.md"),
            runner: String::new(),
            wiring: String::new(),
            net: None,
        }
    }

    fn host_dir(name: &str, make: bool) -> PathBuf {
        let at = PathBuf::from("target").join("test-publish").join(name);
        let _ = std::fs::remove_dir_all(&at);
        let _ = std::fs::create_dir_all(at.join(".rekall").join("skills"));
        if make {
            let _ = std::fs::create_dir_all(at.join(".claude").join("skills"));
        }
        at
    }

    /// V48: a link, not a copy. One file under `.rekall/`, reachable from
    /// the host's directory, so Claude Code indexes it and nothing drifts.
    #[test]
    fn a_skill_is_linked_into_a_host_directory_that_exists() {
        let at = host_dir("linked", true);
        let step = skill_step("some-rule");
        assert!(publish(&at, &step).is_ok());
        let link = at.join(".claude").join("skills").join("some-rule");
        assert!(link.symlink_metadata().is_ok(), "no link at {link:?}");
        assert!(
            std::fs::read_link(&link)
                .map(|p| p.ends_with("skills/some-rule"))
                .unwrap_or_default(),
            "link does not point at the canonical store"
        );
        let _ = std::fs::remove_dir_all(&at);
    }

    /// Creating `.claude/` in a repo that has none would be this crate
    /// deciding which agent someone runs, which V47 refuses to guess.
    #[test]
    fn no_host_directory_means_no_link_and_no_error() {
        let at = host_dir("nohost", false);
        assert!(publish(&at, &skill_step("some-rule")).is_ok());
        assert!(!at.join(".claude").exists(), "a host dir was invented");
        let _ = std::fs::remove_dir_all(&at);
    }

    /// A mechanical rule has no host directory to be indexed by, so it is
    /// never published -- only `S` artifacts are.
    #[test]
    fn a_mechanical_rule_is_not_published() {
        let at = host_dir("mechanical", true);
        let mut step = skill_step("a-rule");
        step.label = "M1".to_string();
        step.artifact = ".rekall/rules/a-rule.sh".to_string();
        assert!(publish(&at, &step).is_ok());
        assert!(!at.join(".claude").join("skills").join("a-rule").exists());
        let _ = std::fs::remove_dir_all(&at);
    }

    #[test]
    fn publishing_twice_leaves_the_first_link_alone() {
        let at = host_dir("twice", true);
        let step = skill_step("some-rule");
        assert!(publish(&at, &step).is_ok());
        assert!(
            publish(&at, &step).is_ok(),
            "a second publish must not fail"
        );
        let _ = std::fs::remove_dir_all(&at);
    }
}
