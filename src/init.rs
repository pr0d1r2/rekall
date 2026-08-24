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
    for candidate in found.iter().filter(|c| c.found && c.scope.is_project()) {
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
# PROJECT roots only. A USER root -- your memory dir, your `~/.claude` --
# is deliberately absent even when `init` found one: this file is TRACKED,
# and a home path here would follow the repo into every checkout & drag
# private memory into anyone else`s scan (V36).
#
# Nothing is lost. `[sources].roots` UNION across scopes, so your own
# `~/.config/rekall/rekall.toml` still contributes its roots -- which is
# the entire reason two scopes exist.

";

/// What `init` did, or would do.
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    /// The file that was written, NAMED before the write happens.
    pub path: String,
    pub wrote: bool,
    /// PROJECT-scope roots. These, and only these, are WRITTEN (V36).
    pub roots: Vec<String>,
    /// USER-scope roots that exist and were deliberately NOT written.
    ///
    /// Named in the output because they were found and the reader should
    /// know -- but kept OUT of the tracked file, which would otherwise
    /// hard-code one developer's home into every checkout and drag private
    /// memory into anyone's scan.
    pub user: Vec<String>,
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
        roots: named(found, |c| c.found && c.scope.is_project()),
        user: named(found, |c| c.found && !c.scope.is_project()),
        absent: named(found, |c| !c.found),
    }
}

fn named(
    found: &[Candidate],
    keep: impl Fn(&Candidate) -> bool,
) -> Vec<String> {
    found
        .iter()
        .filter(|c| keep(c))
        .map(|c| c.root.clone())
        .collect()
}

/// Where a USER root actually belongs, printed only when one was found.
///
/// Naming the file is the difference between a refusal and a handoff: the
/// root is real and worth scanning, it simply does not belong in a file
/// every clone of this repo will read.
const USER_HINT: &str = "\nThose are USER roots. They are NOT written here -- this file is tracked, \
and a home path in it\nwould follow the repo to every checkout. Put them in \
`~/.config/rekall/rekall.toml`;\nroots UNION across scopes, so both get \
scanned.\n";

/// Human rendering. The JSON carries the SAME anatomy (V17).
#[must_use]
pub fn render_human(report: &Report) -> String {
    let mut out = format!("wrote  {}\n", report.path);
    for root in &report.roots {
        out.push_str(&format!("root   {root}\n"));
    }
    for root in &report.user {
        out.push_str(&format!("user   {root}  (yours, not written here)\n"));
    }
    for root in &report.absent {
        out.push_str(&format!("absent {root}\n"));
    }
    if !report.user.is_empty() {
        out.push_str(USER_HINT);
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
        named(found, |c| c.found)
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

    /// V36, THE POINT OF T37. A user root is DETECTED and deliberately
    /// left OUT of the written file. Before this, `init` wrote one
    /// developer's `~/.claude/projects` into a tracked config -- which
    /// follows the repo into every checkout and drags private memory into
    /// anyone else's scan.
    #[test]
    fn a_user_root_is_detected_but_never_written() {
        let found = with_user_root("detect-not-write");
        let written = render(&found);
        assert!(
            !written.contains("~/.claude/CLAUDE.md"),
            "a home path reached the tracked file:\n{written}"
        );
        assert!(written.contains("roots = ["), "{written}");
    }

    /// NAMED, though. V36 leaves it out of the file, not out of the
    /// conversation -- the root is real and worth scanning, it simply does
    /// not belong in a file every clone reads.
    #[test]
    fn a_user_root_is_still_reported_and_says_where_it_goes() {
        let found = with_user_root("detect-report");
        let said = render_human(&report(&found, "rekall.toml"));
        assert!(said.contains("user   ~/.claude/CLAUDE.md"), "{said}");
        assert!(said.contains("~/.config/rekall/rekall.toml"), "{said}");
        assert!(said.contains("UNION"), "{said}");
    }

    /// A PROJECT root still gets written. The narrowing is about scope,
    /// not about writing less.
    #[test]
    fn a_project_root_is_still_written() {
        let base = dir("proj-written");
        let _ = std::fs::write(base.join("CLAUDE.md"), "- x\n");
        let found = candidates(None, &base);
        assert!(render(&found).contains("\"CLAUDE.md\""), "{found:?}");
    }

    /// The written file EXPLAINS the absence, so the next reader does not
    /// think detection failed.
    #[test]
    fn the_written_file_says_why_a_user_root_is_missing() {
        let written = render(&with_user_root("explains"));
        assert!(written.contains("PROJECT roots only"), "{written}");
        assert!(written.contains("UNION"), "{written}");
    }

    /// A home holding a corpus, and a project holding none.
    fn with_user_root(name: &str) -> Vec<Candidate> {
        let home = dir(name);
        let _ = std::fs::create_dir_all(home.join(".claude"));
        let _ = std::fs::write(home.join(".claude").join("CLAUDE.md"), "- x\n");
        candidates(Some(&home.to_string_lossy()), &dir(&format!("{name}-p")))
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
            user: Vec::new(),
            absent: vec!["AGENTS.md".to_string()],
        };
        let text = render_human(&report);
        assert!(text.contains("wrote  rekall.toml"));
        assert!(text.contains("root   CLAUDE.md"));
        assert!(text.contains("absent AGENTS.md"));
    }
}
