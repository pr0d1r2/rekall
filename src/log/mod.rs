//! `rekall log` -- what was extracted, and whether it ever mattered.
//!
//! Reads the ledger and nothing else. The verb is `log`, the store is the
//! LEDGER, and it is called one everywhere it is described.
//!
//! `--dead` is the half that earns the fire counter. A rule that never
//! fires is a wrong trigger or a dead law, and both need to be VISIBLE
//! rather than inferred (V11). That is also the answer to the growth R13
//! measures: files grow +226% across their life and the deletion hazard
//! FALLS with age, because nobody dares remove a rule they cannot prove is
//! unused. `--dead` is that proof, so deletion stops being nerve and
//! becomes arithmetic.
//!
//! SHARPNESS predicts what this MEASURES, so the two are checkable against
//! each other: a `1` gone dead is a classifier defect, a `3` that fires
//! often is a ladder defect. Neither is visible without both numbers, so
//! the label travels with every row.

use crate::ledger;

/// One extraction, as the ledger remembers it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Entry {
    pub id: String,
    /// `file:line-line`, the form section I prints everywhere else.
    pub src: String,
    pub text: String,
    pub label: String,
    pub artifact: String,
    pub fires: u64,
    /// Tokens RECLAIMED, NET of the pointer left behind (V39).
    ///
    /// SIGNED, because it can be negative: an extraction leaves a pointer
    /// that is itself always-on, so the saving is (statement - pointer).
    /// This column reported GROSS until B3 -- two real extractions each
    /// made the corpus BIGGER while it claimed they had shrunk it.
    ///
    /// NULL rather than a guess when `itok` is absent (V26). A char/4
    /// stand-in printed in the column a tokenizer fills is a number that
    /// looks measured and is not.
    pub reclaimed: Option<i64>,
    /// Unix seconds, as recorded.
    pub at: u64,
}

#[derive(Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub entries: Vec<Entry>,
    /// Whether the fire counter has ever been WRITTEN.
    ///
    /// Carried in the report rather than inferred from a column of
    /// zeroes, because those two states look identical and mean opposite
    /// things: nothing has fired, versus nothing has been counting.
    pub instrumented: bool,
    /// How many rows `--dead` DECLINED to judge for want of a counter.
    ///
    /// It separates "nothing is dead" from "nothing was measured", which
    /// is the whole distinction B11 turned on -- and it is what lets an
    /// EMPTY ledger stay silent instead of carrying a warning about a
    /// counter no row needed.
    pub withheld: usize,
}

/// What to leave out.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Filter {
    /// V11: only artifacts that have NEVER fired.
    pub dead: bool,
    /// The oldest `at` to include, in unix seconds. Computed by the caller
    /// from a duration, so this module never learns what "a week" is.
    pub since: Option<u64>,
}

/// Turn a ledger into a report.
///
/// Ledger ORDER, not sorted by fires or date. The ledger is the record of
/// what happened in the order it happened, and re-sorting a log by its
/// most interesting column is how the thing that changed since yesterday
/// stops being where you left it.
#[must_use]
pub fn report(
    held: &ledger::Ledger,
    filter: Filter,
    instrumented: bool,
) -> Report {
    Report {
        entries: entries(held, filter, instrumented),
        instrumented,
        withheld: withheld(held, filter, instrumented),
    }
}

fn withheld(
    held: &ledger::Ledger,
    filter: Filter,
    instrumented: bool,
) -> usize {
    if filter.dead && !instrumented {
        return held.extracted.len();
    }
    0
}

/// The rows to show.
///
/// `--dead` over an UNINSTRUMENTED ledger names NOTHING (V55). Every row
/// reads `fires = 0` there, so the filter would return the whole ledger
/// and call it droppable -- and the caller would be told to delete rules
/// their gate runs on every commit. Silence plus the line that says why is
/// the only honest answer to a question nothing measured.
fn entries(
    held: &ledger::Ledger,
    filter: Filter,
    instrumented: bool,
) -> Vec<Entry> {
    if filter.dead && !instrumented {
        return Vec::new();
    }
    held.extracted
        .iter()
        .filter(|row| keep(row, filter))
        .map(entry)
        .collect()
}

fn keep(row: &ledger::Extracted, filter: Filter) -> bool {
    if filter.dead && row.fires != 0 {
        return false;
    }
    filter.since.is_none_or(|floor| row.at >= floor)
}

fn entry(row: &ledger::Extracted) -> Entry {
    Entry {
        id: row.id.clone(),
        src: format!("{}:{}-{}", row.src, row.line_start, row.line_end),
        text: row.text.clone(),
        label: row.label.clone(),
        artifact: row.artifact.clone(),
        fires: row.fires,
        reclaimed: None,
        at: row.at,
    }
}

/// How far back `--since` reaches, as a count of seconds.
///
/// A DURATION rather than a date, and the ledger's `at` column is why: it
/// holds unix seconds precisely so this crate never learns calendars or
/// takes a date dependency for one column. `7d` is unambiguous in every
/// locale; `08/09` is not.
///
/// The suffix is REQUIRED. `--since 7` could mean days or hours and
/// guessing would silently answer a different question than the one asked
/// -- so it is a usage error that names the forms that work (V28).
pub fn duration(raw: &str) -> Result<u64, String> {
    let (digits, unit) = raw.split_at(raw.len().saturating_sub(1));
    let scale = match unit {
        "h" => 3_600_u64,
        "d" => 86_400,
        "w" => 604_800,
        _ => return Err(bad_duration(raw)),
    };
    let count: u64 = digits.parse().map_err(|_| bad_duration(raw))?;
    Ok(count.saturating_mul(scale))
}

fn bad_duration(raw: &str) -> String {
    format!(
        "`--since {raw}` is not a duration. Use a number and a unit: `12h`, `7d`, `2w`"
    )
}

/// The cutoff a duration means, given the current time.
///
/// SATURATING at zero rather than wrapping: `--since 500w` on a machine
/// whose clock is unset should include everything, not nothing.
#[must_use]
pub fn floor(now: u64, ago: u64) -> u64 {
    now.saturating_sub(ago)
}

/// One row per artifact, then its ORIGINAL TEXT indented beneath.
///
/// Both halves, because section I asks for both and they answer different
/// questions: the row is how the artifact is doing, the text is what it
/// was. A log that truncated the statement to fit a column would be a log
/// you have to leave to understand.
/// What an uninstrumented ledger is told, verbatim.
///
/// It NAMES THE FIX (`.:V28`) and it names what was not measured rather
/// than what is not there. `fires` counts one delivery path of three: an
/// hk step running a runner ENFORCES a rule and an indexed head is LOADED
/// by the host, and neither writes this counter.
pub const UNMEASURED: &str = "\
unmeasured  the fire counter has never been written -- there is no `.rekall/fires` \
here, so nothing has been MEASURED as never firing and no row is named. \
`rekall hook` is what increments it; wire it (docs/INTEGRATION.md) and ask again. \
Note that an hk step running a rule's runner does NOT count: this column is \
DELIVERED-TO-AN-AGENT, not enforced.\n";

/// Is there anything for the note to be ABOUT?
///
/// An empty ledger needs no warning about a counter no row asked for, and
/// a warning printed over nothing is the banner people learn to skip past
/// -- and then skip past on the run where it mattered.
fn says_unmeasured(report: &Report) -> bool {
    !report.entries.is_empty() || report.withheld > 0
}

#[must_use]
pub fn render_human(report: &Report) -> String {
    let mut out = String::new();
    if !report.instrumented && says_unmeasured(report) {
        out.push_str(UNMEASURED);
    }
    for entry in &report.entries {
        out.push_str(&format!("{}\n", render_row(entry)));
        for line in entry.text.lines() {
            out.push_str(&format!("  {line}\n"));
        }
    }
    out
}

fn render_row(entry: &Entry) -> String {
    let net = entry
        .reclaimed
        .map_or_else(|| "-".to_string(), |count| format!("{count:+}"));
    format!(
        "{}  {}  {}  fires={}  net={}  {}",
        entry.id, entry.src, entry.label, entry.fires, net, entry.artifact
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, fires: u64, at: u64) -> ledger::Extracted {
        ledger::Extracted {
            id: id.to_string(),
            src: "CLAUDE.md".to_string(),
            line_start: 3,
            line_end: 4,
            text: "- never commit to `main`".to_string(),
            label: "M1".to_string(),
            artifact: ".rekall/rules/no-main.sh".to_string(),
            fires,
            at,
            issued_to: String::new(),
        }
    }

    fn held(rows: Vec<ledger::Extracted>) -> ledger::Ledger {
        ledger::Ledger {
            extracted: rows,
            candidates: Vec::new(),
        }
    }

    fn ids(report: &Report) -> Vec<String> {
        report.entries.iter().map(|e| e.id.clone()).collect()
    }

    /// Asserted through the `Option` rather than an unwrap or a fallback
    /// value. A fallback here would be a branch nothing can reach -- the
    /// entry always exists -- and unreachable code in a test file is the
    /// same defect as unreachable code anywhere else.
    #[test]
    fn every_extraction_is_listed_with_its_span_and_fire_count() {
        let out =
            report(&held(vec![row("aaa", 3, 100)]), Filter::default(), true);
        let first = out.entries.first();
        assert_eq!(
            first.map(|e| e.src.clone()),
            Some("CLAUDE.md:3-4".to_string())
        );
        assert_eq!(first.map(|e| e.fires), Some(3));
        assert_eq!(
            first.map(|e| e.artifact.clone()),
            Some(".rekall/rules/no-main.sh".to_string())
        );
    }

    /// V11: a never-fired artifact is DETECTABLE. That is what makes
    /// deletion arithmetic instead of nerve.
    /// B11: every row reads `fires = 0` when NOTHING has been counting, so
    /// the unfiltered `--dead` returned the whole ledger and called it
    /// droppable -- seven of those rows being runners this repo's gate
    /// executes on every commit. Unmeasured is not dead.
    #[test]
    fn dead_names_nothing_when_the_counter_was_never_written() {
        let ledger = held(vec![row("aaa", 0, 100), row("bbb", 0, 200)]);
        let filter = Filter {
            dead: true,
            ..Filter::default()
        };
        assert!(report(&ledger, filter, false).entries.is_empty());
    }

    /// And it SAYS why, rather than printing nothing at all. Silence over
    /// a question nobody measured reads as "no dead rules", which is the
    /// same lie in a quieter voice.
    #[test]
    fn an_uninstrumented_report_says_it_was_not_measured() {
        let ledger = held(vec![row("aaa", 0, 100)]);
        let said = render_human(&report(&ledger, Filter::default(), false));
        assert!(said.starts_with("unmeasured"), "{said}");
        assert!(said.contains("rekall hook"), "names the fix: {said}");
        assert!(said.contains("not enforced"), "names the scope: {said}");
    }

    /// An instrumented ledger says nothing extra. A banner on every run is
    /// a banner nobody reads.
    #[test]
    fn an_instrumented_report_is_silent_about_it() {
        let ledger = held(vec![row("aaa", 2, 100)]);
        let said = render_human(&report(&ledger, Filter::default(), true));
        assert!(!said.contains("unmeasured"), "{said}");
    }

    /// The plain log still lists every row when nothing has counted --
    /// only the CLAIM about deadness is withheld, not the ledger itself.
    #[test]
    fn the_plain_log_still_lists_rows_when_uninstrumented() {
        let ledger = held(vec![row("aaa", 0, 100)]);
        let out = report(&ledger, Filter::default(), false);
        assert_eq!(ids(&out), vec!["aaa"]);
        assert!(!out.instrumented);
    }

    #[test]
    fn dead_lists_only_what_never_fired() {
        let ledger = held(vec![row("aaa", 0, 0), row("bbb", 7, 0)]);
        let filter = Filter {
            dead: true,
            since: None,
        };
        assert_eq!(ids(&report(&ledger, filter, true)), vec!["aaa"]);
        assert_eq!(ids(&report(&ledger, Filter::default(), true)).len(), 2);
    }

    /// The LABEL travels with the row. V11 makes sharpness and fire count
    /// checkable against each other -- a `1` gone dead is a classifier
    /// defect, a `3` that fires often is a ladder defect -- and neither is
    /// visible without both numbers side by side.
    #[test]
    fn the_class_is_reported_beside_the_fire_count() {
        let mut fuzzy = row("aaa", 0, 0);
        fuzzy.label = "S3".to_string();
        let out = report(&held(vec![fuzzy]), Filter::default(), true);
        let text = render_human(&out);
        assert!(text.contains("S3"), "{text}");
        assert!(text.contains("fires=0"), "{text}");
    }

    #[test]
    fn since_excludes_anything_older_than_the_floor() {
        let ledger = held(vec![row("old", 1, 100), row("new", 1, 900)]);
        let filter = Filter {
            dead: false,
            since: Some(500),
        };
        assert_eq!(ids(&report(&ledger, filter, true)), vec!["new"]);
    }

    /// A row recorded EXACTLY at the floor is included. A cutoff that
    /// excluded its own boundary would drop the extraction made a week ago
    /// from `--since 7d`, which is the one thing that query names.
    #[test]
    fn the_floor_itself_is_included() {
        let filter = Filter {
            dead: false,
            since: Some(500),
        };
        assert_eq!(
            ids(&report(&held(vec![row("edge", 1, 500)]), filter, true)).len(),
            1
        );
    }

    #[test]
    fn dead_and_since_apply_together() {
        let ledger = held(vec![
            row("old-dead", 0, 100),
            row("new-dead", 0, 900),
            row("new-live", 4, 900),
        ]);
        let filter = Filter {
            dead: true,
            since: Some(500),
        };
        assert_eq!(ids(&report(&ledger, filter, true)), vec!["new-dead"]);
    }

    #[test]
    fn durations_carry_their_unit() {
        assert_eq!(duration("12h"), Ok(43_200));
        assert_eq!(duration("7d"), Ok(604_800));
        assert_eq!(duration("2w"), Ok(1_209_600));
    }

    /// A bare number could mean days or hours, and guessing would answer a
    /// different question than the one asked. The refusal names the forms
    /// that work (V28).
    #[test]
    fn a_duration_without_a_unit_is_refused_and_names_the_forms() {
        let said = duration("7").err().unwrap_or_default();
        assert!(said.contains("12h"), "{said}");
        assert!(duration("").is_err());
        assert!(duration("xd").is_err());
        assert!(duration("7y").is_err());
    }

    /// A clock earlier than the window asked for includes EVERYTHING. The
    /// alternative is wrapping to a floor near u64::MAX, which would hide
    /// every row and report it as an empty ledger.
    #[test]
    fn a_window_longer_than_the_clock_includes_everything() {
        assert_eq!(floor(100, 604_800), 0);
    }

    /// Ledger ORDER. Re-sorting a log by its most interesting column is
    /// how the thing that changed since yesterday stops being where you
    /// left it.
    #[test]
    fn the_order_is_the_ledgers_order() {
        let ledger = held(vec![row("second", 9, 0), row("first", 0, 0)]);
        assert_eq!(
            ids(&report(&ledger, Filter::default(), true)),
            vec!["second", "first"]
        );
    }

    /// V8: this crate does not own token accounting, so the column is
    /// NULL rather than a char/4 stand-in that looks measured.
    #[test]
    fn reclaim_is_absent_rather_than_estimated() {
        let out =
            report(&held(vec![row("aaa", 0, 0)]), Filter::default(), true);
        assert_eq!(out.entries.first().and_then(|e| e.reclaimed), None);
        assert!(render_human(&out).contains("net=-"), "{out:?}");
    }

    /// V39: the column is SIGNED, and a negative is the interesting case.
    /// B3 was this number reported gross, so a corpus that grew read as a
    /// corpus that shrank.
    #[test]
    fn a_losing_extraction_reports_a_negative_net() {
        let mut out =
            report(&held(vec![row("aaa", 0, 0)]), Filter::default(), true);
        if let Some(entry) = out.entries.first_mut() {
            entry.reclaimed = Some(-10);
        }
        assert!(render_human(&out).contains("net=-10"), "{out:?}");
    }

    #[test]
    fn the_original_text_is_printed_under_its_row() {
        let mut wrapped = row("aaa", 0, 0);
        wrapped.text = "- first line\n  continued".to_string();
        let text = render_human(&report(
            &held(vec![wrapped]),
            Filter::default(),
            true,
        ));
        assert!(text.contains("\n  - first line\n"), "{text}");
        assert!(text.contains("\n    continued\n"), "{text}");
    }

    #[test]
    fn an_empty_ledger_renders_nothing() {
        assert_eq!(
            render_human(&report(&held(Vec::new()), Filter::default(), true)),
            ""
        );
    }
}
