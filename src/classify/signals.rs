//! The SIGNAL vocabulary and what each word is worth.
//!
//! Split from the classifier itself because they answer different
//! questions: this file says what EVIDENCE a statement carries, and
//! `classify` says what that evidence adds up to. The words are here, the
//! arithmetic is there, and the module-size gate is what noticed they had
//! grown into one file.

use super::Signal;

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

/// HALF a directive, the same rung a CONDITIONAL sits on below zero (V40).
///
/// Mood is weaker evidence than a modal: "never" states a rule outright,
/// while an imperative or an absolute is a rule only because of WHERE it
/// is written. Half means ONE hedge still beats a bare imperative, which
/// is the verdict a reader would give "- Run the slow suite where
/// possible".
const MOOD_WEIGHT: i32 = 1;

/// Was the statement written as a LIST ITEM or as a PARAGRAPH?
///
/// The form is EVIDENCE (V40). A corpus states its rules as bullets and
/// its context as paragraphs, so the same words argue differently in the
/// two places -- and the caller has to supply this, because `normalize`
/// drops the marker before the classifier ever sees the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    ListItem,
    Paragraph,
}

impl Form {
    #[must_use]
    pub fn from_list_item(is_list_item: bool) -> Self {
        if is_list_item {
            Self::ListItem
        } else {
            Self::Paragraph
        }
    }

    fn is_list_item(self) -> bool {
        self == Self::ListItem
    }
}

/// One family's words at one weight.
///
/// A CONDITIONAL is written with its trailing space ("when ") so the
/// vocabulary reads as the opener it is; the KEY drops it, because a
/// config naming `when ` with a space nobody can see would be a table
/// entry that silently never matches.
fn family(
    weight: &mut std::collections::BTreeMap<String, i32>,
    words: &[&str],
    value: i32,
) {
    for word in words {
        weight.insert(word.trim_end().to_string(), value);
    }
}

impl Default for Weights {
    fn default() -> Self {
        let mut weight = std::collections::BTreeMap::new();
        family(&mut weight, &DIRECTIVE, DIRECTIVE_WEIGHT);
        family(&mut weight, &SOFT, SOFT_WEIGHT);
        family(&mut weight, &CONDITIONAL, CONDITIONAL_WEIGHT);
        family(&mut weight, &IMPERATIVE, MOOD_WEIGHT);
        family(&mut weight, &ABSOLUTE, MOOD_WEIGHT);
        family(&mut weight, &[CONTRAST], MOOD_WEIGHT);
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
    /// MOOD words are matched only inside a LIST ITEM (V40).
    pub(super) fn fired(&self, lower: &str, form: Form) -> Vec<Signal> {
        let mut out: Vec<Signal> = Vec::new();
        for (name, weight) in &self.weight {
            if !hits(lower, name, form) {
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

/// How a word has to appear before it counts.
///
/// The mode belongs to the WORD, not to the caller: "if" is an opener
/// wherever it is configured, and "only" argues for a rule only where a
/// rule is being written. A config that tunes a word tunes its weight and
/// inherits its mode, because a weight is a judgment about evidence and
/// the mode is what the evidence IS.
enum Mode {
    Anywhere,
    Start,
    ImperativeStart,
    ListWord,
    ListContrast,
}

fn mode_of(name: &str) -> Mode {
    if CONDITIONAL.iter().any(|word| word.trim_end() == name) {
        return Mode::Start;
    }
    if IMPERATIVE.contains(&name) {
        return Mode::ImperativeStart;
    }
    if ABSOLUTE.contains(&name) {
        return Mode::ListWord;
    }
    if name == CONTRAST {
        return Mode::ListContrast;
    }
    Mode::Anywhere
}

fn hits(lower: &str, name: &str, form: Form) -> bool {
    match mode_of(name) {
        Mode::Anywhere => lower.contains(name),
        Mode::Start => starts_with_word(lower.trim_start(), name),
        Mode::ImperativeStart => {
            form.is_list_item() && starts_with_word(lower.trim_start(), name)
        }
        Mode::ListWord => form.is_list_item() && contains_word(lower, name),
        Mode::ListContrast => {
            form.is_list_item() && lower.contains(CONTRAST_TEXT)
        }
    }
}

/// A whole word, not a substring.
///
/// "no" is inside "nobody", "not" and "know"; "all" is inside "small".
/// A quantifier matched as a substring would fire on most sentences in
/// English and the signal would mean nothing.
fn contains_word(lower: &str, name: &str) -> bool {
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word == name)
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

/// An IMPERATIVE opener: a bare verb where the subject would be.
///
/// A CLOSED list, because the alternative is a part-of-speech tagger and
/// V5 forbids the model that would carry one. English gives no other
/// mechanical handle: "commit" opens an instruction in "- Commit straight
/// to `main`" and names a thing in "- Commit messages are short", and
/// nothing local to the text separates them. That ambiguity is why the
/// weight is HALF a directive rather than equal to one, and why the list
/// stays short enough to argue with.
const IMPERATIVE: [&str; 25] = [
    "add", "avoid", "check", "cite", "commit", "decide", "delete", "document",
    "extract", "fix", "keep", "make", "measure", "name", "put", "read",
    "record", "remove", "run", "set", "split", "treat", "update", "use",
    "write",
];

/// An ABSOLUTE quantifier: a statement admitting no exception.
///
/// "The coverage floor only ever rises" carries no modal and is a law all
/// the same. Matched as a WHOLE word (`contains_word`), never a substring.
const ABSOLUTE: [&str; 7] =
    ["all", "any", "every", "no", "none", "nothing", "only"];

/// A CONTRAST: a statement that names its own negative case.
///
/// "A gate step that cannot run is a failure, not a pass" states what it
/// excludes, which is V4's logic one level out -- absence stated rather
/// than left to be inferred. The signal is NAMED rather than keyed by the
/// literal, because a `scan` row joins signal names with commas and a
/// name containing one would split a column.
const CONTRAST: &str = "contrast";
const CONTRAST_TEXT: &str = ", not ";

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
