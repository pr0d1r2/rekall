//! The ledger: what was extracted, and what has been proposed.
//!
//! TWO ROW KINDS. `EXTRACTED` rows are the record V9 needs to make `revert`
//! mechanical -- source path, line span and the ORIGINAL TEXT, so putting a
//! statement back is replay rather than a rewrite. `CANDIDATE` rows are
//! what `catch` mines from a transcript: a transcript is ephemeral, so a
//! violation seen at turn 200 is gone tomorrow unless the candidate
//! outlives the session that produced it.
//!
//! It is a TRACKED file, not a cache. `revert` has to work in a fresh
//! clone, and an artifact whose original text lives only on the machine
//! that extracted it is not reversible in any sense that matters.

use std::path::{Path, PathBuf};

/// Where the ledger lives, relative to the project root.
///
/// A directory rather than another root-level dotfile: plans (`--out`) and
/// anything else this tool needs to keep will land beside it instead of
/// growing the sprawl R6 records.
pub const DIR: &str = ".rekall";
pub const FILE: &str = "ledger.toml";

/// One extraction, with everything `revert` needs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Extracted {
    pub id: String,
    /// The STABLE name -- project-relative or `~`-prefixed, never absolute,
    /// so a ledger committed on one machine resolves on another.
    pub src: String,
    pub line_start: usize,
    pub line_end: usize,
    /// VERBATIM. This is the half that makes V9 true: without it `revert`
    /// would be regenerating prose from a rule, which is a rewrite.
    pub text: String,
    pub artifact: String,
    /// How many times the artifact fired. V11 makes a never-fired artifact
    /// DETECTABLE -- a rule that never fires is a wrong trigger or a dead
    /// law, and both need to be visible rather than inferred.
    #[serde(default)]
    pub fires: u64,
    /// Unix seconds. An integer rather than a formatted date, so `log
    /// --since` can compare without this crate learning calendars or
    /// taking a date dependency for one column.
    #[serde(default)]
    pub at: u64,
}

/// A statement `catch` proposed. Report-only for the corpus; promotion goes
/// through `plan` then `apply`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Candidate {
    pub id: String,
    pub src: String,
    pub text: String,
    #[serde(default)]
    pub at: u64,
}

#[derive(
    Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct Ledger {
    #[serde(default)]
    pub extracted: Vec<Extracted>,
    #[serde(default)]
    pub candidates: Vec<Candidate>,
}

#[derive(Debug)]
pub enum Error {
    Read {
        path: PathBuf,
        cause: std::io::Error,
    },
    Parse {
        path: PathBuf,
        cause: toml::de::Error,
    },
    Encode(toml::ser::Error),
    Write {
        path: PathBuf,
        cause: std::io::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, cause } => {
                write!(f, "cannot read {}: {cause}", path.display())
            }
            Self::Parse { path, cause } => {
                write!(f, "cannot parse {}: {cause}", path.display())
            }
            Self::Encode(cause) => {
                write!(f, "cannot encode the ledger: {cause}")
            }
            Self::Write { path, cause } => {
                write!(f, "cannot write {}: {cause}", path.display())
            }
        }
    }
}

impl std::error::Error for Error {}

#[must_use]
pub fn path_in(base: &Path) -> PathBuf {
    base.join(DIR).join(FILE)
}

/// Read the ledger.
///
/// A MISSING file is an EMPTY ledger, not an error: nothing has been
/// extracted yet is a normal state, and the first `apply` should not have
/// to be preceded by a `touch`. A file that exists and will not parse IS an
/// error -- treating it as empty would report every extraction as absent
/// and invite `apply` to redo work that was already done.
pub fn load(path: &Path) -> Result<Ledger, Error> {
    if !path.is_file() {
        return Ok(Ledger::default());
    }
    let text = std::fs::read_to_string(path).map_err(|cause| Error::Read {
        path: path.to_path_buf(),
        cause,
    })?;
    toml::from_str(&text).map_err(|cause| Error::Parse {
        path: path.to_path_buf(),
        cause,
    })
}

/// Write the ledger, creating `.rekall/` if needed.
pub fn save(path: &Path, ledger: &Ledger) -> Result<(), Error> {
    let text = toml::to_string_pretty(ledger).map_err(Error::Encode)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|cause| Error::Write {
            path: parent.to_path_buf(),
            cause,
        })?;
    }
    std::fs::write(path, text).map_err(|cause| Error::Write {
        path: path.to_path_buf(),
        cause,
    })
}

impl Ledger {
    /// The extracted row for an id PREFIX, if exactly one matches.
    ///
    /// Prefixes here for the same reason as `show`: the id a user has is
    /// the one they copied from a report.
    #[must_use]
    pub fn find(&self, prefix: &str) -> Option<&Extracted> {
        match self.matching(prefix).as_slice() {
            [only] => Some(only),
            _ => None,
        }
    }

    /// EVERY extracted row an id prefix matches.
    ///
    /// `find` collapses none and many to the same `None`, which is right
    /// for a caller that only wants the unique hit -- but a caller that
    /// REPORTS the failure needs to tell them apart. Section I makes an
    /// ambiguous prefix a usage error with its own message, not a coin
    /// toss, and "unknown id" is the wrong thing to print at someone whose
    /// id was merely too short.
    #[must_use]
    pub fn matching(&self, prefix: &str) -> Vec<&Extracted> {
        self.extracted
            .iter()
            .filter(|row| row.id.starts_with(prefix))
            .collect()
    }

    /// Whether this id has already been extracted. V13 makes `apply` of an
    /// already-extracted id a NO-OP rather than an error or a second row.
    #[must_use]
    pub fn holds(&self, id: &str) -> bool {
        self.extracted.iter().any(|row| row.id == id)
    }

    /// Record an extraction. Returns whether anything changed, so a caller
    /// can report "already done" without inspecting first.
    pub fn record(&mut self, row: Extracted) -> bool {
        if self.holds(&row.id) {
            return false;
        }
        self.extracted.push(row);
        true
    }

    /// Remove an extraction, returning the row so `revert` can replay it.
    pub fn take(&mut self, id: &str) -> Option<Extracted> {
        let at = self.extracted.iter().position(|row| row.id == id)?;
        Some(self.extracted.remove(at))
    }

    /// Count one firing (V11).
    pub fn fired(&mut self, id: &str) -> bool {
        let Some(row) = self.extracted.iter_mut().find(|row| row.id == id)
        else {
            return false;
        };
        row.fires = row.fires.saturating_add(1);
        true
    }

    /// Artifacts that have never fired. A rule that never fires is a wrong
    /// trigger or a dead law; both need to be VISIBLE rather than inferred.
    #[must_use]
    pub fn dead(&self) -> Vec<&Extracted> {
        self.extracted.iter().filter(|row| row.fires == 0).collect()
    }

    /// Propose a candidate. Duplicate ids are ignored, so re-running
    /// `catch` over the same transcript does not grow the file.
    pub fn propose(&mut self, row: Candidate) -> bool {
        if self.candidates.iter().any(|held| held.id == row.id) {
            return false;
        }
        self.candidates.push(row);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let path = PathBuf::from("target").join("ledger").join(name);
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::create_dir_all(&path);
        path
    }

    fn row(id: &str) -> Extracted {
        Extracted {
            id: id.to_string(),
            src: "CLAUDE.md".to_string(),
            line_start: 3,
            line_end: 3,
            text: "- never commit to `main`".to_string(),
            artifact: ".claude/rules/no-main.sh".to_string(),
            fires: 0,
            at: 1_700_000_000,
        }
    }

    /// Nothing extracted yet is a NORMAL state. The first `apply` must not
    /// have to be preceded by creating a file.
    #[test]
    fn a_missing_ledger_is_empty_not_an_error() {
        let base = dir("absent");
        assert_eq!(load(&path_in(&base)).ok(), Some(Ledger::default()));
    }

    /// A ledger that will not parse is an ERROR. Treating it as empty would
    /// report every extraction as absent and invite `apply` to redo work
    /// that was already done -- against a corpus it has already edited.
    #[test]
    fn a_corrupt_ledger_is_an_error_not_an_empty_one() {
        let base = dir("corrupt");
        let path = path_in(&base);
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&base));
        let _ = std::fs::write(&path, "this = = not toml");
        assert!(load(&path).is_err());
    }

    #[test]
    fn a_saved_ledger_reloads_identically() {
        let base = dir("roundtrip");
        let mut ledger = Ledger::default();
        assert!(ledger.record(row("abc1234")));
        let path = path_in(&base);
        assert!(save(&path, &ledger).is_ok());
        assert_eq!(load(&path).ok(), Some(ledger));
    }

    /// The original text must survive the round trip verbatim -- it is the
    /// half that makes V9's `revert` a replay rather than a rewrite.
    #[test]
    fn the_original_text_survives_the_round_trip() {
        let base = dir("verbatim");
        let mut ledger = Ledger::default();
        let mut original = row("abc1234");
        original.text = "- never  commit\n  to `main`".to_string();
        ledger.record(original.clone());
        let path = path_in(&base);
        let _ = save(&path, &ledger);
        let text = load(&path).ok().and_then(|held| held.find("abc").cloned());
        assert_eq!(text.map(|held| held.text), Some(original.text));
    }

    #[test]
    fn saving_creates_the_directory() {
        let base = dir("mkdir");
        let path = path_in(&base);
        assert!(save(&path, &Ledger::default()).is_ok());
        assert!(path.is_file());
    }

    #[test]
    fn a_write_to_an_impossible_path_names_it() {
        let path = Path::new("/definitely/not/writable/ledger.toml");
        let message = save(path, &Ledger::default())
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(message.contains("cannot write"), "message was {message}");
    }

    /// V13: `apply` of an already-extracted id is a NO-OP, not a second row.
    #[test]
    fn recording_the_same_id_twice_changes_nothing() {
        let mut ledger = Ledger::default();
        assert!(ledger.record(row("abc1234")));
        assert!(!ledger.record(row("abc1234")));
        assert_eq!(ledger.extracted.len(), 1);
    }

    #[test]
    fn find_accepts_a_prefix() {
        let mut ledger = Ledger::default();
        ledger.record(row("abc1234"));
        assert_eq!(
            ledger.find("abc").map(|held| held.id.clone()),
            Some("abc1234".to_string())
        );
    }

    /// An ambiguous prefix resolves to NOTHING rather than to whichever row
    /// happens to come first. The caller reports it; guessing would edit
    /// the wrong artifact.
    #[test]
    fn an_ambiguous_prefix_finds_nothing() {
        let mut ledger = Ledger::default();
        ledger.record(row("abc1234"));
        ledger.record(row("abc9999"));
        assert!(ledger.find("abc").is_none());
        assert!(ledger.find("abc1").is_some());
    }

    #[test]
    fn take_removes_the_row_and_returns_it() {
        let mut ledger = Ledger::default();
        ledger.record(row("abc1234"));
        assert_eq!(
            ledger.take("abc1234").map(|held| held.id),
            Some("abc1234".to_string())
        );
        assert!(ledger.extracted.is_empty());
        assert!(ledger.take("abc1234").is_none());
    }

    #[test]
    fn firing_counts_and_reports_whether_it_landed() {
        let mut ledger = Ledger::default();
        ledger.record(row("abc1234"));
        assert!(ledger.fired("abc1234"));
        assert!(ledger.fired("abc1234"));
        assert!(!ledger.fired("nosuch"));
        assert_eq!(ledger.find("abc").map(|held| held.fires), Some(2));
    }

    /// V11: a never-fired artifact is DETECTABLE.
    #[test]
    fn dead_lists_only_artifacts_that_never_fired() {
        let mut ledger = Ledger::default();
        ledger.record(row("aaa1111"));
        ledger.record(row("bbb2222"));
        ledger.fired("aaa1111");
        let dead: Vec<String> =
            ledger.dead().iter().map(|held| held.id.clone()).collect();
        assert_eq!(dead, vec!["bbb2222".to_string()]);
    }

    #[test]
    fn a_candidate_is_proposed_once() {
        let mut ledger = Ledger::default();
        let candidate = Candidate {
            id: "ccc3333".to_string(),
            src: "session.jsonl".to_string(),
            text: "- never force push".to_string(),
            at: 1,
        };
        assert!(ledger.propose(candidate.clone()));
        assert!(!ledger.propose(candidate));
        assert_eq!(ledger.candidates.len(), 1);
    }

    /// The two row kinds are independent: a candidate is not an extraction.
    #[test]
    fn candidates_and_extractions_do_not_collide() {
        let mut ledger = Ledger::default();
        ledger.record(row("abc1234"));
        ledger.propose(Candidate {
            id: "abc1234".to_string(),
            src: "session.jsonl".to_string(),
            text: "x".to_string(),
            at: 1,
        });
        assert_eq!(ledger.extracted.len(), 1);
        assert_eq!(ledger.candidates.len(), 1);
    }

    #[test]
    fn a_read_error_names_the_path() {
        let base = dir("unreadable");
        let path = path_in(&base);
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&base));
        let _ = std::fs::write(&path, [0xff_u8, 0xfe]);
        let message = load(&path)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(message.contains("cannot read"), "message was {message}");
    }
    #[test]
    fn a_parse_error_names_the_path() {
        let base = dir("parse-message");
        let path = path_in(&base);
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&base));
        let _ = std::fs::write(&path, "this = = not toml");
        let message = load(&path)
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(message.contains("cannot parse"), "message was {message}");
        assert!(message.contains("ledger.toml"), "message was {message}");
    }

    /// The directory exists and is writable, but the ledger PATH is itself
    /// a directory -- so the create succeeds and the write is what fails.
    /// Two different failures reaching the same error, and only one of them
    /// was exercised before.
    #[test]
    fn a_write_over_a_directory_names_the_file() {
        let base = dir("write-over-dir");
        let path = path_in(&base);
        let _ = std::fs::create_dir_all(&path);
        let message = save(&path, &Ledger::default())
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(message.contains("cannot write"), "message was {message}");
        assert!(message.contains("ledger.toml"), "message was {message}");
    }
}
