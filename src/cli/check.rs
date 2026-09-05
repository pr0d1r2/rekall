use super::{Env, Format, Output, need, parse_format};
use crate::{check, corpus, ledger, scan};
use std::path::{Path, PathBuf};
/// `check` takes no ids. It audits EVERYTHING the ledger claims, because a
/// gate you can point at a subset is a gate that reports green for the
/// part you did not point it at.
#[derive(Debug, Default)]
pub struct CheckArgs {
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_check(args: &[String]) -> Result<CheckArgs, String> {
    let mut out = CheckArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_check_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_check_arg<'a>(
    out: &mut CheckArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json;
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other => return Err(format!("unknown flag `{other}`")),
    }
    Ok(())
}

/// A finished audit, with the exit code it earned.
///
/// The count travels WITH the output because drift is not an error: the
/// command ran correctly and the answer is bad news. Folding it into
/// `Err` would make a working gate indistinguishable from a broken one.
pub struct Checked {
    pub output: Output,
    pub drift: usize,
}

/// Run `check` end to end.
///
/// CPU-only, and it reads nothing but the ledger and the files the ledger
/// names (V6). No key, no network, so it runs in `hk` on every commit and
/// in a CI that can reach nothing.
pub fn check_command(flags: &[String], env: &Env) -> Result<Checked, String> {
    let args = parse_check(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let held =
        ledger::load(&ledger::path_in(&base)).map_err(|e| e.to_string())?;
    let bytes = read_rows(&base, &held);
    let seen = zip_rows(&held, &bytes);
    let on_disk = artifacts_on_disk(&base, env)?;
    let mut found = check::audit(&seen, &on_disk);
    found.extend(check::undelivered(&seen, check::wired(&base)));
    render_check(&found, &check::notes(&seen), args.json)
}

/// The bytes each row is judged against, read once.
///
/// A file that cannot be read comes back as `None` rather than an error.
/// It IS a finding -- a ledger naming a file nobody can open is exactly
/// the drift this gate exists for -- and aborting on the first one would
/// report a single problem where there may be five.
struct Bytes {
    source: Option<String>,
    artifact: Option<String>,
}

fn read_rows(base: &Path, held: &ledger::Ledger) -> Vec<Bytes> {
    held.extracted
        .iter()
        .map(|row| Bytes {
            source: std::fs::read_to_string(base.join(&row.src)).ok(),
            artifact: std::fs::read_to_string(base.join(&row.artifact)).ok(),
        })
        .collect()
}

fn zip_rows<'a>(
    held: &'a ledger::Ledger,
    bytes: &'a [Bytes],
) -> Vec<check::Seen<'a>> {
    held.extracted
        .iter()
        .zip(bytes)
        .map(|(row, read)| check::Seen {
            row,
            source: read.source.as_deref(),
            artifact: read.artifact.as_deref(),
        })
        .collect()
}

/// Where `plan` materializes artifacts, and therefore the only places an
/// ORPHAN can be found.
///
/// Two named roots rather than a walk of the project: everything outside
/// them is somebody else's file, and a gate that called every unclaimed
/// file in the repo an orphan would be unusable on its first run.
const ARTIFACT_ROOTS: [&str; 2] = [".rekall/rules", ".claude/skills"];
const ARTIFACT_GLOBS: [&str; 2] = ["**/*.sh", "**/SKILL.md"];

fn artifacts_on_disk(base: &Path, env: &Env) -> Result<Vec<String>, String> {
    let roots: Vec<String> =
        ARTIFACT_ROOTS.iter().map(|r| (*r).to_string()).collect();
    let globs: Vec<String> =
        ARTIFACT_GLOBS.iter().map(|g| (*g).to_string()).collect();
    let walked = corpus::files(&roots, &globs, env.home.as_deref(), base)
        .map_err(|error| error.to_string())?;
    Ok(walked
        .files
        .iter()
        .map(|path| scan::stable_name(path, base, env.home.as_deref()))
        .collect())
}

/// V17: both formats, same anatomy. V28: human output is SILENT when
/// clean -- output that always appears is output nobody reads.
fn render_check(
    found: &[check::Drift],
    notes: &[check::Note],
    json: bool,
) -> Result<Checked, String> {
    let text = if json {
        as_json(found, notes)?
    } else {
        check::render_human(found) + &check::render_notes(notes)
    };
    Ok(Checked {
        output: Output {
            text,
            warnings: Vec::new(),
        },
        drift: found.len(),
    })
}

fn as_json(
    found: &[check::Drift],
    notes: &[check::Note],
) -> Result<String, String> {
    serde_json::to_string_pretty(&Report {
        drift: found,
        notes,
    })
    .map(|text| format!("{text}\n"))
    .map_err(|error| error.to_string())
}

/// The JSON envelope. An OBJECT rather than a bare array, so a later
/// field -- a count, a summary -- can be added without changing the type
/// every consumer already parses.
#[derive(serde::Serialize)]
struct Report<'a> {
    drift: &'a [check::Drift],
    /// True and not wrong, so it travels BESIDE the list the exit code
    /// counts rather than inside it.
    notes: &'a [check::Note],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::testing::*;
    use crate::cli::{Action, DRIFT_EXIT, USAGE_EXIT, decide, perform};
    use crate::{apply, check, ledger};
    use std::path::Path;

    /// Replace the generated placeholder with a runner that actually
    /// checks something -- the state V2 asks for.
    fn write_real_runner(dir: &Path, id: &str) {
        let path = dir.join(artifact_of(dir, id));
        let _ = std::fs::write(
            &path,
            "#!/bin/sh\n# rekall:payload\n# - never commit to `main`\n# rekall:/payload\ngrep -q x f && exit 1\n",
        );
    }

    #[test]
    fn check_is_dispatched() {
        assert_eq!(decide(&args(&["check"])), Action::Check(Vec::new()));
    }

    /// A repo that has extracted nothing is CLEAN, and SILENT (V28). A
    /// gate that fails before the tool has been used is a gate nobody
    /// adopts, and one that chatters on success is one nobody reads.
    #[test]
    fn a_repo_with_no_extractions_is_clean_and_silent() {
        let dir = check_project("clean");
        let checked = check_in(&dir, &[]);
        assert_eq!(checked.as_ref().map(|c| c.drift).unwrap_or(9), 0);
        assert_eq!(
            checked.map(|c| c.output.text).unwrap_or_default(),
            "",
            "a clean gate said something"
        );
        assert_eq!(perform(Action::Check(dash_c(&dir)), &env()), 0);
    }

    /// V2: `apply` writes a runner that announces itself unimplemented, so
    /// a fresh extraction is drift until someone writes the check. That is
    /// the point -- a rule with no runner gates nothing.
    #[test]
    fn a_freshly_extracted_rule_has_no_runner_yet() {
        let dir = check_project("no-runner");
        let (_, _) = extracted(&dir);
        let checked = check_in(&dir, &[]);
        let text = checked.map(|c| c.output.text).unwrap_or_default();
        assert!(text.contains(check::NO_RUNNER), "{text}");
        assert_eq!(perform(Action::Check(dash_c(&dir)), &env()), DRIFT_EXIT);
    }

    #[test]
    fn a_rule_with_a_real_runner_passes() {
        let dir = check_project("runner");
        let (id, _) = extracted(&dir);
        write_real_runner(&dir, &id);
        assert_eq!(check_in(&dir, &[]).map(|c| c.drift).unwrap_or(9), 0);
    }

    /// V1: the statement pasted back beside the artifact it moved into.
    #[test]
    fn a_statement_put_back_by_hand_is_reported() {
        let dir = check_project("copy");
        let (id, before) = extracted(&dir);
        write_real_runner(&dir, &id);
        let pointer = apply::pointer_of(&id);
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            format!("{pointer}\n\n{before}"),
        );
        let text = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(text.contains(check::STATEMENT_RESTORED), "{text}");
    }

    #[test]
    fn a_deleted_artifact_is_reported() {
        let dir = check_project("gone");
        let (id, _) = extracted(&dir);
        let _ = std::fs::remove_file(dir.join(artifact_of(&dir, &id)));
        let text = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(text.contains(check::MISSING_ARTIFACT), "{text}");
    }

    /// An artifact sitting in a rules directory with no ledger row behind
    /// it. Nothing records where it came from, so nothing can undo it.
    #[test]
    fn an_unclaimed_artifact_is_an_orphan() {
        let dir = check_project("orphan");
        let _ = std::fs::create_dir_all(dir.join(".rekall").join("rules"));
        let _ = std::fs::write(
            dir.join(".rekall").join("rules").join("stray.sh"),
            "#!/bin/sh\nexit 0\n",
        );
        let text = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(text.contains(check::ORPHAN_ARTIFACT), "{text}");
        assert!(text.contains("stray.sh"), "{text}");
    }

    /// A `revert` leaves NOTHING for the gate to find. The two verbs agree
    /// about what a finished extraction looks like, in both directions.
    #[test]
    fn a_reverted_extraction_leaves_a_clean_gate() {
        let dir = check_project("reverted");
        let (id, _) = extracted(&dir);
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        assert_eq!(check_in(&dir, &[]).map(|c| c.drift).unwrap_or(9), 0);
    }

    #[test]
    fn check_json_is_parseable_and_carries_the_kind_as_a_field() {
        let dir = check_project("json");
        let _ = extracted(&dir);
        let text = check_in(&dir, &["--format", "json"])
            .map(|c| c.output.text)
            .unwrap_or_default();
        let parsed: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_default();
        let kind = parsed
            .get("drift")
            .and_then(|drift| drift.get(0))
            .and_then(|first| first.get("kind"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(kind, Some(check::NO_RUNNER), "{text}");
    }

    /// V17: a clean gate still emits valid JSON. An agent parsing an empty
    /// string would have to special-case success.
    #[test]
    fn a_clean_gate_still_emits_json() {
        let dir = check_project("json-clean");
        let text = check_in(&dir, &["--format", "json"])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).ok(),
            Some(serde_json::json!({"drift": [], "notes": []})),
            "{text}"
        );
    }

    #[test]
    fn check_takes_no_positionals_and_rejects_unknown_flags() {
        assert!(parse_check(&args(&["abc"])).is_err());
        assert!(parse_check(&args(&["--nope"])).is_err());
        assert!(parse_check(&args(&["--format"])).is_err());
    }

    /// A broken invocation exits 2, not 1. "You typed it wrong" and "your
    /// corpus is wrong" are different answers and the caller reacts to
    /// them differently.
    #[test]
    fn a_usage_error_exits_two_not_one() {
        assert_eq!(perform(Action::Check(args(&["--nope"])), &env()), 2);
    }

    #[test]
    fn a_corrupt_ledger_stops_the_gate_rather_than_passing_it() {
        let dir = check_project("corrupt");
        let _ = std::fs::create_dir_all(dir.join(ledger::DIR));
        let _ = std::fs::write(ledger::path_in(&dir), "not = = toml\n");
        assert!(check_in(&dir, &[]).is_err());
        assert_eq!(perform(Action::Check(dash_c(&dir)), &env()), USAGE_EXIT);
    }
}
