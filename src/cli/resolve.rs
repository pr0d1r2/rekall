//! Where a verb finds out what it is working on.
//!
//! Split out of `cli.rs` when the module-size gate fired at 502 lines. The
//! seam was already there rather than invented to fit: everything here
//! answers ONE question -- what did the two config scopes agree on -- and
//! nothing here knows which verb asked. `cli.rs` keeps the dispatch, and
//! dispatch is a different job from resolution.

use crate::{classify, config};
use std::path::Path;

/// Load both config scopes and report every root with the scope that named
/// it. The union lives in `config::merge`; this keeps the attribution so
/// `--sources` can show where each root came from.
pub fn resolve(cwd: &Path) -> Result<Resolved, String> {
    let user = load_optional(config::user_path().as_deref())?;
    let project = load_optional(config::find_project(cwd).as_deref())?;
    let merged = config::merge(user.clone(), project.clone());
    Ok(Resolved {
        roots: config::attribute(&user, &project),
        globs: merged.sources.globs.unwrap_or_default(),
        weights: classify::Weights::from_config(
            merged.signals.deadband,
            &merged.signals.weight,
        ),
        runner_timeout_ms: merged.triggers.runner_timeout_ms,
    })
}

pub struct Resolved {
    pub roots: Vec<(String, config::Scope)>,
    pub globs: Vec<String>,
    /// The classifier table both scopes agreed on (V30).
    pub weights: classify::Weights,
    /// How long a fired `M` rule gets, from `[triggers]` (B5).
    pub runner_timeout_ms: Option<u64>,
}

/// A config file that is absent is fine; one that is present and broken is
/// not. Treating a parse error as "no config" would run the scan against a
/// corpus the user never described.
fn load_optional(path: Option<&Path>) -> Result<config::Config, String> {
    let Some(path) = path else {
        return Ok(config::Config::default());
    };
    if !path.is_file() {
        return Ok(config::Config::default());
    }
    config::load_file(path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absent config is fine; a broken one is not. Treating a parse
    /// error as "no config" would scan a corpus the user never described.
    #[test]
    fn an_absent_config_is_not_an_error() {
        assert!(load_optional(None).is_ok());
        assert!(
            load_optional(Some(Path::new("definitely/not/here.toml"))).is_ok()
        );
    }

    #[test]
    fn a_broken_config_is_an_error() {
        assert!(load_optional(Some(Path::new("Cargo.lock"))).is_err());
    }
}
