//! `rekall recall` -- which situational skills load HERE.
//!
//! This is the reload rule V3 demands, made answerable. A skill with a
//! trigger is only better than always-on prose if something can say, at a
//! given moment, whether the trigger is met -- and say it where a human
//! can read the answer.
//!
//! REPORT-ONLY (V7). It does not fire an `M` rule and it does not count a
//! firing: `hook` does both, and the counter V11 reads must mean "this
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
const REFUSED: &str =
    "refused by the do-not-fire block, which WINS over a match";
const NO_MATCH: &str = "trigger did not match";
const EMPTY: &str = "trigger is empty, so it matches nothing";
const UNREADABLE: &str = "trigger could not be read";
const GONE: &str = "artifact is missing -- run `rekall check`";

/// Judge every candidate.
///
/// `M` rows are skipped entirely: they are RULES, fired by `hook` at their
/// own moment, and listing them here would answer a question nobody asked
/// with rows nobody can act on.
#[must_use]
pub fn decide(candidates: &[Candidate<'_>], at: &trigger::Situation) -> Report {
    Report {
        rows: candidates
            .iter()
            .filter(|one| one.row.label.starts_with('S'))
            .map(|one| judge(one, at))
            .collect(),
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
    let (Ok(fire), Ok(refuse)) = (
        trigger::parse_block(text, crate::apply::FIRES),
        trigger::parse_block(text, crate::apply::NOT_FIRES),
    ) else {
        return (false, UNREADABLE);
    };
    weigh(&fire, &refuse, at)
}

/// The order is V29's: refusal FIRST and outright.
fn weigh(
    fire: &trigger::Trigger,
    refuse: &trigger::Trigger,
    at: &trigger::Situation,
) -> (bool, &'static str) {
    if fire.is_empty() {
        return (false, EMPTY);
    }
    if trigger::matches(refuse, at) {
        return (false, REFUSED);
    }
    if trigger::matches(fire, at) {
        return (true, LOADS);
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
        }
    }

    fn skill(fire: &str, refuse: &str) -> String {
        format!(
            "# s\n\n{}\n\n```rekall\n{fire}\n```\n\n{}\n\n```rekall\n{refuse}\n```\n",
            crate::apply::FIRES,
            crate::apply::NOT_FIRES
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

    /// `M` rows are RULES, fired by `hook` at their own moment. Listing
    /// them here would answer a question nobody asked.
    #[test]
    fn mechanical_rules_are_not_candidates() {
        let held = row("aaa", "M1");
        let text = skill("path = [\"**/*.rs\"]", "");
        assert!(one(&held, &text).rows.is_empty());
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
