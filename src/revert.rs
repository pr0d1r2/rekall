//! `rekall revert` -- the move back.
//!
//! V9 says every extraction is reversible, and the ledger is what makes
//! that mechanical: source path, line span, the ORIGINAL TEXT and the
//! artifact. Putting a statement back is REPLAY, not a rewrite. Nothing
//! here regenerates prose from a rule -- the bytes that come back are the
//! bytes that left.
//!
//! It is a MOVE in the other direction (V1). The source is restored AND
//! the artifact is removed; restoring one without removing the other would
//! leave two hand-maintained statements of one rule, which is the copy V1
//! exists to forbid.
//!
//! Like the splice it reverses, the edit is a PURE function over text.

use crate::{apply, ledger};

/// Why a revert could not be located.
///
/// Both arms are REFUSALS rather than repairs. This code puts bytes back
/// into someone's private memory, and a revert that guesses where they go
/// is worse than one that stops and says it cannot tell.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// The pointer `apply` left is gone -- the file was edited by hand, or
    /// the statement was already put back some other way.
    PointerMissing { id: String, src: String },
    /// The pointer appears more than once, so there is no single place the
    /// text belongs. Restoring at the first would be a coin toss.
    PointerAmbiguous { id: String, src: String },
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PointerMissing { id, src } => write!(
                f,
                "{src} no longer holds the pointer for `{id}` -- it was edited by hand. \
                 Run `rekall log` to see the original text and put it back yourself, \
                 or delete the ledger row if the statement is already there"
            ),
            Self::PointerAmbiguous { id, src } => write!(
                f,
                "{src} holds the pointer for `{id}` more than once -- there is no single \
                 place the text belongs. Remove the duplicate pointer, then re-run"
            ),
        }
    }
}

impl std::error::Error for Fault {}

/// Where the pointer for a row stands, as a 0-based line index.
///
/// By CONTENT, not by the line span in the ledger. The corpus is live prose
/// a human edits, so an unrelated change above the pointer moves it -- and
/// refusing a legitimate revert over that would be wrong, while trusting
/// the stale number would put the text back in the wrong place. The pointer
/// is the one address in the file that means only itself.
pub fn locate(text: &str, row: &ledger::Extracted) -> Result<usize, Fault> {
    match pointer_lines(text, row).as_slice() {
        [only] => Ok(*only),
        [] => Err(Fault::PointerMissing {
            id: row.id.clone(),
            src: row.src.clone(),
        }),
        _ => Err(Fault::PointerAmbiguous {
            id: row.id.clone(),
            src: row.src.clone(),
        }),
    }
}

/// Every line that IS this row's pointer.
///
/// Trimmed, so a formatter that indented or padded the line has not made
/// the statement unrecoverable -- the pointer is a marker, and whitespace
/// around a marker carries no meaning worth refusing over.
fn pointer_lines(text: &str, row: &ledger::Extracted) -> Vec<usize> {
    let pointer = apply::pointer_of(&row.id);
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.trim() == pointer)
        .map(|(index, _)| index)
        .collect()
}

/// Replace a row's pointer with the text it stood in for.
///
/// The exact inverse of `apply::splice`: that one turned N lines into one
/// pointer, this one turns the pointer back into the same N lines. Round
/// tripping is byte-identical, which is the property V9 actually promises
/// and the one test that matters most here asserts.
pub fn unsplice(text: &str, row: &ledger::Extracted) -> Result<String, Fault> {
    let at = locate(text, row)?;
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = lines
        .iter()
        .take(at)
        .map(|line| (*line).to_string())
        .collect();
    out.extend(row.text.lines().map(ToString::to_string));
    out.extend(
        lines
            .iter()
            .skip(at.saturating_add(1))
            .map(|line| (*line).to_string()),
    );
    Ok(finish(out, text))
}

/// Preserve whether the file ended with a newline, for the same reason
/// `apply` does: gaining or losing one shows up as a spurious change in
/// every diff of the corpus, and a revert that alters a byte it did not
/// mean to is not the verbatim replay V9 asks for.
fn finish(lines: Vec<String>, original: &str) -> String {
    let joined = lines.join("\n");
    if original.ends_with('\n') {
        return format!("{joined}\n");
    }
    joined
}

/// What a revert did, or would do.
///
/// Named BEFORE anything is written (V7). The artifact is listed even when
/// it is already gone, because "removed" and "was not there" are different
/// facts about the corpus and only one of them is a clean reversal.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Outcome {
    pub id: String,
    /// The source file the statement goes back into.
    pub restores: String,
    /// The artifact that goes away. V1: a revert that restored the source
    /// and left the artifact would leave the copy V1 forbids.
    pub removes: String,
    /// Set when the id was not in the ledger but its statement IS in the
    /// corpus. V13's no-op: work already undone is not an unknown id.
    pub already: bool,
}

/// What reverting this row would touch.
#[must_use]
pub fn preview(row: &ledger::Extracted) -> Outcome {
    Outcome {
        id: row.id.clone(),
        restores: row.src.clone(),
        removes: row.artifact.clone(),
        already: false,
    }
}

/// The outcome for an id that has nothing left to undo.
#[must_use]
pub fn nothing_to_do(id: &str) -> Outcome {
    Outcome {
        id: id.to_string(),
        already: true,
        ..Outcome::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan;

    fn step(id: &str, start: usize, end: usize) -> plan::Step {
        plan::Step {
            id: id.to_string(),
            src: "CLAUDE.md".to_string(),
            line_start: start,
            line_end: end,
            text: "- never commit to `main`".to_string(),
            label: "M1".to_string(),
            artifact: ".rekall/rules/no-main.sh".to_string(),
            wiring: "wire it".to_string(),
        }
    }

    fn row(id: &str, text: &str) -> ledger::Extracted {
        ledger::Extracted {
            id: id.to_string(),
            src: "CLAUDE.md".to_string(),
            line_start: 3,
            line_end: 3,
            text: text.to_string(),
            label: "M1".to_string(),
            artifact: ".rekall/rules/no-main.sh".to_string(),
            fires: 0,
            at: 0,
        }
    }

    /// THE PROPERTY V9 PROMISES. Extract, then put it back, and the file is
    /// the file that was there before -- byte for byte, not merely
    /// equivalent. Anything less is a rewrite wearing a revert's name.
    #[test]
    fn apply_then_revert_returns_the_original_bytes() {
        let text = "# Rules\n\n- never commit to `main`\n\n- other prose\n";
        let spliced =
            apply::splice(text, &step("abc1234", 3, 3)).unwrap_or_default();
        let back =
            unsplice(&spliced, &row("abc1234", "- never commit to `main`"));
        assert_eq!(back.ok().unwrap_or_default(), text);
    }

    #[test]
    fn a_multi_line_statement_comes_back_whole() {
        let text = "- first line\n  continued\n\n- other\n";
        let spliced =
            apply::splice(text, &step("abc1234", 1, 2)).unwrap_or_default();
        let back =
            unsplice(&spliced, &row("abc1234", "- first line\n  continued"));
        assert_eq!(back.ok().unwrap_or_default(), text);
    }

    #[test]
    fn a_file_without_a_trailing_newline_does_not_gain_one() {
        let text = "- never commit to `main`";
        let spliced =
            apply::splice(text, &step("abc1234", 1, 1)).unwrap_or_default();
        let back = unsplice(&spliced, &row("abc1234", text));
        assert_eq!(back.ok().unwrap_or_default(), text);
    }

    /// The whole reason the pointer is the address. An edit ABOVE the
    /// pointer moves its line number, and a revert keyed on the ledger's
    /// stale span would put the text back in the wrong place.
    #[test]
    fn an_edit_above_the_pointer_does_not_move_the_revert() {
        let text = "- never commit to `main`\n\n- other\n";
        let spliced =
            apply::splice(text, &step("abc1234", 1, 1)).unwrap_or_default();
        let shifted = format!("# a heading added later\n\n{spliced}");
        let back =
            unsplice(&shifted, &row("abc1234", "- never commit to `main`"))
                .unwrap_or_default();
        assert_eq!(
            back,
            "# a heading added later\n\n- never commit to `main`\n\n- other\n"
        );
    }

    /// A pointer indented or surrounded by whitespace is still THE pointer.
    /// A formatter that reflows the file should not make a statement
    /// unrecoverable.
    #[test]
    fn a_pointer_with_surrounding_whitespace_is_still_found() {
        let held = row("abc1234", "- never commit to `main`");
        let text = format!("# H\n\n  {}  \n", apply::pointer_of(&held.id));
        assert_eq!(locate(&text, &held), Ok(2));
    }

    /// The pointer is gone -- someone edited the file by hand. Restoring at
    /// the ledger's remembered line would write into whatever now stands
    /// there, so it REFUSES and names what to do instead.
    #[test]
    fn a_missing_pointer_is_refused_and_names_the_fix() {
        let held = row("abc1234", "- never commit to `main`");
        let fault = unsplice("# H\n\n- unrelated\n", &held).err();
        assert_eq!(
            fault,
            Some(Fault::PointerMissing {
                id: "abc1234".to_string(),
                src: "CLAUDE.md".to_string(),
            })
        );
        let said = fault.map(|f| f.to_string()).unwrap_or_default();
        assert!(said.contains("rekall log"), "no fix named: {said}");
    }

    /// Two pointers, one id. There is no single place the text belongs, and
    /// picking the first is a coin toss.
    #[test]
    fn a_duplicated_pointer_is_refused() {
        let held = row("abc1234", "- never commit to `main`");
        let pointer = apply::pointer_of(&held.id);
        let text = format!("{pointer}\n\n{pointer}\n");
        let fault = unsplice(&text, &held).err();
        assert_eq!(
            fault,
            Some(Fault::PointerAmbiguous {
                id: "abc1234".to_string(),
                src: "CLAUDE.md".to_string(),
            })
        );
        let said = fault.map(|f| f.to_string()).unwrap_or_default();
        assert!(said.contains("more than once"), "{said}");
    }

    /// V1 in reverse: BOTH files named. Restoring the source and leaving
    /// the artifact would leave two statements of one rule.
    #[test]
    fn the_preview_names_both_halves_of_the_move() {
        let outcome = preview(&row("abc1234", "- never commit to `main`"));
        assert_eq!(outcome.restores, "CLAUDE.md");
        assert_eq!(outcome.removes, ".rekall/rules/no-main.sh");
        assert!(!outcome.already);
    }

    /// V13: an id with nothing left to undo is a no-op, not an error.
    #[test]
    fn an_id_with_nothing_to_undo_is_marked_already_done() {
        let outcome = nothing_to_do("abc1234");
        assert!(outcome.already);
        assert!(outcome.restores.is_empty() && outcome.removes.is_empty());
    }
}
