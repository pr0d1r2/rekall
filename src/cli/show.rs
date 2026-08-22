use super::{
    Env, Format, NO_SOURCES, Output, Resolved, need, parse_format, resolve,
};
use crate::{scan, show};
use std::path::{Path, PathBuf};
/// `show` takes ONE id (or an unambiguous prefix of one) plus the usual
/// format and directory flags.
#[derive(Debug, Default)]
pub struct ShowArgs {
    pub id: Option<String>,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_show(args: &[String]) -> Result<ShowArgs, String> {
    let mut out = ShowArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_show_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_show_arg<'a>(
    out: &mut ShowArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        id => return set_id(out, id),
    }
    Ok(())
}

/// A second positional is a MISTAKE, not a second lookup. `show a b` most
/// likely means the shell split something, and quietly showing only `a`
/// would answer a question nobody asked.
fn set_id(out: &mut ShowArgs, id: &str) -> Result<(), String> {
    if out.id.is_some() {
        return Err(format!("`show` takes one id, got a second: `{id}`"));
    }
    out.id = Some(id.to_string());
    Ok(())
}

/// Run `show` end to end.
pub fn show_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_show(flags)?;
    let id = args.id.clone().ok_or_else(|| NO_ID.to_string())?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let resolved = resolve(&base)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    render_lookup(&find(&resolved, &base, env, &id)?, &id, args.json)
}

pub const NO_ID: &str = "`show` needs an id -- copy one from `rekall scan`";

fn find(
    resolved: &Resolved,
    base: &Path,
    env: &Env,
    id: &str,
) -> Result<show::Lookup, String> {
    let at = scan::Corpus {
        roots: &resolved.roots,
        globs: &resolved.globs,
        home: env.home.as_deref(),
        base,
        weights: &resolved.weights,
    };
    show::lookup(&at, id).map_err(|error| error.to_string())
}

fn render_lookup(
    found: &show::Lookup,
    id: &str,
    json: bool,
) -> Result<Output, String> {
    match found {
        show::Lookup::Unique(found) => Ok(Output {
            text: render_found(found, json)?,
            warnings: Vec::new(),
        }),
        show::Lookup::Ambiguous(ids) => Err(format!(
            "`{id}` matches {} statements: {}. Use more characters.",
            ids.len(),
            ids.join(", ")
        )),
        show::Lookup::Missing => Err(format!("no statement matches `{id}`")),
    }
}

fn render_found(found: &show::Found, json: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string_pretty(found)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(show::render_human(found))
}
