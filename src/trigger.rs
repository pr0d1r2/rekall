//! The trigger: what makes a skill load, and what stops it.
//!
//! V29 makes this a BLOCK rather than prose. A skill file carries a fenced
//! `rekall` block under each of its two headings, and the block IS the
//! trigger -- the prose beside it is for the human and is never read. The
//! alternative was matching English, and V5 forbids the model that would
//! be needed to do it honestly.
//!
//! TOML, because the config already speaks it (V8, and section C's one
//! format / one parser). Three keys, no more: `tool` names exact tools,
//! `path` holds globs, `word` holds literals tested against the situation
//! text. Within a key ANY value matches; across keys ALL PRESENT keys must
//! match. A key that is absent constrains nothing.
//!
//! EXCLUSION WINS. A `## Does NOT fire when` match refuses the load even
//! when the fire block matched, because V4 makes absence a CLAUSE and a
//! clause that loses to a positive match states nothing at all.
//!
//! An `S3` carries an EMPTY block and therefore never fires. That is not a
//! gap to be closed later: its trigger is semantic, V5 forbids the model
//! that would notice it, and V10 already calls a `3` the spec saying out
//! loud that this artifact may never fire. V11's `--dead` is what measures
//! it afterwards.

use crate::corpus;

/// The fence tag that marks a trigger block.
///
/// A tag rather than "the first fence under the heading": a skill file is
/// a document a human edits, and an example shell snippet under `## Fires
/// when` should not become the trigger by accident.
pub const FENCE: &str = "```rekall";

/// One block's worth of conditions.
///
/// Absent keys are EMPTY, and an empty key constrains nothing. That makes
/// the wholly empty block -- every key absent -- the one that matches
/// nothing at all rather than everything, which is the `S3` case and the
/// freshly generated case both.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    /// Exact tool names. `S1`'s sharpest handle.
    #[serde(default)]
    pub tool: Vec<String>,
    /// Globs against the path in play. Also `S1`.
    #[serde(default)]
    pub path: Vec<String>,
    /// Literals tested against the situation text. `S2`: a matchable
    /// CLASS of situations rather than an exact one.
    #[serde(default)]
    pub word: Vec<String>,
}

impl Trigger {
    /// Nothing to match on. Legal, and the whole of `S3`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tool.is_empty() && self.path.is_empty() && self.word.is_empty()
    }
}

/// What the caller knows about right now.
#[derive(Debug, Default, Clone)]
pub struct Situation {
    pub tool: Option<String>,
    pub path: Option<String>,
    /// Where the agent is working. Section I gives `recall` a `--cwd`
    /// separate from every other verb's `-C`: one says where the PROJECT
    /// is, this says where the WORK is, and only the second is a fact a
    /// trigger can turn on.
    pub cwd: Option<String>,
    /// The free text a human typed, or the harness's description.
    pub text: String,
}

impl Situation {
    /// THE path in play: the file when one is named, else the directory
    /// the work is happening in.
    ///
    /// One value rather than matching both, because "either may match"
    /// makes an EXCLUSION fire on the directory while the file it names is
    /// somewhere else entirely -- a refusal nobody could predict from
    /// reading it. The named file is the more specific fact, so it wins.
    fn in_play(&self) -> Option<&String> {
        self.path.as_ref().or(self.cwd.as_ref())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// The heading exists but carries no fenced `rekall` block.
    Missing,
    /// A block that will not parse, or names a key this crate does not
    /// have. V29 REFUSES it rather than reading what it can: a trigger
    /// half-understood is a skill that loads in cases nobody chose.
    Unreadable(String),
    /// A glob that will not compile.
    BadGlob(String),
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(
                f,
                "no {FENCE} block under the heading -- the block IS the trigger \
                 and prose beside it is not read (V29)"
            ),
            Self::Unreadable(said) => write!(
                f,
                "the {FENCE} block does not parse: {said}. Keys are `tool`, \
                 `path` and `word`, each a list of strings"
            ),
            Self::BadGlob(said) => {
                write!(f, "a `path` glob does not compile: {said}")
            }
        }
    }
}

impl std::error::Error for Fault {}

/// Pull the fenced block out from under a heading.
///
/// Scoped to the heading's own section, so the `## Does NOT fire when`
/// block is never read as the fire block when the first one is missing --
/// which would turn an exclusion into a trigger, the worst possible
/// direction for this mistake to go.
pub fn parse_block(text: &str, heading: &str) -> Result<Trigger, Fault> {
    let body = section(text, heading).ok_or(Fault::Missing)?;
    let raw = fenced(&body).ok_or(Fault::Missing)?;
    let held: Trigger = toml::from_str(&raw)
        .map_err(|error| Fault::Unreadable(error.message().to_string()))?;
    check_globs(&held)?;
    Ok(held)
}

/// Globs are compiled AT PARSE TIME, not at match time.
///
/// A trigger whose glob is malformed is drift `check` should report, not a
/// surprise the matcher hits in production while a skill quietly fails to
/// load.
fn check_globs(held: &Trigger) -> Result<(), Fault> {
    corpus::matcher(&held.path)
        .map(|_| ())
        .map_err(|error| Fault::BadGlob(error.to_string()))
}

/// The lines under a heading, up to the next one at the same level.
fn section<'a>(text: &'a str, heading: &str) -> Option<Vec<&'a str>> {
    let mut lines = text.lines().skip_while(|line| line.trim() != heading);
    lines.next()?;
    Some(
        lines
            .take_while(|line| !line.trim_start().starts_with("## "))
            .collect(),
    )
}

/// The body of the first `rekall` fence in these lines.
fn fenced(body: &[&str]) -> Option<String> {
    let start = body.iter().position(|line| line.trim() == FENCE)?;
    let rest = body.get(start.checked_add(1)?..)?;
    let end = rest.iter().position(|line| line.trim() == "```")?;
    Some(rest.get(..end)?.join("\n"))
}

/// Whether a skill loads HERE.
///
/// The order is the invariant: a refusal is checked FIRST and wins
/// outright (V29). Checking it second -- or treating it as a tie-break --
/// would let a positive match override the clause V4 exists to require,
/// and a clause that can be overridden states nothing.
#[must_use]
pub fn fires(fire: &Trigger, refuse: &Trigger, at: &Situation) -> bool {
    if matches(refuse, at) {
        return false;
    }
    matches(fire, at)
}

/// ALL present keys must match; an absent key constrains nothing.
///
/// The empty block therefore matches NOTHING rather than everything. That
/// direction is deliberate: a trigger nobody filled in should load a skill
/// never, not always -- always-on prose is exactly what the skill was
/// extracted from (V3).
#[must_use]
pub fn matches(held: &Trigger, at: &Situation) -> bool {
    if held.is_empty() {
        return false;
    }
    tool_ok(held, at) && path_ok(held, at) && word_ok(held, at)
}

fn tool_ok(held: &Trigger, at: &Situation) -> bool {
    if held.tool.is_empty() {
        return true;
    }
    at.tool
        .as_ref()
        .is_some_and(|name| held.tool.iter().any(|want| want == name))
}

/// Globs, matched against the path in play. A `path` key with no path to
/// test is a NON-match: the condition was stated and nothing satisfied it.
fn path_ok(held: &Trigger, at: &Situation) -> bool {
    if held.path.is_empty() {
        return true;
    }
    let Ok(set) = corpus::matcher(&held.path) else {
        return false;
    };
    at.in_play()
        .is_some_and(|path| set.is_match(std::path::Path::new(path)))
}

/// Literals against the situation text, folded to lowercase so a trigger
/// does not turn on how the caller happened to capitalise.
fn word_ok(held: &Trigger, at: &Situation) -> bool {
    if held.word.is_empty() {
        return true;
    }
    let text = at.text.to_lowercase();
    held.word
        .iter()
        .any(|want| text.contains(&want.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRES: &str = "## Fires when";

    fn skill(block: &str) -> String {
        format!("# s\n\nprose\n\n{FIRES}\n\n{FENCE}\n{block}\n```\n")
    }

    fn at(tool: &str, path: &str, text: &str) -> Situation {
        Situation {
            tool: Some(tool.to_string()),
            path: Some(path.to_string()),
            cwd: None,
            text: text.to_string(),
        }
    }

    fn in_dir(cwd: &str) -> Situation {
        Situation {
            cwd: Some(cwd.to_string()),
            ..Situation::default()
        }
    }

    fn only(block: &str) -> Trigger {
        parse_block(&skill(block), FIRES).unwrap_or_default()
    }

    #[test]
    fn the_three_keys_parse() {
        let held = only(
            "tool = [\"Edit\"]\npath = [\"**/*.rs\"]\nword = [\"clippy\"]",
        );
        assert_eq!(held.tool, vec!["Edit".to_string()]);
        assert_eq!(held.path, vec!["**/*.rs".to_string()]);
        assert_eq!(held.word, vec!["clippy".to_string()]);
    }

    /// A key this crate does not have is REFUSED, not ignored. A trigger
    /// half-understood loads a skill in cases nobody chose, and a typo'd
    /// key that silently does nothing is the quietest way to get one.
    #[test]
    fn an_unknown_key_is_refused() {
        let got = parse_block(&skill("tolo = [\"Edit\"]"), FIRES);
        assert!(matches!(got, Err(Fault::Unreadable(_))), "{got:?}");
    }

    #[test]
    fn a_block_that_is_not_toml_is_refused() {
        let got = parse_block(&skill("tool = [unquoted"), FIRES);
        assert!(matches!(got, Err(Fault::Unreadable(_))), "{got:?}");
    }

    /// Globs compile at PARSE time so a broken one is drift `check`
    /// reports, not a surprise the matcher hits in production.
    #[test]
    fn a_glob_that_will_not_compile_is_refused_at_parse() {
        let got = parse_block(&skill("path = [\"[\"]"), FIRES);
        assert!(matches!(got, Err(Fault::BadGlob(_))), "{got:?}");
    }

    #[test]
    fn a_heading_with_no_block_is_missing_and_says_why() {
        let text = format!("# s\n\n{FIRES}\n\nTODO: name the tool.\n");
        let got = parse_block(&text, FIRES);
        assert_eq!(got, Err(Fault::Missing));
        let said = got.err().map(|f| f.to_string()).unwrap_or_default();
        assert!(said.contains("prose beside it is not read"), "{said}");
    }

    /// THE DIRECTION THAT MATTERS. With no fire block, the parse must not
    /// fall through to the exclusion block -- that would turn a refusal
    /// into a trigger, which is the worst way for this to be wrong.
    #[test]
    fn a_missing_fire_block_never_reads_the_exclusion_block() {
        let text = format!(
            "# s\n\n{FIRES}\n\nTODO\n\n## Does NOT fire when\n\n{FENCE}\ntool = [\"Edit\"]\n```\n"
        );
        assert_eq!(parse_block(&text, FIRES), Err(Fault::Missing));
    }

    /// An untagged fence is not a trigger. A skill is a document, and a
    /// shell example under the heading should not become the rule.
    #[test]
    fn an_untagged_fence_is_not_a_trigger_block() {
        let text = format!("# s\n\n{FIRES}\n\n```sh\ntool = [\"Edit\"]\n```\n");
        assert_eq!(parse_block(&text, FIRES), Err(Fault::Missing));
    }

    #[test]
    fn an_empty_block_parses_and_matches_nothing() {
        let held = only("");
        assert!(held.is_empty());
        assert!(!matches(&held, &at("Edit", "src/a.rs", "anything")));
    }

    /// Within a key, ANY value matches.
    #[test]
    fn any_value_in_a_key_matches() {
        let held = only("tool = [\"Edit\", \"Write\"]");
        assert!(matches(&held, &at("Write", "", "")));
        assert!(!matches(&held, &at("Bash", "", "")));
    }

    /// Across keys, ALL PRESENT keys must match.
    #[test]
    fn every_present_key_must_match() {
        let held = only("tool = [\"Edit\"]\npath = [\"**/*.rs\"]");
        assert!(matches(&held, &at("Edit", "src/main.rs", "")));
        assert!(
            !matches(&held, &at("Edit", "README.md", "")),
            "the path key was ignored"
        );
        assert!(
            !matches(&held, &at("Bash", "src/main.rs", "")),
            "the tool key was ignored"
        );
    }

    #[test]
    fn an_absent_key_constrains_nothing() {
        let held = only("word = [\"clippy\"]");
        assert!(matches(
            &held,
            &at("anything", "any/path", "run clippy now")
        ));
    }

    #[test]
    fn words_are_matched_without_regard_to_case() {
        let held = only("word = [\"Clippy\"]");
        assert!(matches(&held, &at("", "", "please run CLIPPY")));
    }

    /// A stated key with nothing to test it against is a NON-match. The
    /// condition was named and nothing satisfied it.
    #[test]
    fn a_key_with_nothing_to_test_does_not_match() {
        let held = only("path = [\"**/*.rs\"]");
        let nothing = Situation::default();
        assert!(!matches(&held, &nothing));
    }

    /// V29's teeth. An exclusion refuses the load even when the fire block
    /// matched -- V4 makes absence a CLAUSE, and a clause that loses to a
    /// positive match states nothing.
    #[test]
    fn exclusion_wins_over_a_positive_match() {
        let fire = only("path = [\"**/*.rs\"]");
        let refuse = only("path = [\"**/tests/**\"]");
        assert!(fires(&fire, &refuse, &at("", "src/main.rs", "")));
        assert!(
            !fires(&fire, &refuse, &at("", "src/tests/big.rs", "")),
            "a positive match overrode the refusal"
        );
    }

    #[test]
    fn an_empty_exclusion_refuses_nothing() {
        let fire = only("tool = [\"Edit\"]");
        assert!(fires(&fire, &Trigger::default(), &at("Edit", "", "")));
    }

    /// With no file named, `path` tests the directory the work is in --
    /// which is the only path in play when someone asks "what loads here"
    /// before touching anything.
    #[test]
    fn the_cwd_stands_in_when_no_file_is_named() {
        let held = only("path = [\"**/backend/**\"]");
        assert!(matches(&held, &in_dir("srv/backend/api")));
        assert!(!matches(&held, &in_dir("srv/web")));
    }

    /// The NAMED FILE WINS. Matching both would let an exclusion fire on
    /// the directory while the file it names sits elsewhere -- a refusal
    /// nobody could predict from reading the block.
    #[test]
    fn a_named_file_beats_the_directory_it_was_named_from() {
        let held = only("path = [\"**/backend/**\"]");
        let mut at = at("Edit", "srv/web/app.rs", "");
        at.cwd = Some("srv/backend/api".to_string());
        assert!(!matches(&held, &at));
    }

    /// The `S3` case, stated as a test because it is a CONSEQUENCE rather
    /// than a rule anyone wrote: an empty trigger fires for no situation
    /// that exists, so an S3 skill never loads (V29).
    #[test]
    fn an_s3_trigger_fires_for_nothing() {
        let empty = Trigger::default();
        for situation in [
            at("Edit", "src/a.rs", "refactor this"),
            at("", "", ""),
            at("Bash", "Makefile", "deploy"),
        ] {
            assert!(!fires(&empty, &Trigger::default(), &situation));
        }
    }
}
