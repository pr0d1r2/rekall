//! `rekall show <id>` -- one statement in full.
//!
//! `scan` truncates every statement to a row so the inventory stays
//! scannable. This is where a class is ARGUED with: V10 makes a class a
//! CLAIM rather than truth, and a claim nobody can inspect is one nobody
//! can dispute. So this prints the original text and every signal that
//! fired, not a summary of them.
//!
//! Report-only (V7). Nothing here writes.

use crate::{classify, corpus, ledger, scan, statement};

/// One statement, in full.
///
/// `Default` so a test can state the ONE field it is about and leave the
/// rest at nothing -- a fixture that spells out ten fields to assert on
/// two hides which two mattered.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Found {
    pub id: String,
    pub src: String,
    /// The statement VERBATIM, as it appears in the corpus. `scan` never
    /// shows this; it is the thing a classification is a claim about.
    pub text: String,
    pub class: String,
    pub sharpness: Option<u8>,
    pub label: String,
    /// Every signal that fired, WITH its weight (V30). Section I asks for
    /// the weight here and nowhere else: `scan` prints a row, `show` is
    /// where a class is argued with, and a balance you cannot see the
    /// terms of is not arguable.
    pub signals: Vec<classify::Signal>,
    /// The sum those weights came to, against the deadband.
    pub score: i32,
    /// Where the extraction landed, and how often it has been delivered.
    ///
    /// `None` for a statement still sitting in the corpus, which is the
    /// difference section I names as "if extracted". Both fields are on
    /// the struct rather than in a second shape, so JSON and human keep
    /// ONE anatomy across the two halves of the lookup (V17).
    pub artifact: Option<String>,
    pub fires: Option<u64>,
    /// The label the extraction was actually MADE under, when it is not
    /// the one re-derived above.
    ///
    /// `Some` is a disagreement and nothing else: the ledger keeps the
    /// statement's text but not the section it sat in, so a row extracted
    /// from under a heading can re-derive one class short of the one
    /// recorded (V64). Printing both is the only honest answer -- the
    /// verdict shown is the one this classifier reaches today, and the
    /// verdict acted on is the one in the ledger.
    pub recorded: Option<String>,
}

/// What a lookup produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Unique(Box<Found>),
    /// More than one id starts with the given prefix. Reported with the
    /// candidates rather than resolved: picking one would be a coin toss
    /// wearing the appearance of an answer.
    Ambiguous(Vec<String>),
    Missing,
}

/// Find the statement whose id starts with `prefix`.
///
/// A PREFIX, because a 7-hex id is not something anyone retypes in full
/// and `scan` output is meant to be copied from. Git's rule, for git's
/// reason.
pub fn lookup(
    at: &scan::Corpus<'_>,
    prefix: &str,
) -> Result<Lookup, corpus::Error> {
    let loaded = scan::load(at)?;
    let hits: Vec<Found> = loaded
        .statements
        .iter()
        .filter(|found| found.id.starts_with(prefix))
        .map(|found| detail(found, at.weights))
        .collect();
    Ok(resolve(hits))
}

fn resolve(mut hits: Vec<Found>) -> Lookup {
    match hits.len() {
        0 => Lookup::Missing,
        1 => hits
            .pop()
            .map_or(Lookup::Missing, |found| Lookup::Unique(Box::new(found))),
        _ => {
            Lookup::Ambiguous(hits.into_iter().map(|found| found.id).collect())
        }
    }
}

fn detail(found: &statement::Statement, weights: &classify::Weights) -> Found {
    let mut out = judged(&found.text, found.form(), weights);
    out.id = found.id.clone();
    out.src = format!("{}:{}-{}", found.path, found.line_start, found.line_end);
    out
}

/// The half both halves share: a text, a form, and the verdict they reach.
///
/// The id and the span are filled in by the caller, because that is the
/// only thing the corpus and the ledger disagree about -- everything a
/// class is argued from is right here.
fn judged(
    text: &str,
    form: classify::Form,
    weights: &classify::Weights,
) -> Found {
    let verdict =
        classify::classify(&statement::normalize(text), form, weights);
    Found {
        id: String::new(),
        src: String::new(),
        text: text.to_string(),
        class: verdict.class.to_string(),
        sharpness: verdict.sharpness,
        label: verdict.label(),
        signals: verdict.signals,
        score: verdict.score,
        artifact: None,
        fires: None,
        recorded: None,
    }
}

/// The same question, asked of the LEDGER (V67).
///
/// An id OUTLIVES the statement it names: `apply` deletes the span and
/// leaves a pointer, so the corpus stops being able to answer for a row
/// the ledger still holds -- and the verdict most worth arguing with is
/// the one already acted on.
///
/// The text is the ledger's verbatim copy (V9). The heading it sat under
/// is not kept, so the form here is the marker alone and the class is
/// RE-DERIVED rather than repeated; `recorded` carries the ledger's own
/// label wherever the two differ.
#[must_use]
pub fn lookup_extracted(
    held: &ledger::Ledger,
    prefix: &str,
    weights: &classify::Weights,
) -> Lookup {
    let hits: Vec<Found> = held
        .extracted
        .iter()
        .filter(|row| row.id.starts_with(prefix))
        .map(|row| from_row(row, weights))
        .collect();
    resolve(hits)
}

fn from_row(row: &ledger::Extracted, weights: &classify::Weights) -> Found {
    let form =
        classify::Form::from_context(statement::is_list_item(&row.text), &None);
    let mut out = judged(&row.text, form, weights);
    out.id = row.id.clone();
    out.src = format!("{}:{}-{}", row.src, row.line_start, row.line_end);
    out.artifact = Some(row.artifact.clone());
    out.fires = Some(row.fires);
    out.recorded = (row.label != out.label).then(|| row.label.clone());
    out
}

/// Human rendering. The JSON carries the SAME anatomy (V17).
#[must_use]
pub fn render_human(found: &Found) -> String {
    let mut out = format!("id       {}\n", found.id);
    out.push_str(&format!("src      {}\n", found.src));
    out.push_str(&format!("class    {}\n", found.label));
    out.push_str(&format!("score    {}\n", found.score));
    out.push_str("signals\n");
    for signal in &found.signals {
        out.push_str(&format!("  {:+}  {}\n", signal.weight, signal.name));
    }
    out.push_str(&extracted_lines(found));
    out.push_str("text\n");
    for line in found.text.lines() {
        out.push_str(&format!("  {line}\n"));
    }
    out
}

/// The three lines only an extracted row has. Absent, not empty: a field
/// printed blank reads as a value that is blank.
fn extracted_lines(found: &Found) -> String {
    let mut out = String::new();
    if let Some(label) = &found.recorded {
        out.push_str(&format!("recorded {label}  (class when extracted)\n"));
    }
    if let Some(path) = &found.artifact {
        out.push_str(&format!("artifact {path}\n"));
    }
    if let Some(fires) = found.fires {
        out.push_str(&format!("fires    {fires}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Scope;
    use std::path::{Path, PathBuf};

    fn dir(name: &str) -> PathBuf {
        let path = PathBuf::from("target").join("show").join(name);
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::create_dir_all(&path);
        path
    }

    fn look(base: &Path, prefix: &str) -> Lookup {
        let roots = vec![(".".to_string(), Scope::Project)];
        let globs = vec!["**/*.md".to_string()];
        let weights = classify::Weights::default();
        let at = scan::Corpus {
            roots: &roots,
            globs: &globs,
            home: None,
            base,
            weights: &weights,
        };
        lookup(&at, prefix).unwrap_or(Lookup::Missing)
    }

    fn unique_id(found: &Lookup) -> Option<String> {
        match found {
            Lookup::Unique(found) => Some(found.id.clone()),
            _ => None,
        }
    }

    fn candidate_count(found: &Lookup) -> Option<usize> {
        match found {
            Lookup::Ambiguous(ids) => Some(ids.len()),
            _ => None,
        }
    }

    fn text_of(found: &Lookup) -> String {
        match found {
            Lookup::Unique(found) => found.text.clone(),
            _ => String::new(),
        }
    }

    fn signals_of(found: &Lookup) -> Vec<String> {
        match found {
            Lookup::Unique(found) => {
                found.signals.iter().map(|s| s.name.clone()).collect()
            }
            _ => Vec::new(),
        }
    }

    /// The helpers above must report EMPTY for a non-unique lookup, not
    /// panic. Asserting that keeps their fallback arms executable, and it
    /// is also the behaviour every test here relies on: a lookup that went
    /// wrong shows up as a failed assertion about content.
    #[test]
    fn the_test_helpers_report_empty_for_a_non_unique_lookup() {
        assert_eq!(unique_id(&Lookup::Missing), None);
        assert_eq!(candidate_count(&Lookup::Missing), None);
        assert_eq!(text_of(&Lookup::Missing), "");
        assert!(signals_of(&Lookup::Ambiguous(Vec::new())).is_empty());
    }

    fn first_id(base: &Path) -> String {
        let text =
            std::fs::read_to_string(base.join("CLAUDE.md")).unwrap_or_default();
        statement::split(&text, "CLAUDE.md")
            .first()
            .map(|found| found.id.clone())
            .unwrap_or_default()
    }

    #[test]
    fn a_full_id_finds_its_statement() {
        let base = dir("full-id");
        let _ = std::fs::write(
            base.join("CLAUDE.md"),
            "- never commit to `main`\n",
        );
        let id = first_id(&base);
        let hit = look(&base, &id);
        assert_eq!(unique_id(&hit), Some(id), "got {hit:?}");
    }

    /// The whole point of `show`: the VERBATIM text, which `scan` never
    /// prints. A class is a claim about these exact words (V10).
    #[test]
    fn the_original_text_is_returned_verbatim() {
        let base = dir("verbatim");
        let _ = std::fs::write(
            base.join("CLAUDE.md"),
            "- never commit to `main`\n",
        );
        let text = text_of(&look(&base, &first_id(&base)));
        assert_eq!(text, "- never commit to `main`");
    }

    #[test]
    fn every_signal_that_fired_is_reported() {
        let base = dir("signals");
        let _ = std::fs::write(
            base.join("CLAUDE.md"),
            "- never commit to `main`\n",
        );
        let signals = signals_of(&look(&base, &first_id(&base)));
        assert!(signals.contains(&"never".to_string()), "{signals:?}");
        assert!(signals.contains(&"exact-handle".to_string()), "{signals:?}");
    }

    #[test]
    fn a_prefix_finds_its_statement() {
        let base = dir("prefix");
        let _ = std::fs::write(
            base.join("CLAUDE.md"),
            "- never commit to `main`\n",
        );
        let id = first_id(&base);
        let short = id.get(..3).unwrap_or(&id).to_string();
        assert!(matches!(look(&base, &short), Lookup::Unique(_)));
    }

    /// An ambiguous prefix is REPORTED with its candidates, never resolved.
    /// Picking one would be a coin toss wearing the appearance of an answer.
    #[test]
    fn an_empty_prefix_is_ambiguous_and_names_the_candidates() {
        let base = dir("ambiguous");
        let _ = std::fs::write(
            base.join("CLAUDE.md"),
            "- rule one\n\n- rule two\n",
        );
        let hit = look(&base, "");
        assert_eq!(candidate_count(&hit), Some(2), "got {hit:?}");
    }

    #[test]
    fn an_id_that_matches_nothing_is_missing() {
        let base = dir("missing");
        let _ = std::fs::write(base.join("CLAUDE.md"), "- a rule\n");
        assert_eq!(look(&base, "zzzzzzz"), Lookup::Missing);
    }

    #[test]
    fn an_unreadable_file_does_not_stop_the_lookup() {
        let base = dir("unreadable");
        let _ =
            std::fs::write(base.join("good.md"), "- never commit to `main`\n");
        let _ = std::fs::write(base.join("bad.md"), [0xff_u8, 0xfe]);
        assert!(matches!(look(&base, ""), Lookup::Unique(_)));
    }

    fn a_found() -> Found {
        Found {
            id: "abc1234".to_string(),
            src: "CLAUDE.md:1-1".to_string(),
            text: "- never commit to `main`".to_string(),
            class: "M".to_string(),
            sharpness: Some(1),
            label: "M1".to_string(),
            signals: vec![classify::Signal {
                name: "never".to_string(),
                weight: 2,
            }],
            score: 2,
            ..Found::default()
        }
    }

    #[test]
    fn human_output_carries_text_class_and_signals() {
        let text = render_human(&a_found());
        assert!(text.contains("id       abc1234"));
        assert!(text.contains("class    M1"));
        assert!(text.contains("score    2"), "{text}");
        assert!(text.contains("  +2  never"), "{text}");
        assert!(text.contains("  - never commit to `main`"));
    }

    #[test]
    fn a_multi_line_statement_is_indented_line_by_line() {
        let found = Found {
            id: "a".to_string(),
            src: "x:1-2".to_string(),
            text: "- first\n  second".to_string(),
            label: "U".to_string(),
            ..Found::default()
        };
        let text = render_human(&found);
        assert!(
            text.contains("  - first\n  second") || text.contains("    second")
        );
    }
}
