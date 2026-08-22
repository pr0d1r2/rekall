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

/// What each signal is worth, and how close to a tie counts as "I do not
/// know".
///
/// A TABLE rather than a branch, because V30 makes class a BALANCE. The
/// point of the table is not to change verdicts on arrival -- it is to let
/// a corpus TUNE them without a code change. A weight that shipped with
/// new behaviour baked in would be two changes wearing one commit.
///
/// The defaults reproduce every verdict the decision tree gave that a test
/// covered, and ONE it did not: a hedged CONDITIONAL rule was `U` under
/// the tree, because a conflict won outright, and is `S` under the sum.
/// That case is pinned by a test of its own rather than left to be found
/// later -- "exactly" would have been the easier claim and the false one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Weights {
    pub deadband: i32,
    pub weight: std::collections::BTreeMap<String, i32>,
}

/// Positive pulls MECHANICAL, negative pulls SITUATIONAL.
///
/// The magnitudes are the whole model, so they are stated rather than
/// tuned: a DIRECTIVE and a HEDGE are equal and opposite, which is what
/// makes "always prefer X" sum to zero and land on `U` -- V10's "I do not
/// know" falling out of the arithmetic instead of being a special case. A
/// CONDITIONAL is HALF a hedge: it leans situational on its own, and loses
/// to a directive, because "when editing `.rs`, never unwrap" is a lint.
const DIRECTIVE_WEIGHT: i32 = 2;
const SOFT_WEIGHT: i32 = -2;
const CONDITIONAL_WEIGHT: i32 = -1;

impl Default for Weights {
    fn default() -> Self {
        let mut weight = std::collections::BTreeMap::new();
        for word in DIRECTIVE {
            weight.insert(word.to_string(), DIRECTIVE_WEIGHT);
        }
        for word in SOFT {
            weight.insert(word.to_string(), SOFT_WEIGHT);
        }
        for word in CONDITIONAL {
            weight.insert(word.trim_end().to_string(), CONDITIONAL_WEIGHT);
        }
        Self {
            deadband: 0,
            weight,
        }
    }
}

impl Weights {
    /// The shipped defaults, with a config table laid OVER them per word.
    ///
    /// Over, not instead of: a config naming one word should tune that
    /// word, not silently discard the vocabulary the crate ships with.
    /// Setting a weight to 0 is how a word is switched OFF, which is a
    /// statement someone can read in their own config.
    #[must_use]
    pub fn from_config(
        deadband: Option<i32>,
        weight: &std::collections::BTreeMap<String, i32>,
    ) -> Self {
        let mut out = Self::default();
        out.deadband = deadband.unwrap_or(out.deadband);
        for (name, value) in weight {
            out.weight.insert(name.clone(), *value);
        }
        out
    }

    /// Every configured word this text contains, with what it is worth.
    ///
    /// CONDITIONALS are matched at the START only, the rest anywhere --
    /// "if" inside a sentence is not an opener, and treating it as one
    /// made half the corpus situational when the tree was first written.
    fn fired(&self, lower: &str) -> Vec<Signal> {
        let mut out: Vec<Signal> = Vec::new();
        for (name, weight) in &self.weight {
            if !hits(lower, name) {
                continue;
            }
            out.push(Signal {
                name: name.clone(),
                weight: *weight,
            });
        }
        out
    }
}

fn hits(lower: &str, name: &str) -> bool {
    if CONDITIONAL.iter().any(|word| word.trim_end() == name) {
        return starts_with_word(lower.trim_start(), name);
    }
    lower.contains(name)
}

/// A conditional opener is a WORD at the start, not a prefix. Without the
/// boundary, "iff" and "durability" would open a condition.
fn starts_with_word(lower: &str, name: &str) -> bool {
    lower
        .strip_prefix(name)
        .is_some_and(|rest| rest.starts_with(|c: char| !c.is_alphanumeric()))
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
pub fn classify(text: &str, weights: &Weights) -> Verdict {
    let lower = text.to_lowercase();
    let signals = weights.fired(&lower);
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

/// Classify with the shipped defaults. The convenience the tests and every
/// caller without a config file use.
#[must_use]
pub fn classify_default(text: &str) -> Verdict {
    classify(text, &Weights::default())
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
    /// code change. Here a word the shipped defaults never heard of turns
    /// a `U` into an `M` -- which is the answer to the nine unclassified
    /// statements this repo's own CLAUDE.md produced.
    #[test]
    fn a_configured_word_classifies_what_the_defaults_could_not() {
        let text = "commit straight to `main`";
        assert_eq!(classify_default(text).class, Class::U);
        let mut tuned = Weights::default();
        tuned.weight.insert("commit straight to".to_string(), 2);
        assert_eq!(classify(text, &tuned).class, Class::M);
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
        assert_eq!(classify(text, &cautious).class, Class::U);
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
        assert_eq!(label("before every commit here"), "S2");
    }
}
