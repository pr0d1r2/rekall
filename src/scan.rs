//! `rekall scan` -- inventory the corpus, one row per statement.
//!
//! Deterministic and report-only (V7). Nothing here writes, and the same
//! corpus produces the same report on any machine (V13).

use crate::{classify, config, corpus, statement};
use std::path::{Path, PathBuf};

/// One statement, as `scan` reports it.
///
/// `tokens` is `None` until T16 wires itok in. Emitted as an explicit null
/// rather than omitted, so the JSON anatomy does not change shape when it
/// arrives -- and so "not measured" is visible instead of looking like a
/// statement that costs nothing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Row {
    pub id: String,
    /// `file:line-line`, the form section I specifies.
    pub src: String,
    pub tokens: Option<u32>,
    pub class: String,
    pub sharpness: Option<u8>,
    pub label: String,
    pub signals: Vec<String>,
}

/// A corpus root and which config scope named it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Source {
    pub root: String,
    pub scope: String,
    pub files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub rows: Vec<Row>,
    pub sources: Vec<Source>,
}

/// Filters, straight from the flags.
#[derive(Debug, Default, Clone)]
pub struct Filter {
    pub class: Option<String>,
    pub sharpness: Option<u8>,
    pub top: Option<usize>,
}

impl Filter {
    /// `--class M` matches every `M*`: the digit is a DEGREE of the class,
    /// not a separate class, so filtering by letter must not require
    /// knowing the number.
    fn keeps(&self, row: &Row) -> bool {
        let class_ok = self
            .class
            .as_ref()
            .is_none_or(|wanted| row.class.eq_ignore_ascii_case(wanted));
        let sharp_ok = self
            .sharpness
            .is_none_or(|wanted| row.sharpness == Some(wanted));
        class_ok && sharp_ok
    }
}

fn to_row(found: &statement::Statement) -> Row {
    // NORMALIZED, not raw. The raw text still carries its list marker, so
    // "- when editing ..." does not start with "when" and every conditional
    // signal was being missed -- silently, because the statement still got
    // a plausible class from its other words.
    let verdict = classify::classify(&statement::normalize(&found.text));
    Row {
        id: found.id.clone(),
        src: format!("{}:{}-{}", found.path, found.line_start, found.line_end),
        tokens: None,
        class: verdict.class.to_string(),
        sharpness: verdict.sharpness,
        label: verdict.label(),
        signals: verdict.signals.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// Read one file and classify every statement in it.
///
/// An unreadable file is SKIPPED rather than fatal -- a corpus is other
/// people's files and one bad permission bit must not stop the inventory --
/// but the skip is returned so the caller can say so. A silent skip would
/// report a smaller corpus as if it were the whole one (V26).
fn rows_for(path: &Path, name: &str) -> Result<Vec<Row>, PathBuf> {
    let text = std::fs::read_to_string(path).map_err(|_| path.to_path_buf())?;
    Ok(statement::split(&text, name).iter().map(to_row).collect())
}

/// The STABLE name of a corpus file: relative to the project base when it
/// sits under it, `~`-prefixed when it sits under the home directory,
/// absolute otherwise.
///
/// Not cosmetic. Ids are scoped by PATH, so a raw absolute path would give
/// one statement a different id under `-C /abs/path` than from inside the
/// project, and a different one again on a machine whose home is
/// elsewhere. The ledger records ids, so that would make an extraction
/// unfindable from anywhere except the directory that produced it -- V13's
/// stability requirement broken by the environment rather than by an edit.
#[must_use]
pub fn stable_name(path: &Path, base: &Path, home: Option<&str>) -> String {
    if let Ok(relative) = path.strip_prefix(base) {
        return relative.to_string_lossy().to_string();
    }
    if let Some(home) = home
        && let Ok(relative) = path.strip_prefix(home)
    {
        return format!("~/{}", relative.to_string_lossy());
    }
    path.to_string_lossy().to_string()
}

/// What a scan produced, including what it could not read.
pub struct Outcome {
    pub report: Report,
    pub unreadable: Vec<PathBuf>,
}

/// Where the corpus is: the roots with the scope that named each, the
/// globs, and the two anchors those resolve against.
///
/// Bundled into one value when the four-argument limit fired on `run`.
/// They travel together and mean nothing apart -- a base with no roots
/// resolves nothing -- so the limit named a real seam rather than an
/// arbitrary one.
pub struct Corpus<'a> {
    pub roots: &'a [(String, config::Scope)],
    pub globs: &'a [String],
    pub home: Option<&'a str>,
    pub base: &'a Path,
}

/// Run a scan over already-resolved roots.
pub fn run(
    corpus: &Corpus<'_>,
    filter: &Filter,
) -> Result<Outcome, corpus::Error> {
    let mut rows = Vec::new();
    let mut sources = Vec::new();
    let mut unreadable = Vec::new();
    for (root, scope) in corpus.roots {
        let found = corpus.files_under(root)?;
        sources.push(source_of(root, *scope, found.len()));
        gather(&found, &mut rows, &mut unreadable, corpus);
    }
    Ok(Outcome {
        report: finish(rows, sources, filter),
        unreadable,
    })
}

impl Corpus<'_> {
    fn files_under(
        &self,
        root: &String,
    ) -> Result<Vec<PathBuf>, corpus::Error> {
        corpus::files(
            std::slice::from_ref(root),
            self.globs,
            self.home,
            self.base,
        )
    }
}

fn gather(
    files: &[PathBuf],
    rows: &mut Vec<Row>,
    unreadable: &mut Vec<PathBuf>,
    at: &Corpus<'_>,
) {
    for file in files {
        match rows_for(file, &stable_name(file, at.base, at.home)) {
            Ok(found) => rows.extend(found),
            Err(path) => unreadable.push(path),
        }
    }
}

fn source_of(root: &str, scope: config::Scope, files: usize) -> Source {
    let scope = match scope {
        config::Scope::User => "user",
        config::Scope::Project => "project",
    };
    Source {
        root: root.to_string(),
        scope: scope.to_string(),
        files,
    }
}

fn finish(rows: Vec<Row>, sources: Vec<Source>, filter: &Filter) -> Report {
    let mut kept: Vec<Row> =
        rows.into_iter().filter(|row| filter.keeps(row)).collect();
    if let Some(limit) = filter.top {
        kept.truncate(limit);
    }
    Report {
        rows: kept,
        sources,
    }
}

/// Human rendering. The JSON carries the SAME anatomy (V17): the same
/// fields, the same order, the same names -- so an agent reading one and a
/// person reading the other are looking at one report.
#[must_use]
pub fn render_human(report: &Report, show_sources: bool) -> String {
    let mut out = String::new();
    if show_sources {
        for source in &report.sources {
            out.push_str(&format!(
                "source  {}  {}  {} file(s)\n",
                source.scope, source.root, source.files
            ));
        }
    }
    for row in &report.rows {
        out.push_str(&format!("{}\n", render_row(row)));
    }
    out
}

fn render_row(row: &Row) -> String {
    let tokens = row
        .tokens
        .map_or_else(|| "-".to_string(), |count| count.to_string());
    format!(
        "{}  {}  {}  {}  {}",
        row.id,
        row.src,
        tokens,
        row.label,
        row.signals.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `expect` rather than a blanket allow: the exemption names what is
    /// deliberate and stops compiling if the panic ever leaves this helper.
    #[expect(
        clippy::panic,
        reason = "a fixture that produces no statement must fail loudly"
    )]
    fn row_of(text: &str) -> Row {
        let found = statement::split(text, "CLAUDE.md");
        match found.first() {
            Some(first) => to_row(first),
            None => panic!("fixture produced no statement"),
        }
    }

    /// The classifier must see NORMALIZED text. A bullet marker left on the
    /// front stops every conditional opener from matching, and the row
    /// still gets a plausible class from its remaining words -- so the bug
    /// shows up as a missing signal rather than as a wrong answer.
    #[test]
    fn a_bulleted_conditional_still_fires_its_trigger_signal() {
        let row = row_of("- when writing tests, prefer table-driven cases\n");
        assert!(
            row.signals.iter().any(|s| s == "when"),
            "signals were {:?}",
            row.signals
        );
    }

    #[test]
    fn src_is_the_file_line_line_form() {
        let row = row_of("- a rule\n");
        assert_eq!(row.src, "CLAUDE.md:1-1");
    }

    /// `tokens` is null until T16, and stays PRESENT so the JSON anatomy
    /// does not change shape when it arrives.
    #[test]
    fn tokens_is_present_and_null_until_delegated() {
        assert_eq!(row_of("- a rule\n").tokens, None);
    }

    #[test]
    fn class_filter_matches_every_sharpness_of_that_class() {
        let filter = Filter {
            class: Some("M".to_string()),
            ..Filter::default()
        };
        let m1 = row_of("- never commit to `main`\n");
        let m3 = row_of("- code must be readable\n");
        assert!(filter.keeps(&m1) && filter.keeps(&m3));
    }

    #[test]
    fn class_filter_is_case_insensitive() {
        let filter = Filter {
            class: Some("m".to_string()),
            ..Filter::default()
        };
        assert!(filter.keeps(&row_of("- never commit to `main`\n")));
    }

    #[test]
    fn sharpness_filter_selects_one_rung() {
        let filter = Filter {
            sharpness: Some(1),
            ..Filter::default()
        };
        assert!(filter.keeps(&row_of("- never commit to `main`\n")));
        assert!(!filter.keeps(&row_of("- code must be readable\n")));
    }

    #[test]
    fn human_and_json_carry_the_same_row_count() {
        let rows = vec![row_of("- never commit to `main`\n")];
        let report = Report {
            rows,
            sources: Vec::new(),
        };
        let human = render_human(&report, false);
        assert_eq!(human.lines().count(), report.rows.len());
    }
    #[test]
    fn a_name_under_the_base_is_relative_to_it() {
        assert_eq!(
            stable_name(Path::new("/p/CLAUDE.md"), Path::new("/p"), None),
            "CLAUDE.md"
        );
    }

    #[test]
    fn a_name_under_home_is_tilde_prefixed() {
        assert_eq!(
            stable_name(
                Path::new("/home/u/.claude/x.md"),
                Path::new("/p"),
                Some("/home/u")
            ),
            "~/.claude/x.md"
        );
    }

    /// The property that matters: the id must not depend on WHERE the scan
    /// was run from. Scoping ids by a raw absolute path would give one
    /// statement a different id under `-C` than from inside the project,
    /// and a different one again on a machine whose home is elsewhere --
    /// which would make a ledger entry unfindable from anywhere but the
    /// directory that produced it.
    #[test]
    fn the_same_file_gets_the_same_name_from_any_working_directory() {
        let from_inside =
            stable_name(Path::new("/p/CLAUDE.md"), Path::new("/p"), None);
        let from_elsewhere = stable_name(
            Path::new("/p/CLAUDE.md"),
            Path::new("/p"),
            Some("/home/u"),
        );
        assert_eq!(from_inside, from_elsewhere);
    }
}
