//! Signals -> class x sharpness.
//!
//! Every verdict carries the SIGNALS that produced it (V10), because a
//! class is a CLAIM rather than truth and a claim nobody can inspect is one
//! nobody can argue with. `show` prints them; `scan` lists their names.
//!
//! `U` is legal and is the default. A classifier that never says "I do not
//! know" is lying at a fixed rate, so an absent signal and a tied signal
//! both land there rather than being rounded to the nearest verdict.

use std::fmt;

mod signals;

pub use signals::{Form, Weights};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Mechanical: becomes a rule with a runner.
    M,
    /// Situational: becomes a skill with a trigger.
    S,
    /// Unknown. Legal, and the default.
    U,
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::M => "M",
            Self::S => "S",
            Self::U => "U",
        };
        f.write_str(name)
    }
}

/// One signal that fired, and what it was WORTH.
///
/// The weight travels with the name because V10 makes a class a CLAIM and
/// V30 makes the class a BALANCE -- so a reader arguing with a verdict
/// needs to see which side each signal pulled and how hard, not just that
/// it fired.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Signal {
    pub name: String,
    pub weight: i32,
}

/// A class and, unless `U`, how sharp a runner or trigger the statement
/// admits. `U` carries no sharpness: absence of a class has no ladder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub class: Class,
    pub sharpness: Option<u8>,
    pub signals: Vec<Signal>,
    /// The sum the class was decided on (V30). Reported so the deadband is
    /// visible rather than inferred from a verdict that looks arbitrary.
    pub score: i32,
}

impl Verdict {
    /// The joined form section I prints: `M2`, `S1`, or a bare `U`.
    #[must_use]
    pub fn label(&self) -> String {
        match self.sharpness {
            Some(level) => format!("{}{level}", self.class),
            None => self.class.to_string(),
        }
    }

    fn unknown(signals: Vec<Signal>, score: i32) -> Self {
        Self {
            class: Class::U,
            sharpness: None,
            signals,
            score,
        }
    }

    /// Signal names, for the `scan` row -- whose anatomy section I fixes
    /// and where the weight has no column.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.signals.iter().map(|s| s.name.clone()).collect()
    }
}

/// Judgment adjectives: mechanically DETECTABLE at best, never resolvable.
/// These are what separate M3 from M1.
const JUDGMENT: [&str; 9] = [
    "clean",
    "readable",
    "simple",
    "deep",
    "complex",
    "appropriate",
    "reasonable",
    "clear",
    "idiomatic",
];

/// Nouns naming a class of situation rather than an exact one. S2, not S1.
const DOMAIN: [&str; 8] = [
    "test",
    "commit",
    "review",
    "doc",
    "refactor",
    "debug",
    "deploy",
    "migration",
];

fn contains_any(
    haystack: &str,
    needles: &[&'static str],
) -> Option<&'static str> {
    needles
        .iter()
        .find(|needle| haystack.contains(*needle))
        .copied()
}

/// An EXACT handle: something a matcher can test without interpreting it --
/// a backticked token, a file extension, or a path separator.
fn has_exact_handle(raw: &str, lower: &str) -> bool {
    raw.contains('`') || lower.contains('/') || has_extension(lower)
}

fn has_extension(lower: &str) -> bool {
    lower
        .split_whitespace()
        .any(|word| word.starts_with('.') && word.len() > 1)
}

fn has_number(lower: &str) -> bool {
    lower.chars().any(|c| c.is_ascii_digit())
}

/// Sharpness for a mechanical statement: how sharp a RUNNER it admits.
fn mechanical_sharpness(
    raw: &str,
    lower: &str,
    signals: &mut Vec<Signal>,
) -> u8 {
    if let Some(word) = contains_any(lower, &JUDGMENT) {
        signals.push(ladder(word));
        return 3;
    }
    if has_number(lower) {
        signals.push(ladder("threshold"));
        return 2;
    }
    if has_exact_handle(raw, lower) {
        signals.push(ladder("exact-handle"));
        return 1;
    }
    2
}

/// A SHARPNESS signal carries weight ZERO. It says which rung, never which
/// class -- V30 keeps those two questions apart, and a ladder signal that
/// nudged the sum would smuggle one into the other.
fn ladder(name: &str) -> Signal {
    Signal {
        name: name.to_string(),
        weight: 0,
    }
}

/// Sharpness for a situational statement: how sharp a TRIGGER it admits.
fn situational_sharpness(
    raw: &str,
    lower: &str,
    signals: &mut Vec<Signal>,
) -> u8 {
    if has_exact_handle(raw, lower) {
        signals.push(ladder("exact-handle"));
        return 1;
    }
    if let Some(noun) = contains_any(lower, &DOMAIN) {
        signals.push(ladder(noun));
        return 2;
    }
    3
}

/// Classify one statement.
///
/// The CLASS turns on whether the ACTION is enforceable: an absolute
/// directive is mechanical, a hedged one is situational. A conditional
/// opener does NOT decide the class on its own against a directive --
/// "when editing `.rs`, never unwrap" is still a lint -- but it does lean
/// situational when nothing else pulls.
///
/// The class is a SUM and the sharpness is a LADDER (V30). Those are
/// different kinds of question: how enforceable the action is admits a
/// balance of evidence, while which KIND of runner a statement admits does
/// not -- a total cannot say "needs one human-set parameter".
#[must_use]
pub fn classify(text: &str, form: Form, weights: &Weights) -> Verdict {
    let lower = text.to_lowercase();
    let signals = weights.fired(&lower, form);
    let score: i32 = signals.iter().map(|s| s.weight).sum();
    decide(
        text,
        &lower,
        Balance {
            signals,
            score,
            deadband: weights.deadband,
        },
    )
}

/// Classify with the shipped defaults, as a LIST ITEM.
///
/// The convenience the tests and every caller without a config file use.
/// It picks the form a rule is normally written in (V40), so the default
/// path exercises the whole vocabulary; a paragraph is classified by
/// naming `Form::Paragraph` at the call, which is the case worth being
/// explicit about.
#[must_use]
pub fn classify_default(text: &str) -> Verdict {
    classify(text, Form::ListItem, &Weights::default())
}

/// Positive is MECHANICAL, negative is SITUATIONAL, and the DEADBAND in
/// between is `U`.
///
/// The deadband is V10 stated as arithmetic rather than as a special case:
/// a directive that is also hedged sums to zero and lands on "I do not
/// know" because the evidence genuinely cancels, not because a branch was
/// written to catch it.
fn decide(raw: &str, lower: &str, balance: Balance) -> Verdict {
    if balance.score.saturating_abs() <= balance.deadband {
        return Verdict::unknown(balance.signals, balance.score);
    }
    if balance.score > 0 {
        return mechanical(raw, lower, balance.signals, balance.score);
    }
    situational(raw, lower, balance.signals, balance.score)
}

/// What the signals came to, and how close to a tie still counts as
/// undecided. Bundled when the four-argument limit fired: these three are
/// one thought -- the evidence and the bar it has to clear.
struct Balance {
    signals: Vec<Signal>,
    score: i32,
    deadband: i32,
}

fn mechanical(
    raw: &str,
    lower: &str,
    mut signals: Vec<Signal>,
    score: i32,
) -> Verdict {
    let sharpness = mechanical_sharpness(raw, lower, &mut signals);
    Verdict {
        class: Class::M,
        sharpness: Some(sharpness),
        signals,
        score,
    }
}

fn situational(
    raw: &str,
    lower: &str,
    mut signals: Vec<Signal>,
    score: i32,
) -> Verdict {
    let sharpness = situational_sharpness(raw, lower, &mut signals);
    Verdict {
        class: Class::S,
        sharpness: Some(sharpness),
        signals,
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(text: &str) -> String {
        classify_default(text).label()
    }

    #[test]
    fn an_absolute_rule_with_an_exact_handle_is_m1() {
        assert_eq!(label("never commit to `main`"), "M1");
    }

    #[test]
    fn a_rule_with_a_threshold_is_m2() {
        assert_eq!(label("functions must be under 15 lines"), "M2");
    }

    #[test]
    fn a_rule_resting_on_judgment_is_m3() {
        assert_eq!(label("code must be readable"), "M3");
    }

    #[test]
    fn a_trigger_naming_an_exact_thing_is_s1() {
        assert_eq!(label("when editing `.tf` files, prefer modules"), "S1");
    }

    #[test]
    fn a_trigger_naming_a_class_of_situation_is_s2() {
        assert_eq!(
            label("when writing tests, prefer table-driven cases"),
            "S2"
        );
    }

    #[test]
    fn a_semantic_trigger_is_s3() {
        assert_eq!(
            label("if the user seems stuck, consider offering options"),
            "S3"
        );
    }

    /// The class turns on the ACTION, not on the presence of a condition.
    /// "when editing .rs, never unwrap" is a lint, not a skill.
    #[test]
    fn a_conditional_does_not_make_an_enforceable_rule_situational() {
        let verdict = classify_default("when editing `.rs`, never use unwrap");
        assert_eq!(verdict.class, Class::M);
    }

    #[test]
    fn prose_with_no_signal_is_unknown() {
        let verdict = classify_default("this project started in 2024");
        assert_eq!(verdict.class, Class::U);
        assert_eq!(verdict.sharpness, None);
    }

    /// V10: U is legal and is the DEFAULT.
    #[test]
    fn unknown_carries_no_sharpness() {
        assert_eq!(label("some background prose"), "U");
    }

    /// A rule and a hedge in one sentence is an ambiguity in the PROSE,
    /// and it now reports itself as ARITHMETIC. The tree this replaced
    /// pushed a synthetic `conflict` signal; the sum needs no such token,
    /// because +2 and -2 cancelling IS the statement that the evidence is
    /// balanced. V10's "I do not know" falls out rather than being caught
    /// by a branch written to notice it.
    #[test]
    fn a_directive_that_is_also_hedged_is_unknown_and_says_why() {
        let verdict = classify_default("always prefer the simpler option");
        assert_eq!(verdict.class, Class::U);
        assert_eq!(verdict.score, 0);
        assert_eq!(
            verdict.names(),
            vec!["always".to_string(), "prefer".to_string()],
            "both sides must be visible, or the tie is unarguable"
        );
    }

    /// THE ONE VERDICT THE DEFAULTS CHANGE, recorded rather than
    /// discovered later. The tree let a conflict win outright, so a hedged
    /// CONDITIONAL rule was `U`; the sum makes it -1 and therefore `S`.
    ///
    /// Deliberate: "when reviewing, always prefer the simpler option" is a
    /// situational preference, and `S` says that where `U` said nothing.
    /// No existing test covered the combination, which is why it is
    /// pinned here now.
    #[test]
    fn a_hedged_conditional_rule_leans_situational_not_unknown() {
        let verdict = classify_default(
            "when reviewing, always prefer the simpler option",
        );
        assert_eq!(verdict.score, -1);
        assert_eq!(verdict.class, Class::S);
    }

    /// V30: SHARPNESS signals carry weight ZERO. A ladder signal that
    /// nudged the sum would smuggle "which rung" into "which class".
    #[test]
    fn ladder_signals_do_not_move_the_class() {
        let verdict = classify_default("never commit to `main`");
        let ladder: Vec<i32> = verdict
            .signals
            .iter()
            .filter(|s| s.name == "exact-handle")
            .map(|s| s.weight)
            .collect();
        assert_eq!(ladder, vec![0], "{:?}", verdict.signals);
        assert_eq!(verdict.score, 2, "only `never` should have counted");
    }

    /// The TABLE is the point: a corpus tunes its own vocabulary without a
    /// code change. Here a house word the shipped defaults never heard of
    /// turns a `U` into an `M`, which is how a corpus that states its
    /// rules in its own dialect gets classified at all.
    #[test]
    fn a_configured_word_classifies_what_the_defaults_could_not() {
        let text = "release notes ship beside the tag";
        assert_eq!(classify_default(text).class, Class::U);
        let mut tuned = Weights::default();
        tuned.weight.insert("ship beside".to_string(), 2);
        assert_eq!(classify(text, Form::ListItem, &tuned).class, Class::M);
    }

    /// The DEADBAND widens what counts as "I do not know". A corpus full
    /// of near-ties can ask for more evidence before a verdict.
    #[test]
    fn a_wider_deadband_withholds_a_weak_verdict() {
        let text = "when editing `.rs`, never use unwrap";
        assert_eq!(classify_default(text).score, 1);
        assert_eq!(classify_default(text).class, Class::M);
        let cautious = Weights {
            deadband: 1,
            ..Weights::default()
        };
        assert_eq!(classify(text, Form::ListItem, &cautious).class, Class::U);
    }

    /// A conditional opener is a WORD, not a prefix. Without the boundary
    /// "iffy" opens a condition and "during" matches "durable".
    #[test]
    fn a_conditional_needs_a_word_boundary() {
        assert_eq!(classify_default("iffy code is bad").score, 0);
        assert!(classify_default("if the file is `.rs`, skip it").score < 0);
    }

    /// V10: every row carries the signals that fired.
    #[test]
    fn every_verdict_names_the_signals_that_produced_it() {
        let verdict = classify_default("never commit to `main`");
        assert!(verdict.names().contains(&"never".to_string()));
        assert!(verdict.names().contains(&"exact-handle".to_string()));
    }

    #[test]
    fn classification_is_case_insensitive() {
        assert_eq!(
            label("NEVER commit to `main`"),
            label("never commit to `main`")
        );
    }

    #[test]
    fn classification_is_deterministic() {
        let text = "when writing tests, prefer table-driven cases";
        assert_eq!(classify_default(text), classify_default(text));
    }

    /// Sharpness predicts fire rate, so the 3 rows are where `--dead` comes
    /// from. This pins the ladder ordering that claim rests on.
    #[test]
    fn both_ladders_run_sharp_to_fuzzy() {
        let sharp =
            classify_default("when editing `.tf` files, prefer modules")
                .sharpness;
        let fuzzy =
            classify_default("if the user seems stuck, consider options")
                .sharpness;
        assert!(sharp < fuzzy, "{sharp:?} should be sharper than {fuzzy:?}");
    }
    /// A directive with no judgment word, no threshold and no exact handle
    /// lands mid-ladder rather than at either end: a runner is plausible
    /// but nothing in the text says what it would test.
    #[test]
    fn a_directive_with_no_handle_and_no_threshold_is_m2() {
        assert_eq!(label("never do that thing"), "M2");
    }

    /// A bare conditional with no directive and no hedge is still a
    /// TRIGGER, so it is situational rather than unknown.
    #[test]
    fn a_bare_conditional_is_situational() {
        let verdict = classify_default("when the build goes red");
        assert_eq!(verdict.class, Class::S);
        assert_eq!(verdict.sharpness, Some(3));
    }

    #[test]
    fn a_bare_conditional_naming_an_exact_thing_is_s1() {
        assert_eq!(label("when `cargo test` runs"), "S1");
    }

    #[test]
    fn a_bare_conditional_naming_a_domain_is_s2() {
        assert_eq!(label("before each commit here"), "S2");
    }

    /// V40 + V64. The FORM is what the caller reads off the raw text
    /// and the heading context. A list item is always a list item. A
    /// paragraph under a heading is promoted (V64); one before any
    /// heading stays a paragraph (V40).
    #[test]
    fn the_form_follows_the_marker_and_heading() {
        let none: Option<String> = None;
        let some = Some("Applying X".to_string());
        assert_eq!(Form::from_context(true, &none), Form::ListItem);
        assert_eq!(Form::from_context(true, &some), Form::ListItem);
        assert_eq!(Form::from_context(false, &none), Form::Paragraph);
        assert_eq!(Form::from_context(false, &some), Form::ListItem);
    }

    /// V40. A bare imperative carries no modal, and the corpus this crate
    /// points at itself states most of its rules that way.
    #[test]
    fn an_imperative_opener_is_mechanical() {
        assert_eq!(label("run `hk check` before pushing"), "M1");
    }

    /// V40. An absolute quantifier states a law without a modal.
    #[test]
    fn an_absolute_quantifier_is_mechanical() {
        assert_eq!(label("the coverage floor only ever rises"), "M2");
    }

    /// V40. A statement naming its own negative case is stating a rule --
    /// V4's logic, applied to the corpus rather than to a trigger.
    #[test]
    fn a_contrast_is_mechanical() {
        let verdict = classify_default(
            "a gate step that cannot run is a failure, not a pass",
        );
        assert_eq!(verdict.class, Class::M);
        assert!(verdict.names().iter().any(|name| name == "contrast"));
    }

    /// V40's load-bearing half, and the case that was MEASURED before the
    /// invariant was written: this exact paragraph opens with a verb and
    /// names its own negative case, and it is PROSE. Unscoped, the mood
    /// signals turn it into a rule; scoped to list items, it stays `U`.
    #[test]
    fn the_same_mood_in_a_paragraph_is_unknown() {
        let text = "read that as a warning about the file's future, not a claim about its present";
        assert_eq!(classify_default(text).class, Class::M);
        let verdict = classify(text, Form::Paragraph, &Weights::default());
        assert_eq!(verdict.class, Class::U);
        assert!(verdict.signals.is_empty(), "{:?}", verdict.signals);
    }

    /// Mood is HALF a directive, so one hedge outweighs it. A bare
    /// imperative is a rule because of where it sits; a hedged one says
    /// out loud that it is not.
    #[test]
    fn a_hedge_beats_a_bare_imperative() {
        let verdict = classify_default("run the slow suite where possible");
        assert_eq!(verdict.class, Class::S);
    }

    /// A quantifier is a WORD. "no" inside "nobody" is not a claim about
    /// exceptions, and a substring match would fire on most English.
    #[test]
    fn a_quantifier_is_matched_as_a_whole_word() {
        let verdict = classify_default("nobody reads a small changelog anyway");
        assert_eq!(verdict.class, Class::U);
        assert!(verdict.signals.is_empty(), "{:?}", verdict.signals);
    }

    /// V40's REVERSAL, executable: a corpus that disagrees with a mood
    /// signal switches it off by weight, without a code change.
    #[test]
    fn a_mood_word_switched_off_returns_the_verdict_to_unknown() {
        let text = "the coverage floor only ever rises";
        assert_eq!(classify_default(text).class, Class::M);
        let mut tuned = Weights::default();
        tuned.weight.insert("only".to_string(), 0);
        assert_eq!(classify(text, Form::ListItem, &tuned).class, Class::U);
    }
}
