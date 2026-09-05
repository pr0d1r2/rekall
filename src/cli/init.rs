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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::scan::scan_command;
    use crate::cli::testing::*;
    use crate::cli::{Action, USAGE_EXIT, decide, perform};

    #[test]
    fn init_is_dispatched() {
        assert_eq!(
            decide(&args(&["init", "--force"])),
            Action::Init(args(&["--force"]))
        );
    }

    #[test]
    fn init_flags_parse() {
        assert_eq!(
            parse_init(&args(&["--force", "--format", "json"]))
                .map(|parsed| (parsed.force, parsed.json))
                .ok(),
            Some((true, true))
        );
    }

    #[test]
    fn an_unknown_init_flag_is_an_error() {
        assert!(parse_init(&args(&["--nope"])).is_err());
        assert!(parse_init(&args(&["-C"])).is_err());
    }

    #[test]
    fn init_writes_a_config_and_names_it() {
        let dir = PathBuf::from("target").join("cli-init").join("fresh");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("CLAUDE.md"), "- a rule\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let text = init_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("root   CLAUDE.md"), "text was {text}");
    }

    /// init then scan, with nothing hand-written in between. That is the
    /// cold start, and it is the whole reason this verb exists.
    #[test]
    fn init_then_scan_works_with_no_hand_written_config() {
        let dir = PathBuf::from("target").join("cli-init").join("cold-start");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ =
            std::fs::write(dir.join("CLAUDE.md"), "- never commit to `main`\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert!(init_command(&flags, &env()).is_ok());
        let scanned = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(scanned.contains("M1"), "scan output was {scanned}");
    }

    #[test]
    fn init_refusing_to_clobber_exits_two() {
        let dir = PathBuf::from("target").join("cli-init").join("clobber");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("rekall.toml"), "[sources]\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert_eq!(perform(Action::Init(flags), &env()), USAGE_EXIT);
    }

    #[test]
    fn a_successful_init_exits_zero() {
        let dir = PathBuf::from("target").join("cli-init").join("exit-zero");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert_eq!(perform(Action::Init(flags), &env()), 0);
    }

    #[test]
    fn init_json_is_parseable() {
        let dir = PathBuf::from("target").join("cli-init").join("json");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let flags = args(&["-C", &dir.to_string_lossy(), "--format", "json"]);
        let text = init_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "text was {text}"
        );
    }
}
