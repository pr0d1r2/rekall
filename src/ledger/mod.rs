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

/// The fire JOURNAL: one id per line, appended, never rewritten in place.
///
/// V34 requires a fire count that survives CONCURRENT hooks, and one hook
/// runs per tool call. A read-modify-write of `ledger.toml` would have two
/// hooks read the same number, both add one, and one increment vanish --
/// silently, and only under load.
///
/// Appending sidesteps it: `O_APPEND` puts a short write at the end
/// atomically, so two hooks cannot overwrite each other. This is NOT a
/// second store in V8's sense -- it is the SAME number with a
/// write-optimised tail, folded back on every read, the shape a
/// write-ahead log has.
pub const FIRES: &str = "fires";

/// One extraction, with everything `revert` needs.
#[derive(
    Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
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
    /// The class the extraction was made under -- `M1`, `S2` and so on.
    ///
    /// RECORDED rather than re-derived. `check` needs it to know which
    /// obligation applies (a runner for `M`, a trigger for `S`), and the
    /// artifact's PATH would answer the same question by inference about a
    /// decision that was already made and printed. V10 says a class is a
    /// CLAIM; a claim that leaves no record cannot be argued with later.
    #[serde(default)]
    pub label: String,
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
    /// Where the artifact was ISSUED to, empty until it is.
    ///
    /// The row's STAGE, and the only place it is recorded. Empty means the
    /// extraction lives here and nowhere else. Set, with the artifact still
    /// present, means the handover is open and both copies stand
    /// (`src/issue:V54`). Set, with the artifact gone, means RETIRED -- the
    /// rule lives one repo out, and `check` must not call that an orphan.
    ///
    /// A String and not an enum, because it also has to say WHERE: a reader
    /// asking which registry adopted a rule is asking the question this
    /// column exists to answer, and a three-state enum beside a path column
    /// would be two fields that can disagree.
    /// WRITTEN ONLY WHEN SET. An empty column on every row is noise a
    /// reader has to skip past, and this file is read by people as often
    /// as by this crate.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub issued_to: String,
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
    let mut held = read_toml(path)?;
    fold(&mut held, &tally(&fires_beside(path)));
    Ok(held)
}

fn read_toml(path: &Path) -> Result<Ledger, Error> {
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

#[must_use]
pub fn fires_beside(path: &Path) -> PathBuf {
    path.parent().unwrap_or(Path::new(".")).join(FIRES)
}

/// Count one firing (V34). APPENDS -- see `FIRES`.
///
/// A line naming an id nothing holds is harmless: `fold` ignores it, so a
/// revert racing a hook loses a count rather than corrupting a row.
pub fn record_fire(path: &Path, id: &str) -> Result<(), Error> {
    use std::io::Write;
    ensure_dir(path)?;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|cause| Error::Write {
            path: path.to_path_buf(),
            cause,
        })?;
    // ONE `write_all`, never `writeln!`. `write_fmt` may issue a syscall
    // per fragment -- the id, then the newline -- and only a SINGLE write
    // is atomic under `O_APPEND`. Split across two, concurrent hooks
    // interleave mid-line and the ids merge into each other. MEASURED
    // before this line existed: 8 parallel fires tallied as 3.
    file.write_all(format!("{id}\n").as_bytes())
        .map_err(|cause| Error::Write {
            path: path.to_path_buf(),
            cause,
        })
}

fn ensure_dir(path: &Path) -> Result<(), Error> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent).map_err(|cause| Error::Write {
        path: parent.to_path_buf(),
        cause,
    })
}

/// How many times each id appears in the journal. A missing journal is an
/// empty tally, not an error: nothing has fired yet is the normal state.
fn tally(path: &Path) -> std::collections::BTreeMap<String, u64> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(path) else {
        return out;
    };
    for id in text.lines().map(str::trim).filter(|id| !id.is_empty()) {
        *out.entry(id.to_string()).or_insert(0) =
            out.get(id).copied().unwrap_or(0).saturating_add(1);
    }
    out
}

fn fold(held: &mut Ledger, tally: &std::collections::BTreeMap<String, u64>) {
    for row in &mut held.extracted {
        let extra = tally.get(&row.id).copied().unwrap_or(0);
        row.fires = row.fires.saturating_add(extra);
    }
}

/// Write the ledger, creating `.rekall/` if needed.
///
/// This is also the JOURNAL's compaction: the counts being written already
/// include everything the journal held (`load` folded them in), so the
/// journal is cleared or the next read would count those fires twice.
///
/// A hook appending between the fold and this truncation loses ONE count.
/// That window is a few milliseconds inside `apply`/`revert`, which are
/// interactive and rare; the alternative -- read-modify-write on every
/// tool call -- loses counts continuously and only under load, which is
/// the failure V34 actually names.
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
    })?;
    let _ = std::fs::remove_file(fires_beside(path));
    Ok(())
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
    /// Record WHERE a row was issued.
    ///
    /// Set rather than appended: re-issuing to a second registry replaces
    /// the destination, because a row that named two would be describing
    /// a rule with two owners and no answer to which one tends it.
    pub fn issue(&mut self, id: &str, to: &str) -> bool {
        let Some(row) = self.extracted.iter_mut().find(|row| row.id == id)
        else {
            return false;
        };
        row.issued_to = to.to_string();
        true
    }

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
            label: "M1".to_string(),
            artifact: ".claude/rules/no-main.sh".to_string(),
            fires: 0,
            at: 1_700_000_000,
            issued_to: String::new(),
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

    /// A ledger written before `issued_to` existed still loads, and every
    /// row in it reads as NOT ISSUED.
    ///
    /// This is the whole reason the column defaults rather than being
    /// required: every ledger in the field was written without it, and a
    /// store that refuses to parse after an upgrade takes `revert` -- the
    /// one verb that has to work on old rows -- down with it.
    #[test]
    fn a_row_written_before_the_column_existed_reads_as_not_issued() {
        let base = dir("pre-issue");
        let path = path_in(&base);
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(&base));
        let _ = std::fs::write(
            &path,
            "[[extracted]]\nid = \"abc1234\"\nsrc = \"CLAUDE.md\"\n\
             line_start = 1\nline_end = 2\ntext = \"x\"\n\
             artifact = \".rekall/rules/x.sh\"\n",
        );
        let held = load(&path).ok().unwrap_or_default();
        assert_eq!(
            held.extracted.first().map(|row| row.issued_to.clone()),
            Some(String::new())
        );
    }

    /// A row nobody issued writes NO column, so adding the field did not
    /// rewrite every ledger in the fleet the first time anything saved
    /// one. MEASURED: it did, before this attribute -- an empty
    /// `issued_to = ""` appeared on all eleven rows of this repo's own
    /// ledger, in a commit that was supposed to touch one artifact.
    #[test]
    fn a_row_that_was_never_issued_writes_no_column() {
        let base = dir("no-column");
        let path = path_in(&base);
        let mut ledger = Ledger::default();
        assert!(ledger.record(row("abc1234")));
        assert!(save(&path, &ledger).is_ok());
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(!text.contains("issued_to"), "{text}");
    }

    /// And a row that WAS issued keeps it across a save.
    #[test]
    fn an_issued_row_keeps_its_destination() {
        let base = dir("column");
        let path = path_in(&base);
        let mut ledger = Ledger::default();
        assert!(ledger.record(row("abc1234")));
        assert!(ledger.issue("abc1234", "../set-and-setting"));
        assert!(save(&path, &ledger).is_ok());
        assert_eq!(
            load(&path)
                .ok()
                .and_then(|held| held.find("abc1234").cloned())
                .map(|row| row.issued_to),
            Some("../set-and-setting".to_string())
        );
    }

    /// Issuing an id the ledger does not hold changes nothing and says
    /// so, rather than inventing a row for it.
    #[test]
    fn issuing_an_unknown_id_is_false() {
        let mut ledger = Ledger::default();
        assert!(!ledger.issue("nope", "../elsewhere"));
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

    /// A saved ledger holding one row, ready to fire against.
    fn one_row(name: &str) -> PathBuf {
        let path = dir(name).join(FILE);
        let mut held = Ledger::default();
        held.record(row("aaa"));
        let _ = save(&path, &held);
        path
    }

    /// The JOURNAL, folded back on read. This is the whole of V34's
    /// mechanism seen from outside: append, then `load` reports the sum.
    #[test]
    fn a_recorded_fire_is_folded_into_the_row_on_load() {
        let path = one_row("journal");
        let fires = fires_beside(&path);
        let _ = record_fire(&fires, "aaa");
        let _ = record_fire(&fires, "aaa");
        assert_eq!(
            load(&path)
                .ok()
                .and_then(|l| l.extracted.first().map(|r| r.fires)),
            Some(2)
        );
    }

    /// SAVE COMPACTS. The counts written already include the journal, so
    /// leaving it behind would count every fire twice on the next read.
    #[test]
    fn saving_clears_the_journal_it_folded() {
        let path = one_row("compact");
        let _ = record_fire(&fires_beside(&path), "aaa");
        let folded = load(&path).unwrap_or_default();
        let _ = save(&path, &folded);
        assert!(!fires_beside(&path).exists(), "the journal survived");
        assert_eq!(
            load(&path)
                .ok()
                .and_then(|l| l.extracted.first().map(|r| r.fires)),
            Some(1),
            "the fire was counted twice"
        );
    }

    /// A journal line naming an id nothing holds is IGNORED, not fatal --
    /// a revert racing a hook loses a count rather than corrupting a row.
    #[test]
    fn a_fire_for_an_unknown_id_is_ignored() {
        let path = one_row("orphan-fire");
        let _ = record_fire(&fires_beside(&path), "gone");
        assert_eq!(
            load(&path)
                .ok()
                .and_then(|l| l.extracted.first().map(|r| r.fires)),
            Some(0)
        );
    }

    #[test]
    fn a_fire_that_cannot_be_written_names_the_path() {
        let at = dir("stuck-journal");
        let blocked = at.join("wall");
        let _ = std::fs::write(&blocked, "not a directory");
        let said = record_fire(&blocked.join(FIRES), "aaa")
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(said.contains("cannot write"), "{said}");
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
    /// V28, on the ledger's own errors: each one NAMES the file it could
    /// not touch. A message that says only "write failed" sends someone
    /// looking through four paths for the one that was denied.
    #[test]
    fn every_ledger_error_names_what_it_could_not_do() {
        let read = Error::Read {
            path: PathBuf::from(".rekall/ledger.toml"),
            cause: std::io::Error::other("denied"),
        };
        assert!(read.to_string().contains(".rekall/ledger.toml"), "{read}");
        let encode = toml::to_string_pretty(&f64::NAN)
            .err()
            .map(Error::Encode)
            .map(|one| one.to_string())
            .unwrap_or_default();
        assert!(encode.contains("cannot encode the ledger"), "{encode}");
    }

    /// The fire counter writes to a path the caller chose, and a path that
    /// cannot be opened is a REPORTED failure rather than a lost count --
    /// V11 needs the number, so silence here is the bug that hides itself.
    #[test]
    fn a_fire_counter_that_cannot_be_opened_says_which_path() {
        let at = dir("fire-unwritable");
        let blocked = at.join("ledger.fires");
        let _ = std::fs::create_dir_all(&blocked);
        let failed = record_fire(&blocked, "abc1234");
        assert!(failed.is_err(), "a directory is not an appendable file");
        let said = failed.err().map(|one| one.to_string()).unwrap_or_default();
        assert!(said.contains("ledger.fires"), "{said}");
    }

    /// A path with no parent needs no directory made for it. The root is
    /// the case that has none, and it must not be an error.
    #[test]
    fn a_path_with_no_parent_needs_no_directory() {
        assert!(ensure_dir(Path::new("/")).is_ok());
    }

    /// A ledger that cannot be written is an error and not a silent
    /// no-op: every mutating verb saves through here, and a lost row is
    /// an extraction with no way back (V9).
    #[test]
    fn a_ledger_that_cannot_be_written_says_so() {
        let dir = PathBuf::from("target").join("ledger-unwritable");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("ledger.toml");
        let _ = std::fs::create_dir_all(&path);
        assert!(matches!(
            save(&path, &Ledger::default()),
            Err(Error::Write { .. })
        ));
    }
}
