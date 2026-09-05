//! `rekall issue` -- the handover.
//!
//! An extraction is proven HERE and then goes to the loop that tends it.
//! `issue` writes the portable copy out and LEAVES THE LOCAL ONE STANDING
//! (V54): removing it at this point would leave the rule enforced by
//! nothing until the registry materializes it back, which is the gap the
//! extraction existed to close, reopened by the step meant to complete it.
//!
//! So the ledger row carries the stage, and it is the only thing that
//! makes two copies a TRANSITION rather than the duplication V1 forbids.
//!
//! It also acts on rows that already exist, which is the other half of the
//! verb: an artifact whose head is missing the guard V52 requires gets it
//! back, and a checkout whose host link was never made gets it made. Both
//! are `issue` over an existing row, which is why they are not two verbs.
//!
//! NOTHING HERE REGENERATES AN ARTIFACT. The file on disk is what a human
//! filled in -- the triggers above all -- and the ledger keeps the source
//! prose, not the trigger. Rewriting from the row would silently discard
//! the only part of the artifact this crate did not write.

use crate::ledger;

/// The frontmatter key that turns the HOST's own auto-loading off (V52).
pub const GUARD: &str = "disable-model-invocation: true";

/// Where a row stands in the handover.
///
/// Derived from the row and the disk together, because neither answers on
/// its own: the column says whether a registry was named, and only the
/// artifact's presence separates an open handover from a finished one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// Extracted here and nowhere else.
    Local,
    /// Issued, and both copies stand. The overlap is INTENDED.
    Issued,
    /// Issued, and the local copy has been retired. The rule lives one
    /// repo out, so a missing artifact here is the END STATE, not drift.
    Retired,
}

#[must_use]
pub fn stage(row: &ledger::Extracted, artifact_stands: bool) -> Stage {
    if row.issued_to.is_empty() {
        return Stage::Local;
    }
    if artifact_stands {
        Stage::Issued
    } else {
        Stage::Retired
    }
}

/// Does this artifact's head already carry the guard?
///
/// Read from the FRONTMATTER only. The same words in the prose below are a
/// human explaining the key, and a check that accepted them would pass an
/// artifact whose host still auto-loads it.
#[must_use]
pub fn has_guard(text: &str) -> bool {
    head_lines(text).iter().any(|line| is_guard(line))
}

fn is_guard(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("disable-model-invocation:")
        && trimmed.ends_with("true")
}

/// The frontmatter's lines, or nothing when there is no frontmatter.
fn head_lines(text: &str) -> Vec<&str> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    lines.take_while(|line| line.trim() != "---").collect()
}

/// The artifact with the guard in its head, unchanged if it is there.
///
/// An IN-PLACE repair of one line, never a regeneration. The rest of the
/// file -- the triggers a human filled in, the prose they wrote around
/// them -- is bytes this crate did not author and will not rewrite.
///
/// An artifact with NO frontmatter is returned untouched. That is `check`'s
/// `no-head` finding, and inventing a head here would clear a gate reading
/// without anyone deciding what the skill is called.
#[must_use]
pub fn guarded(text: &str) -> String {
    if head_lines(text).is_empty() || has_guard(text) {
        return text.to_string();
    }
    let mut lines: Vec<&str> = text.lines().collect();
    lines.insert(closing_line(&lines), GUARD);
    format!("{}\n", lines.join("\n"))
}

/// Where the frontmatter's CLOSING `---` sits.
///
/// Searched from the SECOND line, because the first one is the marker that
/// opened the head -- counting it would insert the guard above `name:` and
/// outside the block the host reads.
fn closing_line(lines: &[&str]) -> usize {
    lines
        .iter()
        .skip(1)
        .position(|line| line.trim() == "---")
        .map_or(lines.len(), |at| at.saturating_add(1))
}

/// What an issue would do, or did.
///
/// Every field is a PATH or empty, so the human rendering and the JSON say
/// the same thing without either restating the other's logic.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Outcome {
    pub id: String,
    /// Where the portable copy lands. Empty on a local-only reissue.
    pub issues: String,
    /// The artifact whose head this repairs. Empty when it needs none.
    pub repairs: String,
    /// The host link this makes. Empty where the host has no directory.
    pub links: String,
    /// What retirement removes. Empty unless retiring.
    pub retires: String,
    /// Nothing left to do (V13). Exit 0, and say so.
    pub already: bool,
}

/// Every row one `issue` invocation touches.
///
/// A wrapper rather than a bare `Vec`, so the JSON has a named field to
/// grow beside -- a top-level array is the one shape that cannot be
/// extended without breaking every reader (V17).
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub rows: Vec<Outcome>,
}

/// Nothing to do, named.
#[must_use]
pub fn nothing_to_do(id: &str) -> Outcome {
    Outcome {
        id: id.to_string(),
        already: true,
        ..Outcome::default()
    }
}

/// Why an issue could not proceed.
///
/// Both arms are REFUSALS. This verb writes outside the repository and, on
/// the retire path, deletes the only local copy of an enforced rule --
/// guessing at either is worse than stopping.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// Asked to retire a row that was never issued anywhere.
    NotIssued { id: String },
    /// Asked to issue an `M` rule. `issue` moves SKILLS: a rule's artifact
    /// is a script the gate runs by path, and a registry that adopted one
    /// would be adopting a wiring this crate cannot see.
    NotASkill { id: String, label: String },
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotIssued { id } => write!(
                f,
                "{id} was never issued, so there is nothing to retire it from. \
                 Run `rekall issue {id} --to <dir>` first, let the registry \
                 materialize it, and retire only then"
            ),
            Self::NotASkill { id, label } => write!(
                f,
                "{id} is {label}, a mechanical rule. `issue` moves SKILLS -- a \
                 rule's artifact is a script your gate runs by path, and moving \
                 it out would break the wiring that makes it a rule at all"
            ),
        }
    }
}

impl std::error::Error for Fault {}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(issued: &str) -> ledger::Extracted {
        ledger::Extracted {
            id: "abc1234".to_string(),
            src: "CLAUDE.md".to_string(),
            line_start: 1,
            line_end: 2,
            text: "- never commit to `main`".to_string(),
            label: "S2".to_string(),
            artifact: ".rekall/skills/no-main/SKILL.md".to_string(),
            fires: 0,
            at: 0,
            issued_to: issued.to_string(),
        }
    }

    const HEAD: &str = "---\nname: s\ndescription: \"x\"\n---\n\nbody\n";

    #[test]
    fn a_row_nobody_issued_is_local() {
        assert_eq!(stage(&row(""), true), Stage::Local);
        assert_eq!(stage(&row(""), false), Stage::Local);
    }

    /// The whole point of V54: after `issue` BOTH copies stand, and that
    /// is a stage rather than a defect.
    #[test]
    fn an_issued_row_whose_artifact_stands_is_mid_handover() {
        assert_eq!(stage(&row("../set-and-setting"), true), Stage::Issued);
    }

    /// And the same row without its artifact is FINISHED, which is what
    /// keeps `check` from calling the end state an orphan.
    #[test]
    fn an_issued_row_without_its_artifact_is_retired() {
        assert_eq!(stage(&row("../set-and-setting"), false), Stage::Retired);
    }

    #[test]
    fn the_guard_goes_in_the_head_and_only_once() {
        let once = guarded(HEAD);
        assert!(has_guard(&once));
        assert_eq!(guarded(&once), once);
    }

    /// The repair is ONE LINE. Everything a human wrote survives it,
    /// which is the difference between this and regenerating the file.
    #[test]
    fn repairing_the_head_keeps_every_other_byte() {
        let filled = "---\nname: s\ndescription: \"x\"\n---\n\n```rekall\ntool = [\"Edit\"]\n```\n";
        let out = guarded(filled);
        assert!(out.contains("tool = [\"Edit\"]"));
        assert_eq!(out.lines().count(), filled.lines().count() + 1);
    }

    /// No frontmatter is `check`'s `no-head`, not this function's problem.
    /// Inventing one here would clear a gate reading without anyone
    /// deciding what the skill is called.
    #[test]
    fn an_artifact_with_no_head_is_left_alone() {
        assert_eq!(guarded("body only\n"), "body only\n");
        assert!(!has_guard("body only\n"));
    }

    /// The words in the PROSE are a human explaining the key. Accepting
    /// them would pass an artifact the host still auto-loads.
    #[test]
    fn the_guard_is_read_from_the_head_and_not_from_the_prose() {
        let prose = format!("---\nname: s\n---\n\nSet {GUARD} to stop it.\n");
        assert!(!has_guard(&prose));
    }

    #[test]
    fn nothing_to_do_says_which_id() {
        let out = nothing_to_do("abc1234");
        assert!(out.already);
        assert_eq!(out.id, "abc1234");
    }

    /// Both faults NAME THE FIX (V28), because both stop a command
    /// someone meant to run.
    #[test]
    fn a_refusal_says_what_to_do_instead() {
        let never = Fault::NotIssued {
            id: "abc1234".to_string(),
        }
        .to_string();
        assert!(never.contains("--to <dir>"));
        let rule = Fault::NotASkill {
            id: "abc1234".to_string(),
            label: "M1".to_string(),
        }
        .to_string();
        assert!(rule.contains("script your gate runs"));
    }
}
