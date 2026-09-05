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

use super::{Drift, Seen, UNDELIVERED, drift};
use crate::{issue, ledger};
use std::path::Path;

/// The project settings files `docs/INTEGRATION.md` names.
///
/// PROJECT scope only. A home path is not this crate's to read -- `check`
/// reads the ledger and the files the ledger names -- so a user-level
/// wiring is invisible here, and the finding SAYS that rather than
/// asserting there is none.
const SETTINGS: [&str; 2] = ["settings.json", "settings.local.json"];

/// Does anything in this project wire `rekall hook`?
///
/// Matched as TEXT, not by parsing the hook schema. The schema is the
/// harness's and it changes on their release cadence, so a parser here
/// would answer "not wired" the day they nest the key one level deeper --
/// reporting a fault that is really a version skew. The literal
/// `rekall hook` in a settings file means one thing, and JSON has no
/// comments to hide it in.
#[must_use]
pub fn wired(base: &Path) -> bool {
    SETTINGS.iter().any(|name| {
        std::fs::read_to_string(base.join(".claude").join(name))
            .is_ok_and(|text| text.contains("rekall hook"))
    })
}

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
        "{} carries `{}`, so the host will not load it, and this project wires \
         no `rekall hook` to deliver it instead -- nothing can reach the skill \
         (V56). Wire the hook (docs/INTEGRATION.md). Only this project's \
         settings were read, so a hook wired in your user settings is not \
         counted here",
        row.artifact,
        issue::GUARD
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
        assert!(said.contains("Wire the hook"), "{said}");
        assert!(said.contains("user settings"), "{said}");
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

    /// The wiring is read as TEXT from the files `INTEGRATION.md` names,
    /// and `settings.local.json` counts -- it is where a person wiring
    /// this for themselves would put it.
    #[test]
    fn wiring_is_found_in_either_settings_file() {
        let dir = std::path::PathBuf::from("target").join("check-wired");
        let claude = dir.join(".claude");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&claude);
        assert!(!wired(&dir), "nothing written yet");
        let _ = std::fs::write(
            claude.join("settings.local.json"),
            "{\"hooks\":{\"PreToolUse\":[{\"hooks\":[{\"command\":\"rekall hook\"}]}]}}",
        );
        assert!(wired(&dir));
    }

    /// A settings file that mentions the crate but not the VERB is not
    /// wiring. `rekall check` in a lint hook delivers no skill.
    #[test]
    fn another_rekall_verb_is_not_a_hook() {
        let dir = std::path::PathBuf::from("target").join("check-other-verb");
        let claude = dir.join(".claude");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&claude);
        let _ = std::fs::write(
            claude.join("settings.json"),
            "{\"hooks\":{\"PreToolUse\":[{\"hooks\":[{\"command\":\"rekall check\"}]}]}}",
        );
        assert!(!wired(&dir));
    }
}
