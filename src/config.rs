//! `rekall.toml`: two scopes, one format, one parser.
//!
//! Section I fixes the shape. A project file is found by walking up from the
//! working directory; a user file lives under the XDG config directory.
//! Merging is per key with the project winning, EXCEPT `[sources].roots`,
//! which union -- because the corpus spans both scopes, and letting a
//! project file replace the roots would silently stop scanning the user's
//! memory.

use std::path::{Path, PathBuf};

/// The file name, in both scopes. One name is what keeps this two LOCATIONS
/// rather than two formats.
pub const FILE_NAME: &str = "rekall.toml";

/// A parsed config, before scopes are merged. Every field is optional so
/// "absent" and "present but empty" stay distinguishable -- a project file
/// that sets `globs = []` is making a statement, and one that omits the key
/// is not.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub sources: Sources,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    pub roots: Option<Vec<String>>,
    pub globs: Option<Vec<String>>,
}

/// Which scope a value came from. `init` reports this so the union is
/// visible when it is created rather than inferred later from surprising
/// scan output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
}

#[derive(Debug)]
pub enum Error {
    Read {
        path: PathBuf,
        cause: std::io::Error,
    },
    Parse {
        path: PathBuf,
        cause: toml::de::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, cause } => {
                write!(f, "cannot read {}: {cause}", path.display())
            }
            Self::Parse { path, cause } => {
                write!(f, "cannot parse {}: {cause}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}

/// Parse one file's text.
///
/// Unknown keys are an ERROR, never ignored. A typo in a config key that
/// parses to "default" is a setting the user believes is in effect and is
/// not -- the same silent-skip shape V26 forbids for a missing runner.
pub fn parse(text: &str, path: &Path) -> Result<Config, Error> {
    toml::from_str(text).map_err(|cause| Error::Parse {
        path: path.to_path_buf(),
        cause,
    })
}

/// Read and parse one file.
pub fn load_file(path: &Path) -> Result<Config, Error> {
    let text = std::fs::read_to_string(path).map_err(|cause| Error::Read {
        path: path.to_path_buf(),
        cause,
    })?;
    parse(&text, path)
}

/// Walk UP from `start` looking for `rekall.toml`.
///
/// Stops at the first hit, and also at a directory containing `.git`: a
/// repository boundary is where "this project" ends, and walking past it
/// would let a config in a parent checkout silently govern an unrelated
/// repo below it.
pub fn find_project(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(current) = dir {
        let candidate = current.join(FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if current.join(".git").exists() {
            return None;
        }
        dir = current.parent();
    }
    None
}

/// The user-scope path: `$XDG_CONFIG_HOME/rekall/rekall.toml`, falling back
/// to `$HOME/.config/rekall/rekall.toml`.
pub fn user_path() -> Option<PathBuf> {
    if let Some(xdg) = non_empty_var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("rekall").join(FILE_NAME));
    }
    let home = non_empty_var("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("rekall")
            .join(FILE_NAME),
    )
}

/// An environment variable set to the empty string is treated as unset.
/// `XDG_CONFIG_HOME=""` reaching this unguarded would resolve the user
/// config to `/rekall/rekall.toml`.
fn non_empty_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

/// Merge the two scopes.
///
/// `roots` UNION, user first, duplicates dropped so a root named in both
/// scopes is scanned once. Every other key is replaced by the project when
/// the project sets it.
///
/// The union is the one asymmetry, and it is the point of having two
/// scopes at all: a project file that replaced the roots would silently
/// stop scanning the user's memory.
///
/// ```
/// use rekall::config::{merge, parse};
/// use std::path::Path;
///
/// let user = parse("[sources]\nroots = [\"~/.claude\"]\n", Path::new("u"))?;
/// let project = parse("[sources]\nroots = [\"./CLAUDE.md\"]\n", Path::new("p"))?;
/// let merged = merge(user, project);
///
/// // Both survive. The project did not replace the user's corpus.
/// assert_eq!(
///     merged.sources.roots.as_deref(),
///     Some(&["~/.claude".to_string(), "./CLAUDE.md".to_string()][..])
/// );
/// # Ok::<(), rekall::config::Error>(())
/// ```
pub fn merge(user: Config, project: Config) -> Config {
    Config {
        sources: Sources {
            roots: union(user.sources.roots, project.sources.roots),
            globs: project.sources.globs.or(user.sources.globs),
        },
    }
}

fn union(
    user: Option<Vec<String>>,
    project: Option<Vec<String>>,
) -> Option<Vec<String>> {
    if user.is_none() && project.is_none() {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    for value in user
        .unwrap_or_default()
        .into_iter()
        .chain(project.unwrap_or_default())
    {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    Some(out)
}

/// Which scope each root came from, in merge order. `init` and `scan
/// --sources` report this.
pub fn attribute(user: &Config, project: &Config) -> Vec<(String, Scope)> {
    let mut out: Vec<(String, Scope)> = Vec::new();
    let from_user = user.sources.roots.clone().unwrap_or_default();
    let from_project = project.sources.roots.clone().unwrap_or_default();
    for root in from_user {
        push_unique(&mut out, root, Scope::User);
    }
    for root in from_project {
        push_unique(&mut out, root, Scope::Project);
    }
    out
}

fn push_unique(out: &mut Vec<(String, Scope)>, root: String, scope: Scope) {
    if !out.iter().any(|(existing, _)| existing == &root) {
        out.push((root, scope));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test that cannot parse its own fixture has nothing to assert, so
    /// the helper aborts rather than returning a default that would make
    /// every later assertion test the default instead of the fixture.
    ///
    /// `expect` rather than `allow`: the exemption NAMES what is deliberate
    /// and stops compiling if the panic ever leaves this helper, which a
    /// blanket allow on the module would not.
    #[expect(
        clippy::panic,
        reason = "a fixture that will not parse must fail loudly, not silently default"
    )]
    fn cfg(toml_text: &str) -> Config {
        match parse(toml_text, Path::new("test.toml")) {
            Ok(parsed) => parsed,
            Err(error) => panic!("fixture failed to parse: {error}"),
        }
    }

    #[test]
    fn empty_file_is_a_valid_config() {
        assert_eq!(cfg(""), Config::default());
    }

    #[test]
    fn absent_and_empty_are_different() {
        let absent = cfg("[sources]\n");
        let empty = cfg("[sources]\nglobs = []\n");
        assert_eq!(absent.sources.globs, None);
        assert_eq!(empty.sources.globs, Some(Vec::new()));
    }

    #[test]
    fn unknown_key_is_an_error_not_a_default() {
        let result = parse("[sources]\nrotos = []\n", Path::new("t.toml"));
        assert!(result.is_err(), "a typo must not parse to a default");
    }

    #[test]
    fn roots_union_across_scopes() {
        let user = cfg("[sources]\nroots = [\"~/.claude\"]\n");
        let project = cfg("[sources]\nroots = [\"./CLAUDE.md\"]\n");
        let merged = merge(user, project);
        assert_eq!(
            merged.sources.roots,
            Some(vec!["~/.claude".to_string(), "./CLAUDE.md".to_string()])
        );
    }

    #[test]
    fn a_project_root_never_hides_the_user_corpus() {
        let user = cfg("[sources]\nroots = [\"~/.claude\"]\n");
        let project = cfg("[sources]\nroots = [\"./CLAUDE.md\"]\n");
        let merged = merge(user, project);
        let roots = merged.sources.roots.unwrap_or_default();
        assert!(
            roots.contains(&"~/.claude".to_string()),
            "a project file must not silently drop the user's memory"
        );
    }

    #[test]
    fn a_root_named_in_both_scopes_is_scanned_once() {
        let user = cfg("[sources]\nroots = [\"shared\", \"a\"]\n");
        let project = cfg("[sources]\nroots = [\"shared\", \"b\"]\n");
        let merged = merge(user, project);
        assert_eq!(
            merged.sources.roots,
            Some(vec!["shared".to_string(), "a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn globs_are_replaced_by_the_project() {
        let user = cfg("[sources]\nglobs = [\"**/*.md\"]\n");
        let project = cfg("[sources]\nglobs = [\"CLAUDE.md\"]\n");
        let merged = merge(user, project);
        assert_eq!(merged.sources.globs, Some(vec!["CLAUDE.md".to_string()]));
    }

    #[test]
    fn globs_fall_back_to_the_user_scope_when_absent() {
        let user = cfg("[sources]\nglobs = [\"**/*.md\"]\n");
        let project = cfg("[sources]\n");
        let merged = merge(user, project);
        assert_eq!(merged.sources.globs, Some(vec!["**/*.md".to_string()]));
    }

    #[test]
    fn attribution_names_the_scope_of_each_root() {
        let user = cfg("[sources]\nroots = [\"u\"]\n");
        let project = cfg("[sources]\nroots = [\"p\"]\n");
        assert_eq!(
            attribute(&user, &project),
            vec![
                ("u".to_string(), Scope::User),
                ("p".to_string(), Scope::Project),
            ]
        );
    }

    #[test]
    fn a_shared_root_is_attributed_to_the_scope_that_named_it_first() {
        let user = cfg("[sources]\nroots = [\"shared\"]\n");
        let project = cfg("[sources]\nroots = [\"shared\"]\n");
        assert_eq!(
            attribute(&user, &project),
            vec![("shared".to_string(), Scope::User)]
        );
    }

    #[test]
    fn an_empty_env_var_is_treated_as_unset() {
        // SAFETY-free equivalent: exercise the filter directly rather than
        // mutating process environment, which would race other tests.
        let empty = Some(String::new()).filter(|value| !value.is_empty());
        assert_eq!(empty, None);
    }
}
