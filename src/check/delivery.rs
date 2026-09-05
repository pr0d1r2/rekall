//! Whether anything can actually LOAD a guarded skill.
//!
//! `src/apply:V52` switches the host's own auto-loading off and hands
//! delivery to `rekall hook`. That trade is only sound where `hook` is
//! wired: where it is not, the artifact is indexed by nothing, loaded by
//! nothing and fired by nothing -- and every other check passes it, because
//! every other check reads the FILE and the fault is in what surrounds it.
//!
//! B12 is that state, found in this crate's own repository on the day the
//! guard landed.

use super::{DANGLING_LINK, Drift, Seen, UNDELIVERED, drift};
use crate::{apply, issue, ledger};
use std::path::{Path, PathBuf};

/// Every guarded `S` artifact that nothing can deliver.
///
/// Silent when `hook` IS wired, and silent for `M` rules: a rule's runner
/// is executed by a gate, by path, and needs no indexer at all.
#[must_use]
pub fn undelivered(seen: &[Seen<'_>], wired: bool) -> Vec<Drift> {
    if wired {
        return Vec::new();
    }
    seen.iter().filter_map(unreachable).collect()
}

fn unreachable(seen: &Seen<'_>) -> Option<Drift> {
    let row = seen.row;
    let text = seen.artifact?;
    if !row.label.starts_with('S') || !issue::has_guard(text) {
        return None;
    }
    Some(drift(UNDELIVERED, &row.id, &row.artifact, said(row)))
}

fn said(row: &ledger::Extracted) -> String {
    format!(
        "{} carries `{}`, so the host will not load it, and nothing this check \
         can see delivers it instead (V56). Wire `rekall hook` \
         (docs/INTEGRATION.md). ONLY this project's `.claude/` settings were \
         read: a hook in your user settings, or in another harness's config \
         such as Codex's, is real wiring this cannot see",
        row.artifact,
        issue::GUARD
    )
}

/// Where a host indexes skills, relative to the project root.
const HOST_SKILLS: [&str; 2] = [".claude", "skills"];

/// Every published link that points at nothing.
///
/// `src/apply:V48` has claimed since T62 that `check` verifies the link
/// resolves. It did not, and B14 records that the row was flipped done
/// anyway -- a dangling `.claude/skills/<slug>` passed the gate clean while
/// the host kept an entry for a skill that could not be read.
///
/// Two things keep this from claiming other people's files. It reports only
/// links whose TARGET points into `.rekall/skills`, because that is a link
/// this crate would have written and anything else belongs to whoever made
/// it. And it stays silent where a ledger row already names the missing
/// artifact: that is one cause, and `missing-artifact` already names the
/// fix that also repairs the link.
#[must_use]
pub fn dangling(base: &Path, seen: &[Seen<'_>]) -> Vec<Drift> {
    let Ok(entries) = std::fs::read_dir(host_dir(base)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|link| ours_and_broken(link))
        .filter(|link| !already_named(link, seen))
        .map(|link| one_dangling(&link))
        .collect()
}

/// One finding, addressed the way every other finding is.
///
/// PROJECT-RELATIVE, built from the parts rather than stripped off the
/// filesystem path. An absolute path here would be the only one in the
/// report, and `--format json` is written into CI artifacts where a
/// machine's home directory has no business appearing.
fn one_dangling(link: &Path) -> Drift {
    let shown = shown_as(link);
    drift(DANGLING_LINK, "", &shown, said_dangling(&shown))
}

fn shown_as(link: &Path) -> String {
    let slug = link
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("{}/{}/{slug}", HOST_SKILLS[0], HOST_SKILLS[1])
}

fn host_dir(base: &Path) -> PathBuf {
    HOST_SKILLS
        .iter()
        .fold(base.to_path_buf(), |at, part| at.join(part))
}

/// A symlink WE would have written, pointing at nothing.
///
/// `exists` follows the link, so a false answer on something that has
/// `symlink_metadata` is exactly "the target is not there".
fn ours_and_broken(link: &Path) -> bool {
    if link.symlink_metadata().is_err() || link.exists() {
        return false;
    }
    std::fs::read_link(link).is_ok_and(|to| {
        to.components().any(|part| part.as_os_str() == ".rekall")
    })
}

/// Is a ledger row already answering for this?
///
/// A row whose artifact is missing produces `missing-artifact`, and
/// restoring that file repairs the link too. Reporting both would name one
/// cause twice and offer two fixes for it.
fn already_named(link: &Path, seen: &[Seen<'_>]) -> bool {
    let slug = link
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    seen.iter()
        .any(|one| apply::artifact::skill_slug(&one.row.artifact) == slug)
}

fn said_dangling(shown: &str) -> String {
    format!(
        "{shown} is a published link pointing at a skill that is not there, so \
         the host still indexes an entry it cannot read (`src/apply:V48`). \
         DELETE it: no ledger row names that skill, so nothing will republish \
         it -- `rekall issue` only relinks rows the ledger still holds"
    )
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::{Seen, UNDELIVERED};
    use super::*;

    fn guarded_seen(held: &ledger::Extracted) -> Vec<Seen<'_>> {
        vec![Seen {
            row: held,
            source: None,
            artifact: Some(SKILL),
        }]
    }

    /// B12: the guard turns host loading OFF, so with no hook wired the
    /// skill is reachable by nothing at all -- and every other check
    /// passes it, because they read the file and the fault is outside it.
    /// A project with a host skills directory and nothing in it yet.
    fn linked_project(name: &str) -> PathBuf {
        let dir = std::path::PathBuf::from("target")
            .join("dangling")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(host_dir(&dir));
        dir
    }

    fn link(dir: &Path, at: &str, to: &str) {
        let _ = std::os::unix::fs::symlink(to, host_dir(dir).join(at));
    }

    fn kinds(found: &[Drift]) -> Vec<&str> {
        found.iter().map(|one| one.kind).collect()
    }

    /// B14: `src/apply:V48` has claimed since T62 that `check` verifies the
    /// link resolves. It did not, and a dangling entry passed the gate
    /// clean while the host kept indexing a skill it could not read.
    #[test]
    fn a_published_link_pointing_at_nothing_is_drift() {
        let dir = linked_project("ghost");
        link(&dir, "ghost", "../../.rekall/skills/ghost");
        assert_eq!(kinds(&dangling(&dir, &[])), vec![DANGLING_LINK]);
    }

    /// The advice has to be a fix that WORKS. `rekall issue` relinks rows
    /// the ledger holds, and this finding only ever fires for a link no
    /// row names -- MEASURED: `issue --all` republished the live row and
    /// left the stale link exactly where it was.
    #[test]
    fn the_advice_says_delete_and_not_reissue() {
        let dir = linked_project("advice");
        link(&dir, "ghost", "../../.rekall/skills/ghost");
        let found = dangling(&dir, &[]);
        let said = found
            .first()
            .map(|one| one.said.clone())
            .unwrap_or_default();
        assert!(said.contains("DELETE it"), "{said}");
        assert!(said.contains("only relinks rows"), "{said}");
    }

    /// PROJECT-RELATIVE, like every other finding. `--format json` is
    /// written into CI artifacts, where a machine's home directory has no
    /// business appearing.
    #[test]
    fn the_finding_is_addressed_relative_to_the_project() {
        let dir = linked_project("relative");
        link(&dir, "ghost", "../../.rekall/skills/ghost");
        let found = dangling(&dir, &[]);
        let path = found
            .first()
            .map(|one| one.path.clone())
            .unwrap_or_default();
        assert_eq!(path, ".claude/skills/ghost");
    }

    /// A link that RESOLVES is the normal published state and says
    /// nothing.
    #[test]
    fn a_link_that_resolves_is_silent() {
        let dir = linked_project("live");
        let target = dir.join(".rekall").join("skills").join("live");
        let _ = std::fs::create_dir_all(&target);
        let _ = std::fs::write(target.join("SKILL.md"), "---\nname: s\n---\n");
        link(&dir, "live", "../../.rekall/skills/live");
        assert!(dangling(&dir, &[]).is_empty());
    }

    /// A broken link pointing somewhere else is SOMEBODY ELSE'S. This
    /// crate publishes into `.rekall/skills` and claims nothing beyond
    /// what it would have written itself.
    #[test]
    fn a_broken_link_this_crate_did_not_write_is_left_alone() {
        let dir = linked_project("foreign");
        link(&dir, "theirs", "../../somewhere-else/theirs");
        assert!(dangling(&dir, &[]).is_empty());
    }

    /// ONE CAUSE, ONE FINDING. A row whose artifact is missing already
    /// produces `missing-artifact`, and restoring that file repairs the
    /// link too -- so reporting both would offer two fixes for one fault.
    #[test]
    fn a_link_a_ledger_row_already_answers_for_is_not_reported_twice() {
        let dir = linked_project("dedup");
        link(&dir, "no-main", "../../.rekall/skills/no-main");
        let mut held = row("S1");
        held.artifact = ".rekall/skills/no-main/SKILL.md".to_string();
        let seen = vec![Seen {
            row: &held,
            source: None,
            artifact: None,
        }];
        assert!(dangling(&dir, &seen).is_empty());
    }

    /// No host directory at all is the common case, not a fault.
    #[test]
    fn a_project_with_no_host_directory_says_nothing() {
        let dir = std::path::PathBuf::from("target").join("dangling-absent");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(dangling(&dir, &[]).is_empty());
    }

    #[test]
    fn a_guarded_skill_with_no_hook_wired_is_drift() {
        let held = row("S1");
        let found = undelivered(&guarded_seen(&held), false);
        assert_eq!(
            found.iter().map(|one| one.kind).collect::<Vec<_>>(),
            vec![UNDELIVERED]
        );
    }

    /// And it says what it did NOT look at. A user-level hook is real
    /// wiring that this check cannot see, so the line must not read as
    /// proof that none exists.
    #[test]
    fn the_finding_names_the_fix_and_its_own_blind_spot() {
        let held = row("S1");
        let found = undelivered(&guarded_seen(&held), false);
        let said = found
            .first()
            .map(|one| one.said.clone())
            .unwrap_or_default();
        assert!(said.contains("Wire `rekall hook`"), "{said}");
        assert!(said.contains("user settings"), "{said}");
        assert!(said.contains("Codex"), "another harness counts: {said}");
    }

    #[test]
    fn a_wired_project_is_silent() {
        let held = row("S1");
        assert!(undelivered(&guarded_seen(&held), true).is_empty());
    }

    /// An `M` rule needs no indexer: its runner is executed by a gate, by
    /// path. Reporting one here would be noise on every rule in the
    /// ledger.
    #[test]
    fn a_mechanical_rule_is_not_undelivered() {
        let held = row("M1");
        let seen = vec![Seen {
            row: &held,
            source: None,
            artifact: Some("#!/bin/sh\nexit 0\n"),
        }];
        assert!(undelivered(&seen, false).is_empty());
    }

    /// An UNGUARDED skill is still loaded by the host, so it is reachable
    /// whether or not a hook exists. `no-guard` is that row's finding.
    #[test]
    fn an_unguarded_skill_is_reachable_without_a_hook() {
        let held = row("S1");
        let bare = SKILL.replace("disable-model-invocation: true\n", "");
        let seen = vec![Seen {
            row: &held,
            source: None,
            artifact: Some(&bare),
        }];
        assert!(undelivered(&seen, false).is_empty());
    }
}
