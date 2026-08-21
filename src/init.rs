//! `rekall init` -- the cold start.
//!
//! Detects the corpus roots that actually exist and writes a project
//! `rekall.toml` from what it found. Without this, first contact is `scan`
//! failing on a file the user has never heard of and has no template for.
//!
//! Everything that DECIDES is pure: `candidates` reports what exists,
//! `render` turns that into the file's text. Only `write_config` touches
//! the disk, and it refuses to overwrite.

use crate::config::{FILE_NAME, Scope};
use std::path::{Path, PathBuf};

/// A place a corpus root might live, and whether it is really there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// As it should appear in the config: `~`-prefixed for user scope,
    /// relative for project scope. Written this way rather than absolute so
    /// the file is portable between machines and readable in a diff.
    pub root: String,
    pub scope: Scope,
    pub found: bool,
}

/// Where user-scope prose lives, relative to home.
const USER_ROOTS: [&str; 2] = [".claude/CLAUDE.md", ".claude/projects"];

/// Where project-scope prose lives, relative to the project root.
const PROJECT_ROOTS: [&str; 3] = ["CLAUDE.md", "AGENTS.md", ".claude/skills"];

/// Every place worth looking, with whether it exists.
///
/// Reports the MISSES as well as the hits. A root that is absent today may
/// be created tomorrow, and a user who can see what was looked for can add
/// it by hand -- whereas a silent omission looks like the tool not
/// supporting that location at all.
#[must_use]
pub fn candidates(home: Option<&str>, base: &Path) -> Vec<Candidate> {
    let mut out = Vec::new();
    for suffix in USER_ROOTS {
        out.push(user_candidate(home, suffix));
    }
    for suffix in PROJECT_ROOTS {
        out.push(project_candidate(base, suffix));
    }
    out
}

fn user_candidate(home: Option<&str>, suffix: &str) -> Candidate {
    let found = home.is_some_and(|home| Path::new(home).join(suffix).exists());
    Candidate {
        root: format!("~/{suffix}"),
        scope: Scope::User,
        found,
    }
}

fn project_candidate(base: &Path, suffix: &str) -> Candidate {
    Candidate {
        root: suffix.to_string(),
        scope: Scope::Project,
        found: base.join(suffix).exists(),
    }
}

/// The config text, from the candidates that were found.
///
/// Commented, because a generated config nobody can read is a config
/// nobody will edit -- and the roots that were looked for and NOT found are
/// listed too, so adding one later is a matter of uncommenting rather than
/// of guessing the spelling.
#[must_use]
pub fn render(found: &[Candidate]) -> String {
    let mut out = String::from(HEADER);
    out.push_str("[sources]\nroots = [\n");
    for candidate in found.iter().filter(|c| c.found) {
        out.push_str(&format!("  \"{}\",\n", candidate.root));
    }
    out.push_str("]\n");
    out.push_str(&misses(found));
    out
}

fn misses(found: &[Candidate]) -> String {
    let absent: Vec<&Candidate> = found.iter().filter(|c| !c.found).collect();
    if absent.is_empty() {
        return String::new();
    }
    let mut out =
        String::from("\n# Looked for and not found. Uncomment to add:\n");
    for candidate in absent {
        out.push_str(&format!("#   \"{}\"\n", candidate.root));
    }
    out
}

const HEADER: &str = "\
# Written by `rekall init`. Roots are what existed when it ran.
#
# `[sources].roots` UNION across scopes: a project file adds to the user's
# roots rather than replacing them, so dropping this file into a repo can
# never silently stop scanning your memory.

";

/// What `init` did, or would do.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    /// The file that was written, NAMED before the write happens.
    pub path: String,
    pub wrote: bool,
    pub roots: Vec<String>,
    /// Places looked at that held nothing.
    pub absent: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    Exists(PathBuf),
    Write {
        path: PathBuf,
        cause: std::io::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exists(path) => write!(
                f,
                "{} already exists -- pass --force to replace it",
                path.display()
            ),
            Self::Write { path, cause } => {
                write!(f, "cannot write {}: {cause}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Write the config.
///
/// Refuses an existing file without `--force`. A config the user has since
/// edited is not a file to silently regenerate, and `init` is the one verb
/// a newcomer runs without knowing what it will do.
pub fn write_config(
    base: &Path,
    text: &str,
    force: bool,
) -> Result<PathBuf, Error> {
    let path = base.join(FILE_NAME);
    if path.exists() && !force {
        return Err(Error::Exists(path));
    }
    std::fs::write(&path, text).map_err(|cause| Error::Write {
        path: path.clone(),
        cause,
    })?;
    Ok(path)
}

/// The whole verb: detect, render, write, report.
pub fn run(
    base: &Path,
    home: Option<&str>,
    force: bool,
) -> Result<Report, Error> {
    let found = candidates(home, base);
    let path = write_config(base, &render(&found), force)?;
    Ok(report(&found, &path.to_string_lossy()))
}

fn report(found: &[Candidate], path: &str) -> Report {
    Report {
        path: path.to_string(),
        wrote: true,
        roots: pick(found, true),
        absent: pick(found, false),
    }
}

fn pick(found: &[Candidate], wanted: bool) -> Vec<String> {
    found
        .iter()
        .filter(|c| c.found == wanted)
        .map(|c| c.root.clone())
        .collect()
}

/// Human rendering. The JSON carries the SAME anatomy (V17).
#[must_use]
pub fn render_human(report: &Report) -> String {
    let mut out = format!("wrote  {}\n", report.path);
    for root in &report.roots {
        out.push_str(&format!("root   {root}\n"));
    }
    for root in &report.absent {
        out.push_str(&format!("absent {root}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let path = PathBuf::from("target").join("init").join(name);
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::create_dir_all(&path);
        path
    }

    fn roots_of(found: &[Candidate]) -> Vec<String> {
        pick(found, true)
    }

    #[test]
    fn nothing_on_disk_means_no_roots_but_still_a_full_search() {
        let base = dir("empty");
        let found = candidates(None, &base);
        assert!(roots_of(&found).is_empty());
        assert_eq!(found.len(), USER_ROOTS.len() + PROJECT_ROOTS.len());
    }

    #[test]
    fn a_project_claude_md_is_detected() {
        let base = dir("project");
        let _ = std::fs::write(base.join("CLAUDE.md"), "- a rule\n");
        assert_eq!(roots_of(&candidates(None, &base)), vec!["CLAUDE.md"]);
    }

    #[test]
    fn a_user_root_is_detected_and_written_with_a_tilde() {
        let home = dir("home");
        let _ = std::fs::create_dir_all(home.join(".claude"));
        let _ = std::fs::write(home.join(".claude").join("CLAUDE.md"), "- x\n");
        let found = candidates(Some(&home.to_string_lossy()), &dir("proj"));
        assert_eq!(roots_of(&found), vec!["~/.claude/CLAUDE.md"]);
    }

    /// Absolute paths would make the file unportable and unreadable in a
    /// diff. `~` and relative names survive being moved between machines.
    #[test]
    fn written_roots_are_never_absolute() {
        let home = dir("home-abs");
        let _ = std::fs::create_dir_all(home.join(".claude"));
        let _ = std::fs::write(home.join(".claude").join("CLAUDE.md"), "- x\n");
        let found = candidates(Some(&home.to_string_lossy()), &dir("proj-abs"));
        assert!(
            !render(&found).contains(&home.to_string_lossy().to_string()),
            "the home path leaked into the config"
        );
    }

    /// The misses are listed too. A silent omission looks like the tool not
    /// supporting that location at all.
    #[test]
    fn roots_that_were_looked_for_and_missed_are_listed_as_comments() {
        let base = dir("misses");
        let text = render(&candidates(None, &base));
        assert!(text.contains("Looked for and not found"));
        assert!(text.contains("#   \"AGENTS.md\""));
    }

    #[test]
    fn a_config_with_every_root_found_lists_no_misses() {
        let found = vec![Candidate {
            root: "CLAUDE.md".to_string(),
            scope: Scope::Project,
            found: true,
        }];
        assert!(!render(&found).contains("Looked for and not found"));
    }

    #[test]
    fn the_rendered_config_parses_as_the_config_it_describes() {
        let base = dir("parses");
        let _ = std::fs::write(base.join("CLAUDE.md"), "- a rule\n");
        let text = render(&candidates(None, &base));
        let parsed = crate::config::parse(&text, Path::new("rekall.toml"));
        assert_eq!(
            parsed.ok().and_then(|cfg| cfg.sources.roots),
            Some(vec!["CLAUDE.md".to_string()])
        );
    }

    #[test]
    fn init_writes_the_file_and_names_it() {
        let base = dir("writes");
        let _ = std::fs::write(base.join("CLAUDE.md"), "- a rule\n");
        let report = run(&base, None, false);
        assert_eq!(
            report.map(|report| (report.wrote, report.roots)).ok(),
            Some((true, vec!["CLAUDE.md".to_string()]))
        );
        assert!(base.join(FILE_NAME).is_file());
    }

    /// A config the user has since edited is not a file to silently
    /// regenerate.
    #[test]
    fn init_refuses_to_overwrite_without_force() {
        let base = dir("no-clobber");
        let _ = std::fs::write(
            base.join(FILE_NAME),
            "[sources]\nroots = [\"mine\"]\n",
        );
        assert!(run(&base, None, false).is_err());
        let kept =
            std::fs::read_to_string(base.join(FILE_NAME)).unwrap_or_default();
        assert!(kept.contains("mine"), "the existing config was clobbered");
    }

    #[test]
    fn force_replaces_an_existing_config() {
        let base = dir("force");
        let _ = std::fs::write(
            base.join(FILE_NAME),
            "[sources]\nroots = [\"old\"]\n",
        );
        assert!(run(&base, None, true).is_ok());
        let text =
            std::fs::read_to_string(base.join(FILE_NAME)).unwrap_or_default();
        assert!(!text.contains("old"));
    }

    #[test]
    fn the_refusal_says_how_to_proceed() {
        let base = dir("refusal-message");
        let _ = std::fs::write(base.join(FILE_NAME), "[sources]\n");
        let message = run(&base, None, false)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(message.contains("--force"), "message was {message}");
    }

    #[test]
    fn a_write_error_names_the_path() {
        let error =
            write_config(Path::new("target/definitely/absent/dir"), "x", false)
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default();
        assert!(error.contains("cannot write"), "message was {error}");
    }

    #[test]
    fn human_output_names_the_file_and_every_root() {
        let report = Report {
            path: "rekall.toml".to_string(),
            wrote: true,
            roots: vec!["CLAUDE.md".to_string()],
            absent: vec!["AGENTS.md".to_string()],
        };
        let text = render_human(&report);
        assert!(text.contains("wrote  rekall.toml"));
        assert!(text.contains("root   CLAUDE.md"));
        assert!(text.contains("absent AGENTS.md"));
    }
}
