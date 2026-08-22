use super::{Env, Format, Output, need, parse_format};
use crate::init;
use std::path::PathBuf;
/// `init` flags. `--force` is separate from `--auto-approve` on purpose:
/// this writes its OWN config, never the corpus, so it is not the
/// destructive path V20 guards.
#[derive(Debug, Default)]
pub struct InitArgs {
    pub force: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_init(args: &[String]) -> Result<InitArgs, String> {
    let mut out = InitArgs::default();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        apply_init_flag(&mut out, flag, &mut rest)?;
    }
    Ok(out)
}

fn apply_init_flag<'a>(
    out: &mut InitArgs,
    flag: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match flag {
        "--force" => out.force = true,
        "--format" => {
            out.json = parse_format(&need(flag, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(flag, rest)?)),
        other => return Err(format!("unknown flag `{other}`")),
    }
    Ok(())
}

/// Run `init` end to end.
pub fn init_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_init(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let report = init::run(&base, env.home.as_deref(), args.force)
        .map_err(|error| error.to_string())?;
    Ok(Output {
        text: render_init(&report, args.json)?,
        warnings: Vec::new(),
    })
}

fn render_init(report: &init::Report, json: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string_pretty(report)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(init::render_human(report))
}
