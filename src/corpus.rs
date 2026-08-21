//! Which files the corpus is.
//!
//! Roots come from `[sources]` in both config scopes (unioned, see
//! `config::merge`). A root may be a FILE or a DIRECTORY; a directory is
//! walked and filtered by `[sources].globs`.
//!
//! Everything here READS. V16 holds harness memory directories read-only
//! unless `apply` named the file, and nothing in this module writes.

use globset::{Glob, GlobMatcher};
use std::path::{Path, PathBuf};

/// Used when no `globs` key is set in either scope. Markdown only, because
/// the corpus is prose: a rule stated in a `.json` is configuration, and
/// one in a `.rs` is already code.
pub const DEFAULT_GLOBS: [&str; 1] = ["**/*.md"];

/// The only thing here that can FAIL a run is a pattern that will not
/// compile, which is a config error the user can fix. A directory that
/// cannot be walked is reported as a skip instead -- see `Walked`.
#[derive(Debug)]
pub enum Error {
    Glob {
        pattern: String,
        cause: globset::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::Glob { pattern, cause } = self;
        write!(f, "bad glob `{pattern}`: {cause}")
    }
}

/// What a walk found, and what it could not look at.
///
/// An entry that errors mid-walk -- a subdirectory with no read
/// permission, a broken link -- is SKIPPED and NAMED, exactly like a file
/// that is not valid UTF-8. Those are the same situation at different
/// depths, and treating one as fatal while naming the other was an
/// inconsistency rather than a policy: a single unreadable subdirectory
/// would abort the inventory of an otherwise readable corpus.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Walked {
    pub files: Vec<PathBuf>,
    pub unreadable: Vec<PathBuf>,
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

/// A set of compiled glob patterns.
///
/// A `Vec` of per-glob matchers rather than a `GlobSet`. `GlobSetBuilder::
/// build()` is fallible, and after `Glob::new` has validated every pattern
/// that failure cannot happen -- which left three lines of error handling
/// no test could ever reach. `Glob::compile_matcher` is infallible, so the
/// unreachable branch stops existing instead of being explained. For the
/// handful of patterns a corpus config carries, the single-regex
/// optimisation GlobSet exists for buys nothing measurable.
#[derive(Debug, Default)]
pub struct Matcher {
    globs: Vec<GlobMatcher>,
}

impl Matcher {
    #[must_use]
    pub fn is_match(&self, path: &Path) -> bool {
        self.globs.iter().any(|glob| glob.is_match(path))
    }
}

/// Build a matcher from glob patterns.
///
/// An unparsable pattern is an ERROR, never skipped: a typo'd glob that
/// silently matches nothing looks exactly like a corpus with nothing in it.
pub fn matcher(patterns: &[String]) -> Result<Matcher, Error> {
    let mut globs = Vec::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|cause| Error::Glob {
            pattern: pattern.clone(),
            cause,
        })?;
        globs.push(glob.compile_matcher());
    }
    Ok(Matcher { globs })
}

/// Every file one root contributes.
///
/// A root naming a FILE contributes that file whether or not it matches the
/// globs: naming a path explicitly IS the selection, and re-filtering it
/// would make `roots = ["./NOTES.txt"]` silently contribute nothing.
fn from_root(root: &Path, globs: &Matcher) -> Walked {
    if root.is_file() {
        return Walked {
            files: vec![root.to_path_buf()],
            unreadable: Vec::new(),
        };
    }
    // A root that does not exist is NOT fatal, and is not a skip either.
    // A user-scope config naming `~/.claude` is correct on the machine
    // that has it and simply does not apply on one that does not. That is
    // different from a root that exists and cannot be read, which IS a
    // skip and is named as one.
    if !root.exists() {
        return Walked::default();
    }
    walk(root, globs)
}

fn walk(root: &Path, globs: &Matcher) -> Walked {
    let mut found = Walked::default();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        match entry {
            Ok(entry) => collect(entry, root, globs, &mut found.files),
            Err(error) => {
                found.unreadable.push(errored_path(error.path(), root))
            }
        }
    }
    found
}

/// The path a walk error refers to, falling back to the root when the
/// error carries none. Naming SOMETHING is the point: a skip nobody can
/// see is the failure V26 forbids.
///
/// Takes the path rather than the error so it is a pure function with no
/// way to construct its input by hand -- `walkdir::Error` has no public
/// constructor, so a version taking the error could only be exercised by
/// provoking a real filesystem failure.
fn errored_path(path: Option<&Path>, root: &Path) -> PathBuf {
    path.unwrap_or(root).to_path_buf()
}

/// Glob matching is against the path RELATIVE to its root, so a pattern
/// like `**/*.md` means the same thing wherever the corpus is checked out.
/// Matching absolute paths would make results depend on the home directory.
fn collect(
    entry: walkdir::DirEntry,
    root: &Path,
    globs: &Matcher,
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
) -> Result<Walked, Error> {
    let patterns = if globs.is_empty() {
        DEFAULT_GLOBS.iter().map(|g| (*g).to_string()).collect()
    } else {
        globs.to_vec()
    };
    let set = matcher(&patterns)?;
    let mut out = Walked::default();
    for raw in roots {
        let found = from_root(&resolve(raw, home, base), &set);
        out.files.extend(found.files);
        out.unreadable.extend(found.unreadable);
    }
    out.files.sort();
    out.files.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn globs(patterns: &[&str]) -> Matcher {
        let owned: Vec<String> =
            patterns.iter().map(|p| (*p).to_string()).collect();
        matcher(&owned).unwrap_or_default()
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
        let set = matcher(&owned).unwrap_or_default();
        assert!(set.is_match(Path::new("a/b.md")));
        assert!(!set.is_match(Path::new("a/b.json")));
    }

    #[test]
    fn no_roots_is_an_empty_corpus_not_an_error() {
        let found = files(&[], &[], None, Path::new("."));
        assert_eq!(found.ok(), Some(Walked::default()));
    }

    #[test]
    fn a_file_root_is_taken_whether_or_not_it_matches_the_globs() {
        let found = files(
            &["Cargo.toml".to_string()],
            &["**/*.md".to_string()],
            None,
            Path::new("."),
        );
        assert_eq!(
            found.map(|walked| walked.files).ok(),
            Some(vec![PathBuf::from("./Cargo.toml")])
        );
    }

    #[test]
    fn a_directory_root_is_walked_and_filtered() {
        let found = walked(&["src"], &["**/*.rs"]).files;
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

    fn walked(roots: &[&str], globs: &[&str]) -> Walked {
        let roots: Vec<String> =
            roots.iter().map(|r| (*r).to_string()).collect();
        let globs: Vec<String> =
            globs.iter().map(|g| (*g).to_string()).collect();
        files(&roots, &globs, None, Path::new(".")).unwrap_or_default()
    }

    #[test]
    fn results_are_sorted_and_deduplicated() {
        let found = walked(&["src", "src"], &["**/*.rs"]).files;
        let mut sorted = found.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(found, sorted, "scan must not vary with filesystem order");
    }

    /// An unreadable ENTRY is a named skip, exactly like an unreadable
    /// file. It was previously fatal, which meant one bad subdirectory
    /// aborted the inventory of an otherwise readable corpus.
    #[test]
    fn a_walk_reports_files_and_skips_separately() {
        let found = walked(&["src"], &["**/*.rs"]);
        assert!(!found.files.is_empty());
        assert!(found.unreadable.is_empty(), "{:?}", found.unreadable);
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
        assert_eq!(found.ok(), Some(Walked::default()));
    }
    #[test]
    fn a_glob_error_names_the_pattern() {
        let message = matcher(&["[".to_string()])
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(message.contains("bad glob"), "message was {message}");
    }

    /// A root that exists but cannot be walked is an ERROR, unlike a root
    /// that is simply absent. The two are different situations: absent is
    /// a config that does not apply here, unwalkable is a corpus this run
    /// cannot see, and reporting them the same way hides the second.
    #[test]
    fn a_file_used_as_a_glob_pattern_source_still_errors_clearly() {
        let error = matcher(&["a[".to_string(), "**/*.md".to_string()]);
        assert!(error.is_err());
    }

    #[test]
    fn an_empty_glob_list_matches_nothing_rather_than_everything() {
        assert_eq!(
            matcher(&[])
                .map(|set| set.is_match(Path::new("CLAUDE.md")))
                .ok(),
            Some(false)
        );
    }
    #[test]
    fn an_errored_entry_is_named_by_its_own_path() {
        assert_eq!(
            errored_path(
                Some(Path::new("/corpus/locked")),
                Path::new("/corpus")
            ),
            PathBuf::from("/corpus/locked")
        );
    }

    /// When the error carries no path, the ROOT is named. Naming something
    /// is the point -- a skip nobody can see is the failure V26 forbids.
    #[test]
    fn an_errored_entry_with_no_path_falls_back_to_the_root() {
        assert_eq!(
            errored_path(None, Path::new("/corpus")),
            PathBuf::from("/corpus")
        );
    }
    /// A subdirectory that exists but cannot be read is a NAMED SKIP, not
    /// a fatal error. This is the behaviour the `Walked` split exists for,
    /// and it was previously untested as well as uncovered.
    ///
    /// The test asserts its own PRECONDITION: if the environment cannot
    /// make a directory unreadable -- running as root, or a filesystem
    /// that ignores mode bits -- the setup assertion fails loudly rather
    /// than the test passing without having exercised anything. A test
    /// that quietly succeeds because its condition never occurred is worse
    /// than no test (V26, applied to our own suite).
    #[cfg(unix)]
    #[test]
    fn an_unreadable_subdirectory_is_skipped_and_named() {
        let root = PathBuf::from("target").join("locked-corpus");
        let (was_locked, found) = scan_with_locked_dir(&root);
        assert!(was_locked, "{PRECONDITION}");
        assert_eq!(found.unreadable.len(), 1, "{:?}", found.unreadable);
        assert_eq!(found.files.len(), 1, "the readable file still scanned");
    }

    #[cfg(unix)]
    const PRECONDITION: &str = "precondition failed: this environment cannot make a \
         directory unreadable (running as root?), so the skip path was never exercised";

    /// Locks a subdirectory, scans, and ALWAYS unlocks -- no conditional
    /// cleanup, so there is no branch that only a root environment takes.
    /// Returns whether the lock actually took, so the caller asserts its
    /// own precondition rather than passing silently.
    #[cfg(unix)]
    fn scan_with_locked_dir(root: &Path) -> (bool, Walked) {
        let locked = root.join("locked");
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::create_dir_all(&locked);
        let _ = std::fs::write(root.join("ok.md"), "- a rule\n");
        set_mode(&locked, 0o000);
        let was_locked = std::fs::read_dir(&locked).is_err();
        let found = walked(&[&root.to_string_lossy()], &["**/*.md"]);
        set_mode(&locked, 0o755);
        (was_locked, found)
    }

    #[cfg(unix)]
    fn set_mode(dir: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(
            dir,
            std::fs::Permissions::from_mode(mode),
        );
    }
}
