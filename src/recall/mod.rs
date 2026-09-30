//! `rekall recall` -- which situational skills load HERE.
//!
//! This is the reload rule V3 demands, made answerable. A skill with a
//! trigger is only better than always-on prose if something can say, at a
//! given moment, whether the trigger is met -- and say it where a human
//! can read the answer.
//!
//! REPORT-ONLY (V7). It JUDGES both classes -- V37 lets an `M` rule carry
//! a trigger too -- but it never RUNS one and never counts a firing:
//! `hook` does both, and the counter V11 reads must mean "this
//! artifact was actually loaded", not "somebody asked about it". A `recall`
//! that incremented would make `--dead` measure curiosity.
//!
//! It reports EVERY candidate with its verdict, not only the matches. V18
//! makes this the human and debug view of the decision `hook` acts on, and
//! a view that hid the misses would answer "why did my skill not load?"
//! with silence -- which is the question anyone runs this to ask.

use crate::{ledger, trigger};

/// One skill, judged.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Row {
    pub id: String,
    pub artifact: String,
    pub label: String,
    /// Whether it loads HERE.
    pub loads: bool,
    /// WHY, in the words a human needs to fix a trigger that did not fire.
    pub why: String,
}

#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub rows: Vec<Row>,
}

/// A candidate: a ledger row and the artifact text behind it.
///
/// The text arrives already read, so the decision stays a pure function
/// over bytes -- the same shape `check` uses, and for the same reason.
pub struct Candidate<'a> {
    pub row: &'a ledger::Extracted,
    pub artifact: Option<&'a str>,
}

/// Why a skill did not load, or did.
const LOADS: &str = "trigger matched";
/// An `M` rule matched.
///
/// SEPARATE from `LOADS` because the two promise different things. An `S`
/// artifact's payload is INJECTED on a match. An `M` artifact's RUNNER is
/// executed, and it speaks only if it finds a violation -- so `hook`
/// answers `{}` for a matched rule on a clean tree, and a reader told
/// only "trigger matched" would call that a contradiction (B28).
const RUNS: &str =
    "trigger matched; the rule RUNS here and speaks only if it fails";
const REFUSED: &str =
    "refused by the do-not-fire block, which WINS over a match";
const NO_MATCH: &str = "trigger did not match";
const EMPTY: &str = "trigger is empty, so it matches nothing";
const GATE_ONLY: &str = "no trigger, so it gates at commit only (V37)";
const UNREADABLE: &str = "trigger could not be read";
const GONE: &str = "artifact is missing -- run `rekall check`";

/// Judge every candidate.
///
/// BOTH classes are judged. V37 lets an `M` rule carry a trigger too --
/// its runner still gates at commit, and a trigger is how it
/// ADDITIONALLY arrives uninvited. An `M` with an empty block is
/// GATE-ONLY and says so, rather than being filtered out of the debug
/// view that exists to answer "why did this not fire".
#[must_use]
pub fn decide(candidates: &[Candidate<'_>], at: &trigger::Situation) -> Report {
    Report {
        rows: candidates.iter().map(|one| judge(one, at)).collect(),
    }
}

fn judge(one: &Candidate<'_>, at: &trigger::Situation) -> Row {
    let (loads, why) = verdict(one, at);
    Row {
        id: one.row.id.clone(),
        artifact: one.row.artifact.clone(),
        label: one.row.label.clone(),
        loads,
        why: why.to_string(),
    }
}

/// The decision, and the sentence that explains it.
///
/// The REFUSED case is reported distinctly from a plain non-match, because
/// the two send someone to different halves of the file -- and an
/// exclusion that silently looked like "did not match" would make V4's
/// clause invisible exactly when it is doing its job.
fn verdict(
    one: &Candidate<'_>,
    at: &trigger::Situation,
) -> (bool, &'static str) {
    let Some(text) = one.artifact else {
        return (false, GONE);
    };
    let (Some(fire), Some(refuse)) = (
        block(text, crate::apply::artifact::FIRES),
        block(text, crate::apply::artifact::NOT_FIRES),
    ) else {
        return (false, UNREADABLE);
    };
    weigh(&fire, &refuse, at, &one.row.label)
}

/// A block, where ABSENT is not the same as MALFORMED.
///
/// V29 says `check` refuses a block that does not PARSE. An absent one is
/// a different fact: V37 makes an empty `M` block GATE-ONLY -- the runner
/// lives in the gate and a trigger is how it additionally arrives
/// uninvited -- so a generated `M` artifact, which carries no block at
/// all, is in a legal state.
///
/// It read as `UNREADABLE` before this (B4), which made `recall` report a
/// defect on a file `check` was silently happy with. Two verbs
/// contradicting each other about the same bytes is the failure V18 exists
/// to prevent, arriving through the error type rather than through a
/// second matcher.
///
/// `None` is reserved for a block that IS there and cannot be read.
fn block(text: &str, heading: &str) -> Option<trigger::Trigger> {
    match trigger::parse_block(text, heading) {
        Ok(held) => Some(held),
        Err(trigger::Fault::Missing) => Some(trigger::Trigger::default()),
        Err(_) => None,
    }
}

/// The SAME emptiness means different things by class (V37): for an `S`
/// it is work nobody has done, for an `M` it is a deliberate choice to
/// gate at commit and not in the tool path.
fn empty_means(label: &str) -> &'static str {
    if label.starts_with('M') {
        return GATE_ONLY;
    }
    EMPTY
}

/// The order is V29's: refusal FIRST and outright.
fn weigh(
    fire: &trigger::Trigger,
    refuse: &trigger::Trigger,
    at: &trigger::Situation,
    label: &str,
) -> (bool, &'static str) {
    if fire.is_empty() {
        return (false, empty_means(label));
    }
    if trigger::matches(refuse, at) {
        return (false, REFUSED);
    }
    if trigger::matches(fire, at) {
        return (true, if label.starts_with('M') { RUNS } else { LOADS });
    }
    (false, NO_MATCH)
}

/// Human output. The verdict FIRST, so the column a reader scans is the
/// answer to the question they asked.
#[must_use]
pub fn render_human(report: &Report) -> String {
    let mut out = String::new();
    for row in &report.rows {
        let verb = if row.loads { "load" } else { "skip" };
        out.push_str(&format!(
            "{verb}    {}  {}  {}  ({})\n",
            row.id, row.label, row.artifact, row.why
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, label: &str) -> ledger::Extracted {
        ledger::Extracted {
            id: id.to_string(),
            src: "CLAUDE.md".to_string(),
            line_start: 1,
            line_end: 1,
            text: "- when editing Rust, run clippy".to_string(),
            label: label.to_string(),
            artifact: format!(".claude/skills/{id}/SKILL.md"),
            fires: 0,
            at: 0,
            issued_to: String::new(),
        }
    }

    fn skill(fire: &str, refuse: &str) -> String {
        format!(
            "# s\n\n{}\n\n```rekall\n{fire}\n```\n\n{}\n\n```rekall\n{refuse}\n```\n",
            crate::apply::artifact::FIRES,
            crate::apply::artifact::NOT_FIRES
        )
    }

    fn editing(path: &str) -> trigger::Situation {
        trigger::Situation {
            tool: Some("Edit".to_string()),
            path: Some(path.to_string()),
            cwd: None,
            text: String::new(),
        }
    }

    fn one(held: &ledger::Extracted, text: &str) -> Report {
        decide(
            &[Candidate {
                row: held,
                artifact: Some(text),
            }],
            &editing("src/main.rs"),
        )
    }

    fn first(report: &Report) -> (bool, String) {
        report
            .rows
            .first()
            .map_or((false, String::new()), |r| (r.loads, r.why.clone()))
    }

    #[test]
    fn a_matching_trigger_loads() {
        let held = row("aaa", "S1");
        let text = skill("path = [\"**/*.rs\"]", "");
        assert_eq!(first(&one(&held, &text)), (true, LOADS.to_string()));
    }

    #[test]
    fn a_trigger_that_does_not_match_is_reported_with_its_reason() {
        let held = row("aaa", "S1");
        let text = skill("path = [\"**/*.py\"]", "");
        assert_eq!(first(&one(&held, &text)), (false, NO_MATCH.to_string()));
    }

    /// V29's precedence, seen from the verb. The exclusion is reported
    /// DISTINCTLY from a plain miss -- the two send a reader to different
    /// halves of the file, and a refusal that looked like "did not match"
    /// would hide V4's clause exactly when it is working.
    #[test]
    fn a_refusal_is_named_as_a_refusal_not_a_miss() {
        let held = row("aaa", "S1");
        let text = skill("path = [\"**/*.rs\"]", "path = [\"src/**\"]");
        let (loads, why) = first(&one(&held, &text));
        assert!(!loads);
        assert_eq!(why, REFUSED.to_string());
        assert!(why.contains("WINS"), "{why}");
    }

    /// The `S3` case reaching the verb: empty means nothing matches, and
    /// the row SAYS so rather than vanishing. A skill that silently never
    /// appears is the one nobody can debug (V18).
    #[test]
    fn an_empty_trigger_is_listed_as_empty() {
        let held = row("aaa", "S3");
        let text = skill("", "");
        assert_eq!(first(&one(&held, &text)), (false, EMPTY.to_string()));
    }

    /// B4. A generated `M` artifact carries NO block -- it is a shell
    /// script with the rule in comments -- and that is GATE-ONLY, not
    /// broken. It read as "trigger could not be read" before, which
    /// reported a defect on a file `check` was happy with.
    #[test]
    fn a_generated_rule_with_no_block_is_gate_only() {
        let held = row("aaa", "M1");
        let script = "#!/bin/sh\n# THE RULE:\n# - never commit\nexit 1\n";
        assert_eq!(first(&one(&held, script)), (false, GATE_ONLY.to_string()));
    }

    /// The same absence on an `S` is WORK NOT DONE, not gate-only. One
    /// fact, two meanings, decided by the class (V37).
    #[test]
    fn a_skill_with_no_block_is_empty_not_gate_only() {
        let held = row("bbb", "S2");
        let prose = "# s\n\n## Fires when\n\nEditing Rust files.\n";
        assert_eq!(first(&one(&held, prose)), (false, EMPTY.to_string()));
    }

    /// ABSENT is benign; MALFORMED is not. A block that is THERE and will
    /// not parse still refuses, because a trigger half-understood loads a
    /// skill in cases nobody chose (V29).
    #[test]
    fn a_present_but_broken_block_is_still_unreadable() {
        let held = row("aaa", "M1");
        let text = skill("tolo = [\"Edit\"]", "");
        assert_eq!(first(&one(&held, &text)), (false, UNREADABLE.to_string()));
    }

    /// One block present, the other absent: the present one still governs
    /// rather than the absence poisoning the pair.
    #[test]
    fn a_missing_exclusion_does_not_stop_a_fire_block_matching() {
        let held = row("aaa", "S1");
        let text = format!(
            "# s\n\n{}\n\n```rekall\npath = [\"**/*.rs\"]\n```\n",
            crate::apply::artifact::FIRES
        );
        assert_eq!(first(&one(&held, &text)), (true, LOADS.to_string()));
    }

    #[test]
    fn an_unreadable_block_is_reported_not_skipped() {
        let held = row("aaa", "S2");
        let text = skill("tolo = [\"Edit\"]", "");
        assert_eq!(first(&one(&held, &text)), (false, UNREADABLE.to_string()));
    }

    #[test]
    fn a_missing_artifact_points_at_check() {
        let held = row("aaa", "S1");
        let report = decide(
            &[Candidate {
                row: &held,
                artifact: None,
            }],
            &editing("src/main.rs"),
        );
        let (loads, why) = first(&report);
        assert!(!loads);
        assert!(why.contains("rekall check"), "{why}");
    }

    /// V37: an `M` rule with a TRIGGER is judged like any other. Its
    /// runner still gates at commit; the trigger is how it additionally
    /// arrives uninvited.
    #[test]
    fn a_mechanical_rule_with_a_trigger_runs_here() {
        let held = row("aaa", "M1");
        let text = skill("path = [\"**/*.rs\"]", "");
        assert_eq!(first(&one(&held, &text)), (true, RUNS.to_string()));
    }

    /// B28. The MATCH is the same for both classes -- one matcher, V18 --
    /// and what differs is what happens next: a skill's payload is
    /// injected, a rule's runner is executed and may find nothing to say.
    #[test]
    fn a_skill_and_a_rule_match_alike_and_promise_differently() {
        let text = skill("path = [\"**/*.rs\"]", "");
        let rule = first(&one(&row("aaa", "M1"), &text));
        let skill_row = first(&one(&row("bbb", "S1"), &text));
        assert_eq!((rule.0, skill_row.0), (true, true));
        assert_ne!(rule.1, skill_row.1);
        assert!(rule.1.contains("speaks only if it fails"), "{}", rule.1);
    }

    /// The SAME emptiness, read differently by class. For an `M` it is a
    /// deliberate choice to gate at commit; for an `S` it is work nobody
    /// has done. Reporting both as "empty" would hide the difference.
    #[test]
    fn an_empty_mechanical_block_is_gate_only_not_a_gap() {
        let held = row("aaa", "M1");
        assert_eq!(
            first(&one(&held, &skill("", ""))),
            (false, GATE_ONLY.to_string())
        );
        let skill_row = row("bbb", "S2");
        assert_eq!(
            first(&one(&skill_row, &skill("", ""))),
            (false, EMPTY.to_string())
        );
    }

    /// EVERY candidate is reported, hit or miss. A view that hid the
    /// misses would answer "why did my skill not load?" with silence --
    /// the question this verb exists to answer (V18).
    #[test]
    fn misses_are_reported_beside_the_hits() {
        let (hit, miss) = (row("aaa", "S1"), row("bbb", "S1"));
        let (yes, no) = (
            skill("path = [\"**/*.rs\"]", ""),
            skill("tool = [\"Bash\"]", ""),
        );
        let report = decide(
            &[
                Candidate {
                    row: &hit,
                    artifact: Some(&yes),
                },
                Candidate {
                    row: &miss,
                    artifact: Some(&no),
                },
            ],
            &editing("src/main.rs"),
        );
        assert_eq!(
            report.rows.iter().map(|r| r.loads).collect::<Vec<_>>(),
            vec![true, false]
        );
    }

    #[test]
    fn human_output_leads_with_the_verdict() {
        let held = row("aaa", "S1");
        let text = skill("path = [\"**/*.rs\"]", "");
        let out = render_human(&one(&held, &text));
        assert!(out.starts_with("load    aaa  S1  "), "{out}");
    }

    #[test]
    fn nothing_extracted_recalls_nothing() {
        assert_eq!(render_human(&decide(&[], &editing("a.rs"))), "");
    }
}
