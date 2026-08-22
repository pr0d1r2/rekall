//! Token counts, DELEGATED to `itok` (V8).
//!
//! Not reimplemented, and not approximated. Section C fixes the standard:
//! counts are TOKENIZER-backed, so they are measured and deterministic --
//! never a model's guess and never a bytes/4 heuristic. Two estimators
//! disagreeing is the same defect as two rule sets, and the arithmetic
//! kind argues loudest for itself because both numbers look plausible.
//!
//! `itok` is a SIBLING, not a dependency of this crate: it arrives on
//! PATH, not through Cargo. So its absence is a SKIP that is NAMED in the
//! output rather than a failure (V26) -- `scan` has to work on a box that
//! has never heard of it, with the column empty and a line saying why.
//!
//! WHY A TEMP DIRECTORY. `itok estimate` counts FILES, and a statement is
//! a span inside one. Two shapes were measured, and the numbers carry the
//! box they were taken on because a timing without its conditions is an
//! assertion wearing a measurement's clothes:
//!
//!   MEASURED on an Apple M4, 10 core / 16GB, 200 statements:
//!   * one process per statement, text on `/dev/stdin` -- 57ms each, so
//!     the corpus costs 11 SECONDS on every scan. The cost is the BPE
//!     table load, paid once per spawn, not the counting.
//!   * every statement written to a private directory and counted in ONE
//!     call -- 74ms for all 200.
//!
//! That box is the FAST one. The development box this crate is built on
//! is roughly 5x slower, which puts the per-statement shape near 57
//! SECONDS for one scan -- past the point section C names, where a step
//! becomes one people learn to bypass. So the ratio is what decides this,
//! not the absolute figures: batching is not an optimisation here, it is
//! the difference between a verb someone runs and one they avoid. The
//! statements touch disk in a mode-0700 directory for the length of one
//! call and the directory is removed after, including on the error paths.
//! V15 forbids EGRESS -- a byte reaching a host -- and this reaches none:
//! no network, no LAN, no user-named host, and the bytes were already on
//! this disk in the corpus file they were read from.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The sibling that owns token accounting.
pub const TOOL: &str = "itok";

/// Why a count could not be taken.
///
/// Never fatal to the caller. A missing column is a smaller loss than a
/// verb that refuses to run, and V26 only asks that the skip be SAID.
#[derive(Debug, PartialEq, Eq)]
pub enum Fault {
    /// `itok` is not on PATH.
    Absent,
    /// It ran and failed, or produced something unreadable.
    Failed(String),
    /// The private directory could not be made or written.
    Unwritable(String),
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.said())
    }
}

impl Fault {
    fn said(&self) -> String {
        match self {
            Self::Absent => format!(
                "`{TOOL}` is not on PATH, so the tokens column is empty. \
                 Install it, or enter this repo's dev shell, which provides it"
            ),
            Self::Failed(said) => format!(
                "`{TOOL}` could not count this corpus, so the tokens column \
                 is empty: {said}"
            ),
            Self::Unwritable(said) => format!(
                "the tokens column is empty: a private directory for counting \
                 could not be written: {said}"
            ),
        }
    }
}

impl std::error::Error for Fault {}

/// What the sibling answered: a count per path it was given.
///
/// Named rather than spelled out at four call sites, which is what the
/// complexity limit was pointing at -- the pairing is a THING here, not
/// an incidental tuple.
pub type Counts = Vec<(String, u32)>;

/// One line of `itok estimate --format json`.
///
/// The other fields it emits -- `unit`, `estimated`, `method` -- are
/// deliberately not modelled. A sibling adding a column should not break
/// this parse, and the two this crate acts on are the two it names.
#[derive(serde::Deserialize)]
struct Counted {
    path: String,
    tokens: u32,
}

/// Read `itok`'s JSONL into path/count pairs.
///
/// A line that will not parse is DROPPED rather than fatal: the statement
/// it belonged to keeps an empty column, which is the same outcome as the
/// tool being absent and a smaller loss than discarding every count
/// because one line was unexpected.
#[must_use]
pub fn parse(stdout: &str) -> Counts {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Counted>(line).ok())
        .map(|row| (row.path, row.tokens))
        .collect()
}

/// Count every text, in order.
///
/// The result is index-aligned with the input, and an entry is `None`
/// when that one statement had no line in the output. Aligned by FILENAME
/// rather than by output order, because relying on a sibling to emit its
/// answers in the order the paths were given is depending on something it
/// never promised.
pub fn count_all(texts: &[String]) -> Result<Vec<Option<u32>>, Fault> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let held = Scratch::make()?;
    let paths = held.spill(texts)?;
    let counted = run(&paths)?;
    Ok(align(texts.len(), &counted))
}

fn align(count: usize, counted: &[(String, u32)]) -> Vec<Option<u32>> {
    (0..count)
        .map(|at| {
            counted
                .iter()
                .find(|(path, _)| ends_with_name(path, at))
                .map(|(_, tokens)| *tokens)
        })
        .collect()
}

fn ends_with_name(path: &str, at: usize) -> bool {
    Path::new(path)
        .file_name()
        .is_some_and(|name| name == name_of(at).as_str())
}

fn name_of(at: usize) -> String {
    format!("{at}.txt")
}

/// A private directory that removes itself.
///
/// `Drop` rather than a cleanup call at the end: the counting path has
/// three fallible steps after the directory exists, and a cleanup someone
/// has to remember to reach is one an early `?` walks straight past --
/// leaving the user's private prose on disk after a failure.
struct Scratch {
    at: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.at);
    }
}

/// Distinguishes concurrent counts WITHIN one process.
///
/// The pid alone is not enough, and the test suite proved it: many scans
/// run at once in a single test binary, so a pid-named directory had them
/// deleting each other's files mid-count. The symptom was a count that
/// went missing under load and never when run alone -- the worst kind.
static NEXT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

impl Scratch {
    /// Named by PID AND a per-call counter, so no two counts -- in this
    /// process or any other -- can share a directory. Removed first in
    /// case a previous run died before its `Drop` could run, because
    /// reusing a stale directory would count whatever it left behind.
    fn make() -> Result<Self, Fault> {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let at = std::env::temp_dir()
            .join(format!("rekall-tokens-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).map_err(unwritable)?;
        restrict(&at, 0o700)?;
        Ok(Self { at })
    }

    fn spill(&self, texts: &[String]) -> Result<Vec<PathBuf>, Fault> {
        let mut out = Vec::new();
        for (at, text) in texts.iter().enumerate() {
            out.push(self.write_one(at, text)?);
        }
        Ok(out)
    }

    fn write_one(&self, at: usize, text: &str) -> Result<PathBuf, Fault> {
        let path = self.at.join(name_of(at));
        let mut file = std::fs::File::create(&path).map_err(unwritable)?;
        file.write_all(text.as_bytes()).map_err(unwritable)?;
        restrict(&path, 0o600)?;
        Ok(path)
    }
}

/// Mode bits BEFORE the bytes are readable by anyone else.
///
/// `create` honours the process umask, which on a shared box is commonly
/// 022 -- world-readable. The corpus is the user's private memory, so the
/// permissions are set rather than inherited.
fn restrict(path: &Path, mode: u32) -> Result<(), Fault> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(unwritable)
}

fn unwritable(error: std::io::Error) -> Fault {
    Fault::Unwritable(error.to_string())
}

/// ONE call, `--bpe`, so the count is o200k and not bytes/4.
///
/// `--bpe` is the whole point of delegating: without it `itok` answers
/// with the same heuristic section C rules out, and a heuristic printed
/// in a column a tokenizer will later fill is a number that looks
/// measured and is not.
fn run(paths: &[PathBuf]) -> Result<Counts, Fault> {
    let done = std::process::Command::new(TOOL)
        .args(["estimate", "--bpe", "--format", "json"])
        .args(paths)
        .output()
        .map_err(spawn_fault)?;
    if !done.status.success() {
        return Err(Fault::Failed(
            String::from_utf8_lossy(&done.stderr).trim().to_string(),
        ));
    }
    Ok(parse(&String::from_utf8_lossy(&done.stdout)))
}

/// A spawn that fails because the binary is not there is ABSENT, which is
/// a legal state with its own message. Any other spawn failure is a real
/// error and says what the OS said.
fn spawn_fault(error: std::io::Error) -> Fault {
    if error.kind() == std::io::ErrorKind::NotFound {
        return Fault::Absent;
    }
    Fault::Failed(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_json_line_yields_its_path_and_count() {
        let line = r#"{"path":"a.md","tokens":7,"unit":"input_tokens","estimated":false,"method":"o200k"}"#;
        assert_eq!(parse(line), vec![("a.md".to_string(), 7)]);
    }

    #[test]
    fn every_line_is_read() {
        let out = "{\"path\":\"0.txt\",\"tokens\":7}\n{\"path\":\"1.txt\",\"tokens\":3}\n";
        assert_eq!(parse(out).len(), 2);
    }

    /// A sibling adding a field must not break this parse. Only the two
    /// columns this crate acts on are modelled.
    #[test]
    fn unknown_fields_do_not_break_the_parse() {
        let line =
            r#"{"path":"a.md","tokens":7,"a_field_from_next_year":true}"#;
        assert_eq!(parse(line).len(), 1);
    }

    /// One unreadable line costs ONE column, not every count.
    #[test]
    fn a_line_that_will_not_parse_is_dropped_not_fatal() {
        let out = "not json\n{\"path\":\"1.txt\",\"tokens\":3}\n";
        assert_eq!(parse(out), vec![("1.txt".to_string(), 3)]);
    }

    #[test]
    fn nothing_to_count_asks_nothing() {
        assert_eq!(count_all(&[]), Ok(Vec::new()));
    }

    /// Aligned by FILENAME, not by output order. Depending on a sibling to
    /// answer in the order it was asked is depending on a promise it never
    /// made -- and the failure would be silent, pairing each statement
    /// with its neighbour's count.
    #[test]
    fn counts_align_by_name_even_when_the_output_is_shuffled() {
        let counted = vec![
            ("/tmp/x/1.txt".to_string(), 30),
            ("/tmp/x/0.txt".to_string(), 10),
        ];
        assert_eq!(align(2, &counted), vec![Some(10), Some(30)]);
    }

    #[test]
    fn a_statement_with_no_line_of_its_own_keeps_an_empty_column() {
        let counted = vec![("/tmp/x/0.txt".to_string(), 10)];
        assert_eq!(align(2, &counted), vec![Some(10), None]);
    }

    /// V26: the skip is NAMED, and the message says how to get the column
    /// back rather than only that it is missing (V28).
    #[test]
    fn an_absent_sibling_names_itself_and_the_fix() {
        let said = Fault::Absent.to_string();
        assert!(said.contains(TOOL), "{said}");
        assert!(said.contains("dev shell"), "{said}");
    }

    #[test]
    fn every_fault_says_the_column_is_empty() {
        for fault in [
            Fault::Absent,
            Fault::Failed("boom".to_string()),
            Fault::Unwritable("no space".to_string()),
        ] {
            assert!(fault.to_string().contains("empty"), "{fault:?}");
        }
    }

    /// A spawn that fails because nothing is there is ABSENT; anything
    /// else is a real error that reports what the OS said.
    #[test]
    fn a_missing_binary_is_absent_and_other_errors_are_not() {
        let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert_eq!(spawn_fault(missing), Fault::Absent);
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(matches!(spawn_fault(denied), Fault::Failed(_)));
    }

    /// THE END TO END, against the real sibling.
    ///
    /// Asserted UNCONDITIONALLY, with no "skip if absent" guard. The dev
    /// shell puts `itok` on PATH and the gate already depends on it, so a
    /// guard here would be a branch that never runs on any box that runs
    /// this suite -- and on a box where it did run, it would turn a real
    /// failure into a silent pass, which is the thing V26 exists to stop.
    /// The absent case is a legal state for the VERB, not for this test.
    #[test]
    fn real_statements_get_real_counts() {
        let texts = vec![
            "- never commit to `main`".to_string(),
            "- when editing Rust, run clippy first".to_string(),
        ];
        let counts = count_all(&texts);
        assert_eq!(counts.as_ref().map(Vec::len), Ok(2), "{counts:?}");
        assert_eq!(
            counts.as_ref().map(|got| got.iter().all(is_a_real_count)),
            Ok(true),
            "a count was missing or zero: {counts:?}"
        );
    }

    fn is_a_real_count(count: &Option<u32>) -> bool {
        count.is_some_and(|n| n > 0)
    }

    /// The sibling ran and refused. Reported as a NAMED failure carrying
    /// what it said, rather than as an absence -- "not installed" and
    /// "installed and unhappy" send the reader to different places.
    #[test]
    fn a_sibling_that_exits_nonzero_is_a_named_failure() {
        let missing = vec![PathBuf::from("/definitely/not/here.txt")];
        let got = run(&missing);
        assert!(
            matches!(&got, Err(Fault::Failed(said)) if said.contains("here.txt")),
            "expected a Failed fault naming the path, got {got:?}"
        );
    }

    #[test]
    fn an_io_error_becomes_an_unwritable_fault() {
        let fault = unwritable(std::io::Error::other("disk full"));
        assert_eq!(fault, Fault::Unwritable("disk full".to_string()));
    }

    /// The private directory does not outlive the call, on ANY path. The
    /// corpus is the user's private prose and it has no business staying
    /// on disk after the count is taken.
    #[test]
    fn the_scratch_directory_is_gone_afterwards() {
        let at = Scratch::make().ok().map(|held| {
            assert!(held.at.is_dir(), "the directory was never made");
            held.at.clone()
        });
        assert_eq!(
            at.as_ref().map(|path| path.exists()),
            Some(false),
            "private prose was left at {at:?}"
        );
    }

    /// TWO COUNTS AT ONCE must not share a directory. A pid-named one had
    /// concurrent scans deleting each other's files mid-count, which showed
    /// up as a column that went missing under load and never when run
    /// alone. Asserted here rather than left to the suite to rediscover.
    #[test]
    fn concurrent_scratch_directories_are_distinct() {
        let (first, second) = (Scratch::make(), Scratch::make());
        let held = |made: &Result<Scratch, Fault>| {
            made.as_ref().ok().map(|one| one.at.clone())
        };
        let (one, two) = (held(&first), held(&second));
        assert!(one.is_some(), "no directory was made");
        assert_ne!(one, two, "two counts shared a directory");
        assert_eq!(one.map(|at| at.is_dir()), Some(true));
    }

    /// The same property, exercised the way it actually broke: two counts
    /// running at the same time in one process.
    #[test]
    fn two_counts_at_once_do_not_disturb_each_other() {
        let done = count_in_parallel(4);
        let first = done.first().and_then(|one| one.as_ref().ok());
        assert!(first.is_some(), "no parallel count finished: {done:?}");
        for other in &done {
            assert_eq!(other.as_ref().ok(), first, "a count differed");
        }
    }

    type Counted = Result<Vec<Option<u32>>, Fault>;

    fn count_in_parallel(jobs: usize) -> Vec<Counted> {
        let texts = vec!["- never commit to `main`".to_string()];
        std::thread::scope(|scope| {
            let running: Vec<_> = (0..jobs)
                .map(|_| scope.spawn(|| count_all(&texts)))
                .collect();
            running
                .into_iter()
                .filter_map(|job| job.join().ok())
                .collect()
        })
    }

    /// Mode 0700, not whatever the umask happens to be. A world-readable
    /// directory of extracted memory is the kind of default nobody sets on
    /// purpose and everybody inherits.
    #[test]
    fn the_scratch_directory_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let made = Scratch::make();
        let mode = made
            .as_ref()
            .ok()
            .and_then(|held| std::fs::metadata(&held.at).ok())
            .map(|meta| meta.permissions().mode() & 0o777);
        assert_eq!(mode, Some(0o700));
    }
}
