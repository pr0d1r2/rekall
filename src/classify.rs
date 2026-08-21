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

/// A class and, unless `U`, how sharp a runner or trigger the statement
/// admits. `U` carries no sharpness: absence of a class has no ladder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub class: Class,
    pub sharpness: Option<u8>,
    pub signals: Vec<&'static str>,
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

    fn unknown(signals: Vec<&'static str>) -> Self {
        Self {
            class: Class::U,
            sharpness: None,
            signals,
        }
    }
}

/// An absolute directive: the action is stated as a rule rather than a
/// preference.
const DIRECTIVE: [&str; 8] = [
    "never", "always", "must not", "must", "do not", "don't", "ensure",
    "require",
];

/// A conditional opener. Marks a statement as having a TRIGGER, which is
/// what a skill needs and what decides sharpness rather than class.
const CONDITIONAL: [&str; 7] = [
    "when ",
    "whenever ",
    "if ",
    "before ",
    "after ",
    "while ",
    "during ",
];

/// Softeners. A statement hedged this way is not a rule a runner can
/// enforce, whatever else it contains.
const SOFT: [&str; 8] = [
    "prefer",
    "consider",
    "try to",
    "generally",
    "usually",
    "tends to",
    "where possible",
    "as needed",
];

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

fn starts_with_any(
    haystack: &str,
    needles: &[&'static str],
) -> Option<&'static str> {
    needles
        .iter()
        .find(|needle| haystack.starts_with(*needle))
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
    signals: &mut Vec<&'static str>,
) -> u8 {
    if let Some(word) = contains_any(lower, &JUDGMENT) {
        signals.push(word);
        return 3;
    }
    if has_number(lower) {
        signals.push("threshold");
        return 2;
    }
    if has_exact_handle(raw, lower) {
        signals.push("exact-handle");
        return 1;
    }
    2
}

/// Sharpness for a situational statement: how sharp a TRIGGER it admits.
fn situational_sharpness(
    raw: &str,
    lower: &str,
    signals: &mut Vec<&'static str>,
) -> u8 {
    if has_exact_handle(raw, lower) {
        signals.push("exact-handle");
        return 1;
    }
    if let Some(noun) = contains_any(lower, &DOMAIN) {
        signals.push(noun);
        return 2;
    }
    3
}

/// Classify one statement.
///
/// The CLASS turns on whether the ACTION is enforceable: an absolute
/// directive is mechanical, a hedged one is situational. A conditional
/// opener does NOT decide the class -- "when editing `.rs`, never unwrap"
/// is still a lint. It decides SHARPNESS, because a trigger is what a skill
/// needs and how exact that trigger is is the whole ladder.
#[must_use]
pub fn classify(text: &str) -> Verdict {
    let lower = text.to_lowercase();
    let mut signals: Vec<&'static str> = Vec::new();
    let directive = contains_any(&lower, &DIRECTIVE);
    let soft = contains_any(&lower, &SOFT);
    if let Some(word) = starts_with_any(lower.trim_start(), &CONDITIONAL) {
        signals.push(word.trim_end());
    }
    decide(text, &lower, Fired { directive, soft }, signals)
}

struct Fired {
    directive: Option<&'static str>,
    soft: Option<&'static str>,
}

fn decide(
    raw: &str,
    lower: &str,
    fired: Fired,
    signals: Vec<&'static str>,
) -> Verdict {
    match (fired.directive, fired.soft) {
        (Some(directive), Some(soft)) => conflict(directive, soft, signals),
        (Some(directive), None) => mechanical(raw, lower, directive, signals),
        (None, Some(soft)) => situational(raw, lower, soft, signals),
        (None, None) => situational_or_unknown(raw, lower, signals),
    }
}

/// Both a rule and a hedge -- "always prefer X". That is a genuine
/// ambiguity in the PROSE, so it is REPORTED as one rather than resolved by
/// whichever word list happened to be checked first. Resolving it would
/// hand back a confident verdict the text does not support, which is what
/// V10 means by a classifier that never says "I do not know".
fn conflict(
    directive: &'static str,
    soft: &'static str,
    mut signals: Vec<&'static str>,
) -> Verdict {
    signals.push(directive);
    signals.push(soft);
    signals.push("conflict");
    Verdict::unknown(signals)
}

fn mechanical(
    raw: &str,
    lower: &str,
    directive: &'static str,
    mut signals: Vec<&'static str>,
) -> Verdict {
    signals.push(directive);
    let sharpness = mechanical_sharpness(raw, lower, &mut signals);
    Verdict {
        class: Class::M,
        sharpness: Some(sharpness),
        signals,
    }
}

fn situational(
    raw: &str,
    lower: &str,
    soft: &'static str,
    mut signals: Vec<&'static str>,
) -> Verdict {
    signals.push(soft);
    let sharpness = situational_sharpness(raw, lower, &mut signals);
    Verdict {
        class: Class::S,
        sharpness: Some(sharpness),
        signals,
    }
}

/// No directive and no hedge. A bare conditional is still a trigger, so it
/// is situational; anything else is genuinely unclassified.
fn situational_or_unknown(
    raw: &str,
    lower: &str,
    mut signals: Vec<&'static str>,
) -> Verdict {
    if signals.is_empty() {
        return Verdict::unknown(signals);
    }
    let sharpness = situational_sharpness(raw, lower, &mut signals);
    Verdict {
        class: Class::S,
        sharpness: Some(sharpness),
        signals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(text: &str) -> String {
        classify(text).label()
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
        let verdict = classify("when editing `.rs`, never use unwrap");
        assert_eq!(verdict.class, Class::M);
    }

    #[test]
    fn prose_with_no_signal_is_unknown() {
        let verdict = classify("this project started in 2024");
        assert_eq!(verdict.class, Class::U);
        assert_eq!(verdict.sharpness, None);
    }

    /// V10: U is legal and is the DEFAULT.
    #[test]
    fn unknown_carries_no_sharpness() {
        assert_eq!(label("some background prose"), "U");
    }

    /// A rule and a hedge in one sentence is an ambiguity in the PROSE.
    /// Reporting it beats resolving it by list order.
    #[test]
    fn a_directive_that_is_also_hedged_is_unknown_and_says_why() {
        let verdict = classify("always prefer the simpler option");
        assert_eq!(verdict.class, Class::U);
        assert!(
            verdict.signals.contains(&"conflict"),
            "{:?}",
            verdict.signals
        );
    }

    /// V10: every row carries the signals that fired.
    #[test]
    fn every_verdict_names_the_signals_that_produced_it() {
        let verdict = classify("never commit to `main`");
        assert!(verdict.signals.contains(&"never"));
        assert!(verdict.signals.contains(&"exact-handle"));
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
        assert_eq!(classify(text), classify(text));
    }

    /// Sharpness predicts fire rate, so the 3 rows are where `--dead` comes
    /// from. This pins the ladder ordering that claim rests on.
    #[test]
    fn both_ladders_run_sharp_to_fuzzy() {
        let sharp =
            classify("when editing `.tf` files, prefer modules").sharpness;
        let fuzzy =
            classify("if the user seems stuck, consider options").sharpness;
        assert!(sharp < fuzzy, "{sharp:?} should be sharper than {fuzzy:?}");
    }
}
