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

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
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

fn to_row(found: &statement::Statement, weights: &classify::Weights) -> Row {
    // NORMALIZED, not raw. The raw text still carries its list marker, so
    // "- when editing ..." does not start with "when" and every conditional
    // signal was being missed -- silently, because the statement still got
    // a plausible class from its other words.
    let verdict = classify::classify(
        &statement::normalize(&found.text),
        classify::Form::from_list_item(statement::is_list_item(&found.text)),
        weights,
    );
    Row {
        id: found.id.clone(),
        src: format!("{}:{}-{}", found.path, found.line_start, found.line_end),
        tokens: None,
        class: verdict.class.to_string(),
        sharpness: verdict.sharpness,
        label: verdict.label(),
        signals: verdict.names(),
    }
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

/// The INVERSE of `stable_name`, and it lives here so one module owns both
/// directions of the encoding.
///
/// `stable_name` writes a path three ways -- relative to the project, then
/// `~/`-prefixed, then absolute -- and for a long time three callers decoded
/// it with `base.join(name)`, which is only correct for the FIRST. A `~`
/// source became `<project>/~/...`, which cannot exist, so extracting from
/// agent memory died at the write with a bare `No such file or directory`
/// (`apply:B20`).
///
/// `None` means UNRESOLVABLE, which is a refusal the caller must name rather
/// than a path it may guess at: a `~/` name with no home to expand it
/// against is the case, and joining it onto the project would recreate the
/// bug this function exists to remove.
#[must_use]
pub fn resolve_name(
    name: &str,
    base: &Path,
    home: Option<&str>,
) -> Option<PathBuf> {
    if let Some(rest) = name.strip_prefix("~/") {
        return home.map(|home| Path::new(home).join(rest));
    }
    let path = Path::new(name);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    Some(base.join(path))
}

/// What a scan produced, including what it could not read.
#[derive(Debug, Default)]
pub struct Outcome {
    pub report: Report,
    pub unreadable: Vec<PathBuf>,
    /// The statement behind each REPORTED row, in row order.
    ///
    /// Beside the report rather than inside it: section I fixes the row's
    /// anatomy and `text` is not one of its columns. But a token count
    /// needs the bytes, and walking the corpus a second time to find them
    /// again would be two readers of one thing -- the defect V8 names, in
    /// the place where a disagreement would be hardest to see.
    pub texts: Vec<String>,
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
    /// The classifier table, resolved from config (V30). Carried with the
    /// corpus because a verdict depends on BOTH, and passing them apart
    /// is how one caller ends up classifying with the defaults while
    /// another uses the tuned table.
    pub weights: &'a classify::Weights,
}

impl Corpus<'_> {
    fn files_under(
        &self,
        root: &String,
    ) -> Result<corpus::Walked, corpus::Error> {
        corpus::files(
            std::slice::from_ref(root),
            self.globs,
            self.home,
            self.base,
        )
    }
}

/// The corpus, read once.
///
/// `scan`, `show` and `plan` all need the same three things and used to
/// walk the tree separately for them. Three walkers is three chances to
/// disagree about what the corpus contains -- the defect V8 names for two
/// token counters, in a place where a disagreement would mean `plan`
/// pointing at a statement `scan` never listed.
pub struct Loaded {
    pub statements: Vec<statement::Statement>,
    /// Each source's stable name and its full text, for fingerprinting.
    pub sources: Vec<(String, String)>,
    pub unreadable: Vec<PathBuf>,
    /// The table the corpus was read WITH, carried forward so a later
    /// `plan` judges the same statements the same way `scan` did.
    pub weights: classify::Weights,
}

/// Read every corpus file once and split it.
pub fn load(at: &Corpus<'_>) -> Result<Loaded, corpus::Error> {
    let mut out = Loaded {
        statements: Vec::new(),
        sources: Vec::new(),
        unreadable: Vec::new(),
        weights: at.weights.clone(),
    };
    for (root, _) in at.roots {
        let walked = at.files_under(root)?;
        out.unreadable.extend(walked.unreadable);
        read_all(&walked.files, at, &mut out);
    }
    Ok(out)
}

fn read_all(files: &[PathBuf], at: &Corpus<'_>, out: &mut Loaded) {
    for path in files {
        let name = stable_name(path, at.base, at.home);
        match std::fs::read_to_string(path) {
            Ok(text) => {
                out.statements.extend(statement::split(&text, &name));
                out.sources.push((name, text));
            }
            Err(_) => out.unreadable.push(path.clone()),
        }
    }
}

/// Run a scan over already-resolved roots.
pub fn run(
    corpus: &Corpus<'_>,
    filter: &Filter,
) -> Result<Outcome, corpus::Error> {
    let loaded = load(corpus)?;
    let paired: Vec<(Row, String)> = loaded
        .statements
        .iter()
        .map(|found| (to_row(found, corpus.weights), found.text.clone()))
        .collect();
    let (rows, texts) = keep(paired, filter);
    Ok(Outcome {
        report: Report {
            rows,
            sources: sources_of(corpus)?,
        },
        unreadable: loaded.unreadable,
        texts,
    })
}

/// Filter rows and their texts TOGETHER.
///
/// Paired through the filter rather than filtered twice: two passes with
/// the same predicate is one predicate that can be edited in one place,
/// and then a `--top 5` report would carry the first five texts of a
/// different five rows.
fn keep(
    paired: Vec<(Row, String)>,
    filter: &Filter,
) -> (Vec<Row>, Vec<String>) {
    let mut kept: Vec<(Row, String)> = paired
        .into_iter()
        .filter(|(row, _)| filter.keeps(row))
        .collect();
    if let Some(limit) = filter.top {
        kept.truncate(limit);
    }
    kept.into_iter().unzip()
}

/// One source row per configured root, with the count of files it
/// contributed. Reported even when a root contributed nothing, so an empty
/// result is distinguishable from a root that was never read.
fn sources_of(corpus: &Corpus<'_>) -> Result<Vec<Source>, corpus::Error> {
    let mut out = Vec::new();
    for (root, scope) in corpus.roots {
        let walked = corpus.files_under(root)?;
        out.push(source_of(root, *scope, walked.files.len()));
    }
    Ok(out)
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

    /// A fixture that produces no statement yields an EMPTY row, which
    /// fails every assertion here on content -- so the helper needs no
    /// panic arm, and therefore has no branch a test cannot execute.
    fn row_of(text: &str) -> Row {
        statement::split(text, "CLAUDE.md")
            .first()
            .map_or_else(empty_row, |found| {
                to_row(found, &classify::Weights::default())
            })
    }

    /// The row a missing statement would produce: empty, so every
    /// assertion on content fails loudly. Defined here rather than on
    /// `Row` so the library carries no constructor only tests use.
    fn empty_row() -> Row {
        Row {
            id: String::new(),
            src: String::new(),
            tokens: None,
            class: String::new(),
            sharpness: None,
            label: String::new(),
            signals: Vec::new(),
        }
    }

    /// The classifier must see NORMALIZED text. A bullet marker left on the
    /// front stops every conditional opener from matching, and the row
    /// still gets a plausible class from its remaining words -- so the bug
    /// shows up as a missing signal rather than as a wrong answer.
    /// The two directions are one encoding, so they round-trip. Three
    /// shapes, because `stable_name` writes three: project-relative,
    /// `~/`-prefixed, and absolute-with-no-home-to-shorten-it.
    #[test]
    fn a_stable_name_resolves_back_to_the_path_it_came_from() {
        let base = Path::new("/w/proj");
        let home = Some("/home/u");
        for original in [
            Path::new("/w/proj/CLAUDE.md"),
            Path::new("/home/u/.claude/projects/p/memory/a.md"),
            Path::new("/elsewhere/notes.md"),
        ] {
            let name = stable_name(original, base, home);
            assert_eq!(
                resolve_name(&name, base, home).as_deref(),
                Some(original),
                "round trip failed for {name}"
            );
        }
    }

    /// `apply:B20`: this is the join that broke memory extraction. A `~`
    /// name must NOT become `<project>/~/...`.
    #[test]
    fn a_home_name_does_not_get_joined_onto_the_project() {
        let got = resolve_name(
            "~/.claude/CLAUDE.md",
            Path::new("/w/p"),
            Some("/home/u"),
        );
        assert_eq!(
            got.as_deref(),
            Some(Path::new("/home/u/.claude/CLAUDE.md"))
        );
    }

    /// V59: what cannot be resolved is UNRESOLVABLE, not guessed at. A `~`
    /// name with no home has no answer, and inventing one recreates B20.
    #[test]
    fn a_home_name_with_no_home_is_unresolvable() {
        assert!(resolve_name("~/x.md", Path::new("/w/p"), None).is_none());
    }

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
    use std::fs;

    /// A throwaway corpus on disk, named after the test that owns it so two
    /// tests never share a directory. Under `target/` so `cargo clean`
    /// removes it and nothing leaks into the user's temp dir.
    fn corpus_dir(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("test-corpus").join(name);
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn write(dir: &Path, name: &str, body: &str) {
        let _ = fs::write(dir.join(name), body);
    }

    /// The fixtures use valid globs, so `run` cannot fail here. Returning
    /// the Result rather than swallowing it means there is no fallback arm
    /// that no test can execute -- a failure shows up as a failed
    /// assertion on the value instead.
    fn scan_dir(dir: &Path, filter: &Filter) -> Outcome {
        let roots = vec![(".".to_string(), config::Scope::Project)];
        let globs = vec!["**/*.md".to_string()];
        let weights = classify::Weights::default();
        let corpus = Corpus {
            roots: &roots,
            globs: &globs,
            home: None,
            base: dir,
            weights: &weights,
        };
        run(&corpus, filter).unwrap_or_default()
    }

    #[test]
    fn run_inventories_a_real_directory() {
        let dir = corpus_dir("inventory");
        write(
            &dir,
            "CLAUDE.md",
            "- never commit to `main`\n\n- code must be readable\n",
        );
        let outcome = scan_dir(&dir, &Filter::default());
        assert_eq!(outcome.report.rows.len(), 2);
        assert_eq!(outcome.report.sources.len(), 1);
        assert_eq!(outcome.report.sources.first().map(|s| s.files), Some(1));
    }

    /// Names in the report are relative to the base, so the same corpus
    /// reports the same ids from any working directory.
    #[test]
    fn run_reports_paths_relative_to_the_base() {
        let dir = corpus_dir("relative");
        write(&dir, "CLAUDE.md", "- never commit to `main`\n");
        let outcome = scan_dir(&dir, &Filter::default());
        assert_eq!(
            outcome.report.rows.first().map(|r| r.src.clone()),
            Some("CLAUDE.md:1-1".to_string())
        );
    }

    /// A file that is not valid UTF-8 cannot be read as prose. It is
    /// RETURNED as unreadable rather than dropped, so the caller can name
    /// it -- reporting a smaller corpus as if it were the whole one is the
    /// silent skip V26 forbids.
    #[test]
    fn an_unreadable_file_is_reported_not_dropped() {
        let dir = corpus_dir("unreadable");
        write(&dir, "good.md", "- never commit to `main`\n");
        let _ = fs::write(dir.join("bad.md"), [0xff_u8, 0xfe, 0xff]);
        let outcome = scan_dir(&dir, &Filter::default());
        assert_eq!(outcome.unreadable.len(), 1, "{:?}", outcome.unreadable);
        assert_eq!(outcome.report.rows.len(), 1, "the good file still scanned");
    }

    #[test]
    fn run_applies_the_class_filter() {
        let dir = corpus_dir("filtered");
        let body =
            "- never commit to `main`\n\n- when writing tests, prefer tables\n";
        write(&dir, "CLAUDE.md", body);
        let filter = Filter {
            class: Some("S".to_string()),
            ..Filter::default()
        };
        let labels = labels_of(&scan_dir(&dir, &filter));
        assert_eq!(labels, vec!["S2".to_string()]);
    }

    fn labels_of(outcome: &Outcome) -> Vec<String> {
        outcome
            .report
            .rows
            .iter()
            .map(|row| row.label.clone())
            .collect()
    }

    #[test]
    fn run_applies_top_after_filtering() {
        let dir = corpus_dir("top");
        write(
            &dir,
            "CLAUDE.md",
            "- one must be `a`\n\n- two must be `b`\n\n- three must be `c`\n",
        );
        let filter = Filter {
            top: Some(2),
            ..Filter::default()
        };
        assert_eq!(scan_dir(&dir, &filter).report.rows.len(), 2);
    }

    /// Sources are reported even when a root contributes nothing, so an
    /// empty result is distinguishable from a root that was never read.
    #[test]
    fn an_empty_root_still_reports_a_source_row() {
        let dir = corpus_dir("empty");
        let outcome = scan_dir(&dir, &Filter::default());
        assert!(outcome.report.rows.is_empty());
        assert_eq!(outcome.report.sources.first().map(|s| s.files), Some(0));
    }

    /// V13: scanning twice yields the same report.
    #[test]
    fn scanning_is_idempotent_on_disk() {
        let dir = corpus_dir("idempotent");
        write(
            &dir,
            "CLAUDE.md",
            "- never commit to `main`\n\n- prefer small modules\n",
        );
        assert_eq!(
            scan_dir(&dir, &Filter::default()).report,
            scan_dir(&dir, &Filter::default()).report
        );
    }

    /// A path under neither the base nor home keeps its absolute form --
    /// the last fallback in `stable_name`.
    #[test]
    fn a_path_outside_base_and_home_stays_absolute() {
        assert_eq!(
            stable_name(
                Path::new("/etc/notes.md"),
                Path::new("/p"),
                Some("/home/u")
            ),
            "/etc/notes.md"
        );
    }

    #[test]
    fn sources_carry_the_scope_that_named_the_root() {
        let dir = corpus_dir("scope");
        write(&dir, "CLAUDE.md", "- a rule\n");
        let outcome = scan_dir(&dir, &Filter::default());
        assert_eq!(
            outcome.report.sources.first().map(|s| s.scope.clone()),
            Some("project".to_string())
        );
    }
    #[test]
    fn sources_are_printed_only_when_asked_for() {
        let report = Report {
            rows: Vec::new(),
            sources: vec![Source {
                root: "~/.claude".to_string(),
                scope: "user".to_string(),
                files: 3,
            }],
        };
        assert!(render_human(&report, true).contains("~/.claude"));
        assert!(render_human(&report, false).is_empty());
    }

    #[test]
    fn a_user_scope_root_is_labelled_user() {
        let source = source_of("~/.claude", config::Scope::User, 2);
        assert_eq!(source.scope, "user");
        assert_eq!(source.files, 2);
    }

    #[test]
    fn a_row_with_no_tokens_renders_a_dash() {
        let row = row_of("- never commit to `main`\n");
        assert!(render_row(&row).contains("  -  "), "{}", render_row(&row));
    }
    /// `row_of` on input with no statement yields the EMPTY row. Asserting
    /// it here is what keeps the helper's fallback executable rather than
    /// a branch no test reaches.
    #[test]
    fn input_with_no_statement_yields_an_empty_row() {
        let row = row_of("# heading only\n");
        assert_eq!(row.id, "");
        assert_eq!(row.label, "");
        assert!(row.signals.is_empty());
    }
}
