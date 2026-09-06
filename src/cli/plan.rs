use super::{
    Env, Format, Output, choose, load_corpus, need, net_reclaim, parse_format,
};
use crate::{hook, plan, scan};
use std::path::{Path, PathBuf};
/// `plan` takes one or more ids plus `--out` and the usual flags.
#[derive(Debug, Default)]
pub struct PlanArgs {
    pub ids: Vec<String>,
    pub out: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_plan(args: &[String]) -> Result<PlanArgs, String> {
    let mut out = PlanArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_plan_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_plan_arg<'a>(
    out: &mut PlanArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json
        }
        "--out" => out.out = Some(PathBuf::from(need(arg, rest)?)),
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        id => out.ids.push(id.to_string()),
    }
    Ok(())
}

/// Run `plan` end to end.
pub fn plan_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_plan(flags)?;
    if args.ids.is_empty() {
        return Err(NO_PLAN_IDS.to_string());
    }
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let loaded = load_corpus(&base, env)?;
    let mut built = built_from(&args, &loaded, env, &base)?;
    fill_net(&mut built);
    emit_plan(&built, &args, &base)
}

fn built_from(
    args: &PlanArgs,
    loaded: &scan::Loaded,
    at: &Env,
    base: &Path,
) -> Result<plan::Plan, String> {
    let chosen = choose(&loaded.statements, &args.ids)?;
    let built = plan::build(
        &chosen,
        &loaded.sources,
        &loaded.weights,
        hook::wired(base),
    )
    .map_err(|error| error.to_string())?;
    refuse_unresolvable(&built, base, at.home.as_deref())?;
    Ok(built)
}

/// V59: a plan does not promise a `delete` that `apply` will refuse.
///
/// `plan` prints `delete <src>:<span>` for every row, and `apply` is the verb
/// that has to find that file. Where it cannot -- a `~/` source with no HOME
/// to expand it against -- the two disagree, and the user meets the
/// disagreement as a failure after approving the move rather than as a line
/// before it. That is `B13` in a place where the file is somebody's memory.
fn refuse_unresolvable(
    built: &plan::Plan,
    base: &Path,
    home: Option<&str>,
) -> Result<(), String> {
    for step in &built.steps {
        if scan::resolve_name(&step.src, base, home).is_none() {
            return Err(format!(
                "{} cannot be resolved: it is under your home directory and \
                 HOME is not set, so `rekall apply` could not edit it. This \
                 plan is not offered rather than offered and refused later",
                step.src
            ));
        }
    }
    Ok(())
}

/// V39: name the net BEFORE the move, so a losing extraction is visible
/// while it is still hypothetical.
fn fill_net(built: &mut plan::Plan) {
    let ids: Vec<String> =
        built.steps.iter().map(|step| step.id.clone()).collect();
    let texts: Vec<String> =
        built.steps.iter().map(|step| step.text.clone()).collect();
    for (step, net) in built.steps.iter_mut().zip(net_reclaim(&ids, &texts)) {
        step.net = net;
    }
}

pub const NO_PLAN_IDS: &str =
    "`plan` needs at least one id -- copy them from `rekall scan`";

fn emit_plan(
    built: &plan::Plan,
    args: &PlanArgs,
    base: &Path,
) -> Result<Output, String> {
    let mut warnings = Vec::new();
    if let Some(path) = args.out.as_deref() {
        // ANCHORED to `-C`, like every other path this command touches. A
        // relative `--out` resolved against the PROCESS directory would
        // write the plan somewhere the project it describes cannot see --
        // the same split `-C` already had to fix for corpus roots.
        let path = base.join(path);
        write_plan(&path, built)?;
        warnings.push(format!("wrote plan to {}", path.display()));
    }
    Ok(Output {
        text: render_plan(built, args.json)?,
        warnings,
    })
}

/// The path is always base-joined by the caller, so it always has a
/// parent and the directory creation needs no "if there is one" branch.
fn write_plan(path: &Path, built: &plan::Plan) -> Result<(), String> {
    let text =
        toml::to_string_pretty(built).map_err(|error| error.to_string())?;
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(path, text).map_err(|error| error.to_string())
}

fn render_plan(built: &plan::Plan, json: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string_pretty(built)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(plan::render_human(built))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::scan::scan_command;
    use crate::cli::testing::*;
    use crate::cli::{Action, Output, USAGE_EXIT, decide, perform};
    use crate::plan;
    use std::path::Path;

    fn plan_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        plan_command(&flags, &env())
    }

    fn first_scan_id(dir: &Path) -> String {
        scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
            .map(|out| out.text)
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn plan_is_dispatched() {
        assert_eq!(
            decide(&args(&["plan", "abc"])),
            Action::Plan(args(&["abc"]))
        );
    }

    #[test]
    fn plan_without_ids_says_where_to_get_them() {
        let message = plan_in(Path::new("."), &[]).err().unwrap_or_default();
        assert!(message.contains("rekall scan"), "message was {message}");
    }

    #[test]
    fn plan_names_the_delete_the_write_and_the_wiring() {
        let dir = plan_project("human");
        let id = first_scan_id(&dir);
        let text = plan_in(&dir, &[&id])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("delete  CLAUDE.md:1-1"), "{text}");
        assert!(text.contains("write   .rekall/rules/"), "{text}");
        assert!(text.contains("wire "), "{text}");
    }

    /// Every id resolves BEFORE any step is built, so a typo in the second
    /// id does not hand back a partial plan for the first.
    #[test]
    fn one_bad_id_produces_no_plan_at_all() {
        let dir = plan_project("partial");
        let id = first_scan_id(&dir);
        let outcome = plan_in(&dir, &[&id, "zzzzzzz"]);
        assert!(outcome.is_err());
    }

    #[test]
    fn planning_an_unclassified_statement_is_refused() {
        let dir = plan_project("unclassified");
        let listed =
            scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
                .map(|out| out.text)
                .unwrap_or_default();
        let last = listed.lines().last().unwrap_or_default();
        let id = last.split_whitespace().next().unwrap_or_default();
        let message = plan_in(&dir, &[id]).err().unwrap_or_default();
        assert!(message.contains("unclassified"), "message was {message}");
    }

    #[test]
    fn an_unknown_plan_flag_is_an_error() {
        assert!(parse_plan(&args(&["--nope"])).is_err());
        assert!(parse_plan(&args(&["--out"])).is_err());
    }

    /// `--out` makes the plan an ARTIFACT: reviewable, diffable, and
    /// committable before a byte of corpus moves.
    #[test]
    fn out_writes_a_plan_file_that_parses_back() {
        let dir = plan_project("out");
        let id = first_scan_id(&dir);
        let result = plan_in(&dir, &[&id, "--out", "nested/x.plan"]);
        assert!(result.is_ok(), "{result:?}");
        let text = std::fs::read_to_string(dir.join("nested").join("x.plan"))
            .unwrap_or_default();
        assert!(
            toml::from_str::<plan::Plan>(&text).is_ok(),
            "plan was {text}"
        );
    }

    #[test]
    fn writing_a_plan_is_reported_as_a_warning_not_silently() {
        let dir = plan_project("reported");
        let id = first_scan_id(&dir);
        let warnings = plan_in(&dir, &[&id, "--out", "x.plan"])
            .map(|result| result.warnings)
            .unwrap_or_default();
        assert!(
            warnings.first().is_some_and(|w| w.contains("wrote plan")),
            "{warnings:?}"
        );
    }

    #[test]
    fn plan_json_is_parseable() {
        let dir = plan_project("json");
        let id = first_scan_id(&dir);
        let text = plan_in(&dir, &[&id, "--format", "json"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }

    #[test]
    fn a_successful_plan_exits_zero() {
        let dir = plan_project("exit-zero");
        let id = first_scan_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        assert_eq!(perform(Action::Plan(flags), &env()), 0);
    }

    #[test]
    fn a_failed_plan_exits_two() {
        assert_eq!(
            perform(Action::Plan(args(&["zzzzzzz"])), &env()),
            USAGE_EXIT
        );
    }

    #[test]
    fn a_plan_that_cannot_be_written_is_an_error() {
        let dir = plan_project("unwritable");
        let id = first_scan_id(&dir);
        let outcome =
            plan_in(&dir, &[&id, "--out", "/definitely/not/writable/x.plan"]);
        assert!(outcome.is_err());
    }

    #[test]
    fn an_ambiguous_plan_id_names_the_candidates() {
        let dir = plan_project("ambiguous");
        let message = plan_in(&dir, &[""]).err().unwrap_or_default();
        assert!(
            message.contains("matches 2 statements"),
            "message was {message}"
        );
    }

    /// The warning that a plan file was written goes to STDERR and the
    /// plan to stdout, so `rekall plan --out p | less` still shows the plan
    /// and the write is still announced.
    #[test]
    fn writing_a_plan_still_exits_zero() {
        let dir = plan_project("out-exit");
        let id = first_scan_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.extend(args(&["--out", "x.plan"]));
        assert_eq!(perform(Action::Plan(flags), &env()), 0);
    }

    /// `--out` is anchored to `-C` like every other path. A relative
    /// `--out` resolved against the PROCESS directory would drop the plan
    /// somewhere the project it describes cannot see.
    #[test]
    fn a_relative_out_lands_inside_the_project() {
        let dir = plan_project("relative-out");
        let id = first_scan_id(&dir);
        assert!(plan_in(&dir, &[&id, "--out", "x.plan"]).is_ok());
        assert!(
            dir.join("x.plan").is_file(),
            "plan did not land in the project"
        );
    }

    /// B13's shape where the file is somebody's memory: a corpus path
    /// under `~` cannot be resolved with HOME unset, so the plan is not
    /// offered rather than offered and refused at `apply`.
    #[test]
    fn a_plan_it_could_not_execute_is_not_offered() {
        let built = plan::Plan {
            steps: vec![plan::Step {
                src: "~/CLAUDE.md".to_string(),
                ..plan::Step::default()
            }],
            ..plan::Plan::default()
        };
        let why = refuse_unresolvable(&built, Path::new("."), None)
            .err()
            .unwrap_or_default();
        assert!(why.contains("HOME is not set"), "{why}");
        assert!(why.contains("not offered"), "{why}");
    }
}
