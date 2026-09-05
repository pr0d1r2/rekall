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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::scan::scan_command;
    use crate::cli::testing::*;
    use crate::cli::{Action, USAGE_EXIT, decide, perform};

    use std::path::{Path, PathBuf};

    fn show_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-show").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "- never commit to `main`\n\n- when writing tests, prefer tables\n",
        );
        dir
    }

    #[test]
    fn show_is_dispatched_with_its_id() {
        assert_eq!(
            decide(&args(&["show", "abc"])),
            Action::Show(args(&["abc"]))
        );
    }

    #[test]
    fn show_parses_an_id_and_flags_in_any_order() {
        assert_eq!(
            parse_show(&args(&["--format", "json", "abc"]))
                .map(|parsed| (parsed.id, parsed.json))
                .ok(),
            Some((Some("abc".to_string()), true))
        );
    }

    /// A second positional is a MISTAKE. `show a b` most likely means the
    /// shell split something, and quietly showing only `a` would answer a
    /// question nobody asked.
    #[test]
    fn a_second_id_is_an_error_not_a_second_lookup() {
        assert!(parse_show(&args(&["abc", "def"])).is_err());
    }

    #[test]
    fn show_without_an_id_says_where_to_get_one() {
        let message =
            show_command(&args(&[]), &env()).err().unwrap_or_default();
        assert!(message.contains("rekall scan"), "message was {message}");
    }

    #[test]
    fn show_prints_the_verbatim_statement() {
        let dir = show_project("verbatim");
        let text = show_first(&dir);
        assert!(text.contains("never commit"), "text was {text}");
        assert!(text.contains("class    M1"), "text was {text}");
    }

    /// Scans, takes the first id from the output, and shows it -- the
    /// copy-an-id-from-scan path a user actually takes.
    fn show_first(dir: &Path) -> String {
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.push(id.to_string());
        show_command(&show_flags, &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }

    #[test]
    fn an_ambiguous_prefix_names_the_candidates() {
        let dir = show_project("ambiguous");
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(String::new());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(
            message.contains("matches 2 statements"),
            "message was {message}"
        );
        assert!(
            message.contains("Use more characters"),
            "message was {message}"
        );
    }

    #[test]
    fn an_unknown_id_says_so() {
        let dir = show_project("unknown");
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push("zzzzzzz".to_string());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    #[test]
    fn a_failed_show_exits_two() {
        assert_eq!(perform(Action::Show(args(&["zzz"])), &env()), USAGE_EXIT);
    }

    #[test]
    fn an_unknown_show_flag_is_an_error() {
        assert!(parse_show(&args(&["--nope"])).is_err());
        assert!(parse_show(&args(&["--format"])).is_err());
    }

    #[test]
    fn show_json_is_parseable_and_carries_the_text() {
        let dir = show_project("json");
        let text = show_first_as(&dir, &["--format", "json"]);
        let parsed = serde_json::from_str::<serde_json::Value>(&text).ok();
        assert!(parsed.is_some(), "text was {text}");
        assert!(text.contains("\"text\""), "text was {text}");
    }

    fn show_first_as(dir: &Path, extra: &[&str]) -> String {
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.extend(args(extra));
        show_flags.push(id.to_string());
        show_command(&show_flags, &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }

    #[test]
    fn a_successful_show_exits_zero() {
        let dir = show_project("exit-zero");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.push(id.to_string());
        assert_eq!(perform(Action::Show(show_flags), &env()), 0);
    }

    #[test]
    fn show_without_configured_roots_says_what_to_run() {
        let mut flags = args(&["-C", "/tmp"]);
        flags.push("abc".to_string());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(message.contains("rekall init"), "message was {message}");
    }
}
