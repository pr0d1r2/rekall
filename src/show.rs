//! `rekall show <id>` -- one statement in full.
//!
//! `scan` truncates every statement to a row so the inventory stays
//! scannable. This is where a class is ARGUED with: V10 makes a class a
//! CLAIM rather than truth, and a claim nobody can inspect is one nobody
//! can dispute. So this prints the original text and every signal that
//! fired, not a summary of them.
//!
//! Report-only (V7). Nothing here writes.

use crate::{classify, corpus, scan, statement};

/// One statement, in full.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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
    let verdict =
        classify::classify(&statement::normalize(&found.text), weights);
    Found {
        id: found.id.clone(),
        src: format!("{}:{}-{}", found.path, found.line_start, found.line_end),
        text: found.text.clone(),
        class: verdict.class.to_string(),
        sharpness: verdict.sharpness,
        label: verdict.label(),
        signals: verdict.signals.clone(),
        score: verdict.score,
    }
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
    out.push_str("text\n");
    for line in found.text.lines() {
        out.push_str(&format!("  {line}\n"));
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
            class: "U".to_string(),
            sharpness: None,
            label: "U".to_string(),
            signals: Vec::new(),
            score: 0,
        };
        let text = render_human(&found);
        assert!(
            text.contains("  - first\n  second") || text.contains("    second")
        );
    }
}
