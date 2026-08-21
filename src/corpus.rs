//! Which files the corpus is.
//!
//! Roots come from `[sources]` in both config scopes (unioned, see
//! `config::merge`). A root may be a FILE or a DIRECTORY; a directory is
//! walked and filtered by `[sources].globs`.
//!
//! Everything here READS. V16 holds harness memory directories read-only
//! unless `apply` named the file, and nothing in this module writes.

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};

/// Used when no `globs` key is set in either scope. Markdown only, because
/// the corpus is prose: a rule stated in a `.json` is configuration, and
/// one in a `.rs` is already code.
pub const DEFAULT_GLOBS: [&str; 1] = ["**/*.md"];

#[derive(Debug)]
pub enum Error {
    Glob {
        pattern: String,
        cause: globset::Error,
    },
    Walk {
        root: PathBuf,
        cause: walkdir::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Glob { pattern, cause } => {
                write!(f, "bad glob `{pattern}`: {cause}")
            }
            Self::Walk { root, cause } => {
                write!(f, "cannot walk {}: {cause}", root.display())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Expand `~` against `$HOME`.
///
/// Done here rather than left to the shell because roots arrive from a
/// CONFIG FILE, which no shell expands. A literal `~/.claude` directory
/// would otherwise be created relative to the working directory by anything
/// that later wrote there.
#[must_use]
pub fn expand_home(raw: &str, home: Option<&str>) -> PathBuf {
    let Some(home) = home else {
        return PathBuf::from(raw);
    };
    if raw == "~" {
        return PathBuf::from(home);
    }
    match raw.strip_prefix("~/") {
        Some(rest) => Path::new(home).join(rest),
        None => PathBuf::from(raw),
    }
}

/// Resolve one root: expand `~`, then anchor a RELATIVE path to `base`.
///
/// `base` is what `-C` sets. Without this a relative root like
/// `./CLAUDE.md` resolves against the PROCESS working directory while the
/// config it came from was found under `-C`, so the two halves of one
/// command disagree about where the project is. `git -C` anchors
/// everything; so does this.
#[must_use]
pub fn resolve(raw: &str, home: Option<&str>, base: &Path) -> PathBuf {
    let expanded = expand_home(raw, home);
    if expanded.is_absolute() {
        return expanded;
    }
    base.join(expanded)
}

/// Build a matcher from glob patterns.
///
/// An unparsable pattern is an ERROR, never skipped: a typo'd glob that
/// silently matches nothing looks exactly like a corpus with nothing in it.
pub fn matcher(patterns: &[String]) -> Result<GlobSet, Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|cause| Error::Glob {
            pattern: pattern.clone(),
            cause,
        })?;
        builder.add(glob);
    }
    builder.build().map_err(|cause| Error::Glob {
        pattern: patterns.join(", "),
        cause,
    })
}

/// Every file one root contributes.
///
/// A root naming a FILE contributes that file whether or not it matches the
/// globs: naming a path explicitly IS the selection, and re-filtering it
/// would make `roots = ["./NOTES.txt"]` silently contribute nothing.
fn from_root(root: &Path, globs: &GlobSet) -> Result<Vec<PathBuf>, Error> {
    if root.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    // A root that does not exist is NOT fatal. A user-scope config naming
    // `~/.claude` is correct on the machine that has it and wrong on the
    // one that does not, and failing the whole inventory over an absent
    // optional root would make the tool unusable across machines. The
    // caller reports it BY NAME -- an absent root is a named skip, never a
    // silent one (V26).
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|cause| Error::Walk {
            root: root.to_path_buf(),
            cause,
        })?;
        collect(entry, root, globs, &mut out);
    }
    Ok(out)
}

/// Glob matching is against the path RELATIVE to its root, so a pattern
/// like `**/*.md` means the same thing wherever the corpus is checked out.
/// Matching absolute paths would make results depend on the home directory.
fn collect(
    entry: walkdir::DirEntry,
    root: &Path,
    globs: &GlobSet,
    out: &mut Vec<PathBuf>,
) {
    if !entry.file_type().is_file() {
        return;
    }
    let path = entry.path();
    let relative = path.strip_prefix(root).unwrap_or(path);
    if globs.is_match(relative) {
        out.push(path.to_path_buf());
    }
}

/// Resolve roots and globs into the ordered, deduplicated file list `scan`
/// reads.
///
/// SORTED and deduplicated because V13 requires `scan(scan(x))` to be
/// identical: directory iteration order is a property of the filesystem,
/// not of the corpus, so leaving it unsorted would make the report vary
/// between machines while nothing had changed.
pub fn files(
    roots: &[String],
    globs: &[String],
    home: Option<&str>,
    base: &Path,
) -> Result<Vec<PathBuf>, Error> {
    let patterns = if globs.is_empty() {
        DEFAULT_GLOBS.iter().map(|g| (*g).to_string()).collect()
    } else {
        globs.to_vec()
    };
    let set = matcher(&patterns)?;
    let mut out = Vec::new();
    for raw in roots {
        out.extend(from_root(&resolve(raw, home, base), &set)?);
    }
    out.sort();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn globs(patterns: &[&str]) -> GlobSet {
        let owned: Vec<String> =
            patterns.iter().map(|p| (*p).to_string()).collect();
        matcher(&owned).unwrap_or_else(|_| GlobSet::empty())
    }

    #[test]
    fn expand_home_replaces_a_leading_tilde() {
        assert_eq!(
            expand_home("~/.claude", Some("/home/x")),
            PathBuf::from("/home/x/.claude")
        );
        assert_eq!(expand_home("~", Some("/home/x")), PathBuf::from("/home/x"));
    }

    #[test]
    fn expand_home_leaves_other_paths_alone() {
        assert_eq!(
            expand_home("./CLAUDE.md", Some("/home/x")),
            PathBuf::from("./CLAUDE.md")
        );
        assert_eq!(
            expand_home("/etc/x", Some("/home/x")),
            PathBuf::from("/etc/x")
        );
    }

    /// `~someone/notes` is another user's home in shell convention, not a
    /// path under ours. Treating it as literal is wrong in a different way
    /// than expanding it would be, so this pins the choice.
    #[test]
    fn expand_home_does_not_touch_a_named_user_tilde() {
        assert_eq!(
            expand_home("~someone/notes", Some("/home/x")),
            PathBuf::from("~someone/notes")
        );
    }

    #[test]
    fn expand_home_without_a_home_is_a_no_op() {
        assert_eq!(expand_home("~/.claude", None), PathBuf::from("~/.claude"));
    }

    #[test]
    fn a_bad_glob_is_an_error_not_an_empty_match() {
        let result = matcher(&["[".to_string()]);
        assert!(
            result.is_err(),
            "a typo'd glob must not look like an empty corpus"
        );
    }

    #[test]
    fn globs_match_relative_to_the_root() {
        let set = globs(&["**/*.md"]);
        assert!(set.is_match(Path::new("nested/deep/CLAUDE.md")));
        assert!(set.is_match(Path::new("CLAUDE.md")));
        assert!(!set.is_match(Path::new("notes.txt")));
    }

    #[test]
    fn default_globs_select_markdown_only() {
        let owned: Vec<String> =
            DEFAULT_GLOBS.iter().map(|g| (*g).to_string()).collect();
        let set = matcher(&owned).unwrap_or_else(|_| GlobSet::empty());
        assert!(set.is_match(Path::new("a/b.md")));
        assert!(!set.is_match(Path::new("a/b.json")));
    }

    #[test]
    fn no_roots_is_an_empty_corpus_not_an_error() {
        let found = files(&[], &[], None, Path::new("."));
        assert_eq!(found.ok(), Some(Vec::new()));
    }

    #[test]
    fn a_file_root_is_taken_whether_or_not_it_matches_the_globs() {
        let found = files(
            &["Cargo.toml".to_string()],
            &["**/*.md".to_string()],
            None,
            Path::new("."),
        );
        assert_eq!(found.ok(), Some(vec![PathBuf::from("./Cargo.toml")]));
    }

    #[test]
    fn a_directory_root_is_walked_and_filtered() {
        let found = files(
            &["src".to_string()],
            &["**/*.rs".to_string()],
            None,
            Path::new("."),
        )
        .unwrap_or_default();
        assert!(
            found.contains(&PathBuf::from("./src/corpus.rs")),
            "found {found:?}"
        );
        assert!(
            !found
                .iter()
                .any(|p| p.extension().is_some_and(|e| e == "md"))
        );
    }

    #[test]
    fn results_are_sorted_and_deduplicated() {
        let roots = vec!["src".to_string(), "src".to_string()];
        let found =
            files(&roots, &["**/*.rs".to_string()], None, Path::new("."))
                .unwrap_or_default();
        let mut sorted = found.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(found, sorted, "scan must not vary with filesystem order");
    }
    /// `-C` must anchor ROOTS, not only config discovery. Without this the
    /// config found under `-C` and the roots read from it disagree about
    /// where the project is.
    #[test]
    fn a_relative_root_is_anchored_to_the_base() {
        assert_eq!(
            resolve("CLAUDE.md", None, Path::new("/tmp/x")),
            PathBuf::from("/tmp/x/CLAUDE.md")
        );
    }

    #[test]
    fn an_absolute_root_ignores_the_base() {
        assert_eq!(
            resolve("/etc/notes.md", None, Path::new("/tmp/x")),
            PathBuf::from("/etc/notes.md")
        );
    }

    #[test]
    fn a_tilde_root_expands_before_it_is_anchored() {
        assert_eq!(
            resolve("~/.claude", Some("/home/u"), Path::new("/tmp/x")),
            PathBuf::from("/home/u/.claude")
        );
    }

    /// A configured root that does not exist is a NAMED skip, not a fatal
    /// error: a user-scope config is right on one machine and wrong on the
    /// next, and failing the whole inventory over it would make the tool
    /// unusable across machines.
    #[test]
    fn a_missing_root_is_skipped_rather_than_fatal() {
        let found = files(
            &["definitely/not/here".to_string()],
            &["**/*.md".to_string()],
            None,
            Path::new("."),
        );
        assert_eq!(found.ok(), Some(Vec::new()));
    }
}
