//! `rekall apply` -- the only verb that edits the corpus.
//!
//! Extraction is a MOVE, not a copy (V1): the artifact lands, the source
//! span is DELETED, and a pointer is left where it stood. A copy would
//! leave two hand-maintained statements of one rule and would leave the
//! context cost unpaid, which is the whole purpose lost.
//!
//! The splice is a PURE function over text. Everything that decides which
//! bytes disappear is testable without a filesystem, because this is the
//! code that edits someone's memory and "it looked right" is not a
//! standard it can be held to.

use crate::{ledger, plan};

/// The note left where a statement used to be.
///
/// A pointer rather than a silent deletion. Someone reading `CLAUDE.md`
/// later needs to know the rule still exists and where it went -- prose
/// that simply vanished reads as a mistake, and the next person restates
/// it (V1's founding defect, one level down).
#[must_use]
pub fn pointer(step: &plan::Step) -> String {
    pointer_of(&step.id)
}

/// The pointer's SHAPE, defined once.
///
/// `revert` reads this line back out of the corpus to find where a
/// statement belongs (V9), so the two directions have to agree on it
/// exactly. Two format strings that must match is two rule sets, and the
/// invisible kind: each looks right alone, and the divergence only shows
/// when a revert cannot find a pointer that is plainly there.
///
/// THE ID ALONE, because the pointer is a COST (V39). It stays in the
/// corpus forever and is re-read every turn, so it is subtracted from
/// whatever the extraction saved. MEASURED: naming the artifact too cost
/// 28 tokens against an 18-token statement -- the extraction went
/// BACKWARDS. This form is ~8.
///
/// The trade is real and is worth stating: a reader with no tooling now
/// sees an opaque id where they used to see a path. `rekall log` and
/// `rekall show <id>` both resolve it, and the ledger is tracked, so the
/// answer is one command away rather than inline. Paying 20 tokens on
/// every turn of every session to save that one command is the wrong side
/// of the trade -- which is the whole argument of this crate, applied to
/// its own output. REJECTED: the artifact's basename (still ~15, and a
/// slug is not a location); a footnote index at the end of the file (one
/// pointer becomes two, and they drift apart under edits).
#[must_use]
pub fn pointer_of(id: &str) -> String {
    format!("{POINTER_OPEN}{id} -->")
}

/// The prefix every pointer starts with, named ONCE.
///
/// The splitter needs to recognize a pointer without knowing which id is
/// in it (a pointer is structure, not a statement), and `revert` needs to
/// build one for a known id. Two literals that must agree is the
/// invisible divergence this file already warns about above.
pub const POINTER_OPEN: &str = "<!-- rekall ";

/// Is this line one of the marks `apply` leaves behind?
///
/// Whitespace-insensitive, like `revert`'s own lookup: re-indenting a
/// pointer must not turn it back into corpus prose.
#[must_use]
pub fn is_pointer(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with(POINTER_OPEN) && trimmed.ends_with("-->")
}

/// Replace a statement's span with its pointer.
///
/// Line numbers are 1-based and inclusive, as `scan` reports them. A span
/// outside the file is returned UNCHANGED rather than clamped: a plan that
/// points past the end of a file is stale, and quietly editing the nearest
/// line would be the exact failure V19 exists to prevent.
#[must_use]
pub fn splice(text: &str, step: &plan::Step) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = step.line_start.checked_sub(1)?;
    if !in_range(step, lines.len(), start) {
        return None;
    }
    let mut out: Vec<String> = lines
        .get(..start)?
        .iter()
        .map(|l| (*l).to_string())
        .collect();
    out.push(pointer(step));
    out.extend(lines.get(step.line_end..)?.iter().map(|l| (*l).to_string()));
    Some(finish(out, text))
}

fn in_range(step: &plan::Step, len: usize, start: usize) -> bool {
    step.line_end <= len && step.line_start <= step.line_end && start < len
}

/// Preserve whether the file ended with a newline. Adding or removing one
/// would show up as a spurious change in every diff of the corpus.
fn finish(lines: Vec<String>, original: &str) -> String {
    let joined = lines.join("\n");
    if original.ends_with('\n') {
        return format!("{joined}\n");
    }
    joined
}

/// Apply every step that touches one file.
///
/// BOTTOM-UP, deliberately. Each splice replaces N lines with one pointer,
/// so applying a lower span first would shift every span above it and the
/// second edit would land on the wrong lines. Descending order means no
/// span moves before it is used.
#[must_use]
pub fn splice_all(text: &str, steps: &[plan::Step]) -> Option<String> {
    let mut ordered: Vec<&plan::Step> = steps.iter().collect();
    ordered.sort_by_key(|step| std::cmp::Reverse(step.line_start));
    let mut out = text.to_string();
    for step in ordered {
        out = splice(&out, step)?;
    }
    Some(out)
}

pub mod artifact;

/// A ledger row for a step, ready to record.
#[must_use]
pub fn row_for(step: &plan::Step, at: u64) -> ledger::Extracted {
    ledger::Extracted {
        id: step.id.clone(),
        src: step.src.clone(),
        line_start: step.line_start,
        line_end: step.line_end,
        text: step.text.clone(),
        label: step.label.clone(),
        artifact: step.artifact.clone(),
        fires: 0,
        at,
        issued_to: String::new(),
    }
}

/// What an apply did, or would do.
#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Outcome {
    /// EVERY file touched, named before anything is written (V7).
    pub writes: Vec<String>,
    pub edits: Vec<String>,
    /// Ids already in the ledger. V13 makes these a no-op, not an error.
    pub skipped: Vec<String>,
}

/// Which steps still need applying, and every file that would change.
///
/// Computed BEFORE any write so the caller can name the whole blast radius
/// while it is still hypothetical. V7 requires naming files before writing;
/// naming them afterwards is a receipt, not a warning.
#[must_use]
pub fn preview(steps: &[plan::Step], held: &ledger::Ledger) -> Outcome {
    let mut out = Outcome::default();
    for step in steps {
        if held.holds(&step.id) {
            out.skipped.push(step.id.clone());
            continue;
        }
        out.writes.push(step.artifact.clone());
        if !out.edits.contains(&step.src) {
            out.edits.push(step.src.clone());
        }
    }
    out
}

/// The steps `preview` decided are still outstanding.
#[must_use]
pub fn pending(steps: &[plan::Step], held: &ledger::Ledger) -> Vec<plan::Step> {
    steps
        .iter()
        .filter(|step| !held.holds(&step.id))
        .cloned()
        .collect()
}

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests {
    use super::testing::step;
    use super::*;

    #[test]
    fn a_span_is_replaced_by_its_pointer() {
        let text = "# H\n\n- never commit to `main`\n\n- another\n";
        let out =
            splice(text, &step("abc1234", 3, 3, "M1")).unwrap_or_default();
        assert!(out.contains("<!-- rekall abc1234 -->"), "{out}");
        assert!(
            !out.contains("never commit"),
            "the source text survived: {out}"
        );
        assert!(
            out.contains("- another"),
            "unrelated lines were lost: {out}"
        );
    }

    /// V1: extraction is a MOVE. A pointer, not a silent deletion -- prose
    /// that simply vanished reads as a mistake, and the next person
    /// restates the rule.
    ///
    /// V39 makes it the ID ALONE. The pointer is always-on cost, so it is
    /// subtracted from whatever the extraction saved -- and naming the
    /// artifact made that subtraction bigger than the payload.
    #[test]
    fn the_pointer_names_the_id_and_nothing_else() {
        let held = pointer(&step("abc1234", 1, 1, "M1"));
        assert_eq!(held, "<!-- rekall abc1234 -->");
    }

    /// THE POINT OF THE SHAPE, asserted as arithmetic rather than trusted.
    /// A pointer costing more than the statement it replaces makes the
    /// extraction go backwards, which is what V39 measured in the wild.
    #[test]
    fn the_pointer_is_smaller_than_a_short_statement() {
        let held = pointer(&step("abc1234", 1, 1, "M1"));
        assert!(
            held.len() < "- **Never `--no-verify`.** The gate refusing is the system working.".len(),
            "the pointer is no smaller than the prose it replaces: {held}"
        );
    }

    #[test]
    fn a_multi_line_span_collapses_to_one_pointer() {
        let text = "- first line\n  continued\n\n- other\n";
        let out =
            splice(text, &step("abc1234", 1, 2, "M1")).unwrap_or_default();
        assert!(!out.contains("continued"), "{out}");
        assert_eq!(out.lines().count(), 3, "{out}");
    }

    #[test]
    fn a_trailing_newline_is_preserved() {
        let text = "- never commit to `main`\n";
        assert!(
            splice(text, &step("a", 1, 1, "M1"))
                .unwrap_or_default()
                .ends_with('\n')
        );
    }

    #[test]
    fn a_file_without_a_trailing_newline_does_not_gain_one() {
        let text = "- never commit to `main`";
        let out = splice(text, &step("a", 1, 1, "M1")).unwrap_or_default();
        assert!(!out.ends_with('\n'), "a newline appeared: {out:?}");
    }

    /// A span past the end of the file means the plan is STALE. Clamping to
    /// the nearest line would edit text nobody chose -- exactly the failure
    /// V19's fingerprint exists to prevent, arriving through a different
    /// door.
    #[test]
    fn a_span_past_the_end_of_the_file_is_refused() {
        let text = "- only line\n";
        assert_eq!(splice(text, &step("a", 5, 5, "M1")), None);
        assert_eq!(splice(text, &step("a", 1, 9, "M1")), None);
    }

    #[test]
    fn a_zero_or_inverted_span_is_refused() {
        let text = "- only line\n";
        assert_eq!(splice(text, &step("a", 0, 1, "M1")), None);
        assert_eq!(splice(text, &step("a", 2, 1, "M1")), None);
    }

    /// THE ORDERING HAZARD. Each splice replaces N lines with one pointer,
    /// so applying the LOWER span first shifts every span above it and the
    /// second edit lands on the wrong lines. Bottom-up means no span moves
    /// before it is used.
    #[test]
    fn two_statements_in_one_file_both_land_correctly() {
        let text = "- first rule\n\n- second rule\n\n- third rule\n";
        let steps = vec![step("aaa", 1, 1, "M1"), step("ccc", 5, 5, "M1")];
        let out = splice_all(text, &steps).unwrap_or_default();
        assert!(out.contains("rekall aaa"), "{out}");
        assert!(out.contains("rekall ccc"), "{out}");
        assert!(
            out.contains("- second rule"),
            "the untouched rule moved: {out}"
        );
        assert!(
            !out.contains("- first rule") && !out.contains("- third rule"),
            "{out}"
        );
    }

    /// The same set in the other order must produce the same file. If it
    /// did not, the result would depend on the order ids were typed.
    #[test]
    fn the_order_ids_are_given_does_not_change_the_result() {
        let text = "- first rule\n\n- second rule\n\n- third rule\n";
        let forward = vec![step("aaa", 1, 1, "M1"), step("ccc", 5, 5, "M1")];
        let backward = vec![step("ccc", 5, 5, "M1"), step("aaa", 1, 1, "M1")];
        assert_eq!(splice_all(text, &forward), splice_all(text, &backward));
    }

    #[test]
    fn one_bad_span_refuses_the_whole_file() {
        let text = "- first rule\n";
        let steps = vec![step("aaa", 1, 1, "M1"), step("bbb", 9, 9, "M1")];
        assert_eq!(splice_all(text, &steps), None);
    }

    #[test]
    fn a_ledger_row_carries_the_span_and_the_text() {
        let row = row_for(&step("abc1234", 3, 4, "M1"), 1_700_000_000);
        assert_eq!((row.line_start, row.line_end), (3, 4));
        assert_eq!(row.text, "- never commit to `main`");
        assert_eq!(row.fires, 0);
        assert_eq!(row.at, 1_700_000_000);
    }

    /// V7: every file named BEFORE anything is written. Naming them
    /// afterwards is a receipt, not a warning.
    #[test]
    fn preview_names_every_file_that_would_change() {
        let steps = vec![step("aaa", 1, 1, "M1"), step("bbb", 3, 3, "M1")];
        let outcome = preview(&steps, &ledger::Ledger::default());
        assert_eq!(outcome.writes.len(), 2);
        assert_eq!(
            outcome.edits,
            vec!["CLAUDE.md".to_string()],
            "one file, named once"
        );
        assert!(outcome.skipped.is_empty());
    }

    /// V13: `apply` of an already-extracted id is a NO-OP, not an error.
    #[test]
    fn an_already_extracted_id_is_skipped_not_reapplied() {
        let mut held = ledger::Ledger::default();
        held.record(row_for(&step("aaa", 1, 1, "M1"), 0));
        let steps = vec![step("aaa", 1, 1, "M1"), step("bbb", 3, 3, "M1")];
        let outcome = preview(&steps, &held);
        assert_eq!(outcome.skipped, vec!["aaa".to_string()]);
        assert_eq!(outcome.writes.len(), 1);
        assert_eq!(pending(&steps, &held).len(), 1);
    }

    #[test]
    fn applying_nothing_new_reports_no_changes() {
        let mut held = ledger::Ledger::default();
        held.record(row_for(&step("aaa", 1, 1, "M1"), 0));
        let outcome = preview(&[step("aaa", 1, 1, "M1")], &held);
        assert!(outcome.writes.is_empty() && outcome.edits.is_empty());
    }
}
