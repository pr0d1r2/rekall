use super::{
    Env, Format, Output, choose, load_corpus, need, net_reclaim, parse_format,
};
use crate::plan;
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
    let chosen = choose(&loaded.statements, &args.ids)?;
    let mut built = plan::build(&chosen, &loaded.sources, &loaded.weights)
        .map_err(|error| error.to_string())?;
    fill_net(&mut built);
    emit_plan(&built, &args, &base)
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
