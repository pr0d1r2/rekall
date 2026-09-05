//! What `check` reports WITHOUT failing.
//!
//! Split from the audit when the module hit its size cap, along the seam
//! the types already drew: a `Drift` fails the gate and a `Note` never
//! does, and keeping them in one file made every reader check which list
//! a finding was going into.

use crate::issue;

/// Something true and worth saying that is NOT drift.
///
/// A separate type rather than a `Drift` with a severity field, because
/// the exit code is the contract: section I fixes 1 for drift, and a
/// finding that must not fail the gate has no business travelling in the
/// list the gate counts. Same three questions, so the shape matches.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Note {
    pub kind: &'static str,
    pub id: String,
    pub path: String,
    pub said: String,
}

/// What is true about these rows without being wrong with them.
///
/// Today that is one thing: an extraction that has been issued and whose
/// local artifact still stands. Both copies standing is the STAGE
/// `src/issue:V54` describes, and this report is the only thing that stops
/// it quietly becoming permanent -- so it is said on every run, and it
/// fails nothing.
#[must_use]
pub fn notes(seen: &[super::Seen<'_>]) -> Vec<Note> {
    seen.iter().filter_map(handover_note).collect()
}

fn handover_note(seen: &super::Seen<'_>) -> Option<Note> {
    let row = seen.row;
    if issue::stage(row, seen.artifact.is_some()) != issue::Stage::Issued {
        return None;
    }
    Some(Note {
        kind: super::HANDOVER_OPEN,
        id: row.id.clone(),
        path: row.artifact.clone(),
        said: format!(
            "{} was issued to {} and still stands here. That is the handover, \
             not a fault -- retire the local copy with `rekall issue {} --retire` \
             once the registry has materialized it",
            row.artifact, row.issued_to, row.id
        ),
    })
}

/// Notes read like drift and are marked as NOT it.
///
/// The `note` prefix is what keeps a green run legible: a line that
/// looks exactly like a failure, on a gate that passed, is a line people
/// learn to ignore -- and then ignore the failures too.
#[must_use]
pub fn render_notes(notes: &[Note]) -> String {
    let mut out = String::new();
    for one in notes {
        out.push_str(&format!("note  {:<13} {}\n", one.kind, one.said));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::testing::*;
    use super::super::{HANDOVER_OPEN, Seen, audit};
    use super::*;

    /// Both copies standing is the HANDOVER. It is reported on every run
    /// and it fails nothing -- the report is the only thing stopping the
    /// overlap becoming permanent.
    #[test]
    fn an_open_handover_is_a_note_and_not_drift() {
        let mut held = row("S1");
        held.issued_to = "../set-and-setting".to_string();
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(SKILL),
        }];
        assert!(audit(&seen, &[]).is_empty(), "not drift");
        let said = notes(&seen);
        assert_eq!(said.len(), 1);
        assert!(said.iter().all(|one| one.kind == HANDOVER_OPEN));
        assert!(render_notes(&said).starts_with("note "));
    }

    /// A row nobody issued has nothing to say.
    #[test]
    fn a_local_extraction_gets_no_note() {
        let held = row("S1");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(SKILL),
        }];
        assert!(notes(&seen).is_empty());
    }
}
