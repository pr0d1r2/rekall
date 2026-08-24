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
    format!("<!-- rekall {id} -->")
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

/// The artifact a step materializes.
///
/// Both forms arrive INERT and LOUD. A generated `M` script exits nonzero
/// saying it is unimplemented, so wiring it into the gate before writing
/// the check fails visibly rather than passing green; a generated `S` skill
/// carries a trigger heading and an explicit do-not-fire heading, because
/// V4 wants absence stated rather than inferred and a template that omits
/// it teaches the omission.
#[must_use]
pub fn artifact_text(step: &plan::Step) -> String {
    if step.label.starts_with('M') {
        return rule_script(step);
    }
    skill_file(step)
}

fn rule_script(step: &plan::Step) -> String {
    fill(RULE_TEMPLATE, step)
}

fn skill_file(step: &plan::Step) -> String {
    fill(SKILL_TEMPLATE, step)
}

/// Templates are CONSTS, not inline format walls. The generated artifact is
/// the thing a human edits next, so its text should be readable and
/// editable here rather than reassembled from fragments.
fn fill(template: &str, step: &plan::Step) -> String {
    template
        .replace("{ID}", &step.id)
        .replace("{SRC}", &step.src)
        .replace("{START}", &step.line_start.to_string())
        .replace("{END}", &step.line_end.to_string())
        .replace("{ARTIFACT}", &step.artifact)
        .replace("{TEXT}", step.text.trim())
}

/// What a generated runner says about itself until someone writes the
/// check, and the two headings a generated skill must end up carrying.
///
/// CONSTS because `check` reads them back out of the artifact (V2, V3,
/// V4). A template and the gate that judges it are two halves of one rule,
/// and two string literals that must match is the invisible kind of
/// divergence: each looks right alone, and the gate silently stops
/// noticing the placeholder it was written to catch. The tests below pin
/// each const to the template that carries it.
pub const UNIMPLEMENTED: &str = "is not implemented yet";
pub const FIRES: &str = "## Fires when";
pub const NOT_FIRES: &str = "## Does NOT fire when";

/// Arrives INERT and LOUD: exits nonzero until the check is written, so
/// wiring it into the gate before implementing it fails visibly rather
/// than passing green.
const RULE_TEMPLATE: &str = "\
#!/bin/sh
# Extracted by rekall from {SRC}:{START}-{END} (id {ID}).
#
# THE RULE, verbatim:
# {TEXT}
#
# Exits NONZERO until the check is written. A runner that passes without
# testing anything gates nothing, and is worse than no runner (V2, V22).
#
# ## Fires when
#
# This rule already gates at COMMIT -- that is what the wiring line in
# `rekall plan` asks you to do. The block below is different: it makes the
# rule arrive at a TOOL CALL too, before the mistake instead of after
# (V37).
#
# It arrives EMPTY, which means GATE-ONLY: nothing fires it early until
# you say when. Keys are `tool` (exact names), `path` (globs) and `word`
# (literals tested against the situation text). Within a key ANY value
# matches; across keys ALL present keys must match.
#
# ```rekall
# tool = []
# path = []
# word = []
# ```
#
# ## Does NOT fire when
#
# A match here refuses the load even when the block above matched. State
# the absence rather than leaving it inferred (V4).
#
# ```rekall
# tool = []
# path = []
# word = []
# ```
echo 'rekall: {ARTIFACT} is not implemented yet' >&2
exit 1
";

/// Carries BOTH headings. V4 wants absence stated rather than inferred, so
/// a template that omits the do-not-fire section teaches the omission.
const SKILL_TEMPLATE: &str = "\
# {ID}

Extracted by rekall from {SRC}:{START}-{END}.

{TEXT}

## Fires when

The block below IS the trigger. Prose here is for you and is never read
(V29). Keys: `tool` (exact names), `path` (globs), `word` (literals tested
against the situation text). Within a key ANY value matches; across keys
ALL present keys must match.

It arrives EMPTY, which matches nothing -- so this skill does not load
until you say when. A trigger nobody filled in should fire never, not
always: always-on prose is what this was extracted FROM (V3).

```rekall
tool = []
path = []
word = []
```

## Does NOT fire when

State the absence rather than leaving it inferred (V4). A list of what
fires says nothing about what does not, and a matcher has to decide both.
This block WINS: a match here refuses the load even when the block above
matched.

```rekall
tool = []
path = []
word = []
```
";

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
mod tests {
    use super::*;

    fn step(id: &str, start: usize, end: usize, label: &str) -> plan::Step {
        plan::Step {
            id: id.to_string(),
            src: "CLAUDE.md".to_string(),
            line_start: start,
            line_end: end,
            text: "- never commit to `main`".to_string(),
            label: label.to_string(),
            artifact: ".rekall/rules/no-main.sh".to_string(),
            wiring: "wire it".to_string(),
            net: None,
        }
    }

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

    /// V2: a runner that passes without testing anything gates nothing.
    /// The generated script fails until someone writes the check.
    #[test]
    fn a_generated_rule_script_exits_nonzero() {
        let text = artifact_text(&step("abc1234", 1, 1, "M1"));
        assert!(text.starts_with("#!/bin/sh"), "{text}");
        assert!(text.contains("exit 1"), "{text}");
        assert!(
            text.contains("- never commit to `main`"),
            "the rule is missing: {text}"
        );
    }

    /// V37, DISCOVERABLE. T39 shipped `M`-rule firing and the generated
    /// artifact said nothing about it, so the feature could only be found
    /// by reading the spec. Both blocks are now emitted, EMPTY -- which is
    /// gate-only, so behaviour is unchanged until someone fills one in.
    ///
    /// Commented, because this artifact is a SCRIPT: uncommented text
    /// would be executed. V29's block, in a file that runs.
    #[test]
    fn the_generated_runner_carries_two_parsable_empty_blocks() {
        let script = artifact_text(&step("abc1234", 1, 1, "M1"));
        for heading in [FIRES, NOT_FIRES] {
            let held = crate::trigger::parse_block(&script, heading);
            assert_eq!(
                held.as_ref().map(crate::trigger::Trigger::is_empty),
                Ok(true),
                "{heading} did not parse to an empty block: {held:?}"
            );
        }
    }

    /// The blocks are COMMENTED, or the shell would try to run them. A
    /// runner that fails on its own trigger block is worse than one with
    /// no block at all.
    #[test]
    fn the_generated_runner_keeps_its_blocks_commented() {
        let script = artifact_text(&step("abc1234", 1, 1, "M2"));
        for line in script.lines() {
            let held = line.trim();
            assert!(
                !held.starts_with("```") && !held.starts_with("## "),
                "an uncommented block line would execute: {held}"
            );
        }
    }

    /// V39: this costs NO window. The artifact is never always-on -- only
    /// the pointer is -- so the explanation is free where a longer pointer
    /// would not have been.
    #[test]
    fn the_runner_is_bigger_than_the_pointer_and_that_is_fine() {
        let script = artifact_text(&step("abc1234", 1, 1, "M1"));
        assert!(script.len() > pointer_of("abc1234").len() * 10);
    }

    /// V3 and V4: a trigger AND an explicit do-not-fire clause. A template
    /// that omitted the second would teach the omission.
    #[test]
    fn a_generated_skill_carries_both_headings() {
        let text = artifact_text(&step("abc1234", 1, 1, "S2"));
        assert!(text.contains(FIRES), "{text}");
        assert!(text.contains(NOT_FIRES), "{text}");
    }

    /// PINS the template to the const `check` reads back. If the wording
    /// moved on one side only, the gate would stop noticing the very
    /// placeholder it exists to catch -- and would report green.
    #[test]
    fn the_markers_check_reads_are_the_ones_the_templates_write() {
        assert!(
            artifact_text(&step("a", 1, 1, "M1")).contains(UNIMPLEMENTED),
            "the runner template no longer announces itself unimplemented"
        );
        let skill = artifact_text(&step("a", 1, 1, "S1"));
        assert!(skill.contains(FIRES) && skill.contains(NOT_FIRES));
    }

    /// V29: what `apply` writes must PARSE, and must arrive EMPTY. A
    /// template whose block did not parse would make every fresh
    /// extraction unreadable rather than merely unfinished, and one that
    /// arrived non-empty would load the skill somewhere nobody chose.
    #[test]
    fn the_generated_skill_carries_two_parsable_empty_blocks() {
        let skill = artifact_text(&step("abc1234", 1, 1, "S2"));
        for heading in [FIRES, NOT_FIRES] {
            let held = crate::trigger::parse_block(&skill, heading);
            assert_eq!(
                held.as_ref().map(crate::trigger::Trigger::is_empty),
                Ok(true),
                "{heading} did not parse to an empty block: {held:?}"
            );
        }
    }

    #[test]
    fn every_sharpness_of_m_generates_a_script() {
        for label in ["M1", "M2", "M3"] {
            assert!(
                artifact_text(&step("a", 1, 1, label)).starts_with("#!/bin/sh"),
                "{label}"
            );
        }
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
