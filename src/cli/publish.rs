//! Putting an artifact on disk, and into a host's index where there is one.
//!
//! Lifted out of `cli::apply` when that file passed both size caps. The seam
//! is the one the callers already draw: `issue` republishes a link from a
//! LEDGER ROW (`src/issue:T65`) and `revert` removes one, so "where does the
//! host link go" has three readers and belongs in the file named for it.
//!
//! A FILE and not a module directory: `src/cli/` is one federated node, and
//! `check` made the same trade earlier for the same measured reason.

use crate::apply;
use crate::plan;
use std::path::Path;

pub(super) fn write_artifact(
    base: &Path,
    step: &plan::Step,
) -> Result<(), String> {
    let path = base.join(&step.artifact);
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(&path, apply::artifact::text(step))
        .map_err(|error| error.to_string())?;
    if step.label.starts_with('M') {
        make_runnable(&path)?;
    }
    publish(base, step)
}

/// Link the artifact into a host's own skills directory where one exists.
///
/// A SYMLINK and not a copy: Claude Code follows a `<skill-name>` link,
/// reads the target, and loads the skill once however many paths reach it
/// (`.:R17`). So the canonical file stays single under `.rekall/` -- V1
/// holds, because a link is not a copy and cannot drift from what it
/// points at.
///
/// Only where the host directory ALREADY exists. Creating `.claude/` in a
/// repo that has none would be this crate deciding which agent someone
/// runs, which is exactly what V47 refuses to guess at.
pub(super) fn publish(base: &Path, step: &plan::Step) -> Result<(), String> {
    publish_artifact(base, &step.label, &step.artifact)
}

/// The same link, from a LEDGER ROW rather than a plan step.
///
/// `issue` republishes rows that already exist (`src/issue:T65`), and a
/// second implementation of "where does the host link go" is how the two
/// paths end up disagreeing about it.
pub(super) fn publish_artifact(
    base: &Path,
    label: &str,
    artifact: &str,
) -> Result<(), String> {
    if !label.starts_with('S') {
        return Ok(());
    }
    let Some(slug) = apply::artifact::skill_slug(artifact) else {
        return Ok(());
    };
    let hosts = base.join(".claude").join("skills");
    if !hosts.is_dir() {
        return Ok(());
    }
    link_skill(
        &hosts.join(&slug),
        &base.join(".rekall").join("skills").join(&slug),
    )
}

fn link_skill(link: &Path, target: &Path) -> Result<(), String> {
    if link.exists() || link.symlink_metadata().is_ok() {
        return Ok(());
    }
    let rel = Path::new("..").join("..").join(".rekall").join("skills");
    let _ = target;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(rel.join(name_of(link)), link)
            .map_err(|why| format!("cannot link `{}`: {why}", link.display()))
    }
    #[cfg(not(unix))]
    {
        Ok(())
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// An `M` artifact arrives EXECUTABLE. A runner nothing can execute is a
/// rule with no runner, which V2 says gates nothing -- and the failure
/// would surface as "permission denied" at a tool call rather than as
/// something `check` could tell you about.
pub(super) fn make_runnable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| error.to_string())
}
