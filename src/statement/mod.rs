//! Prose -> addressable statements.
//!
//! A corpus file is markdown-ish: headings, bullets, paragraphs, fenced
//! code. This splits it into the units a class is passed on, and gives each
//! one the id section I specifies -- 7 hex of a digest over the normalized
//! text, scoped by source path, with `.2`, `.3` for exact repeats.

/// The FNV-1a 64 offset basis and prime. Both are constants of the
/// published algorithm, not tuning knobs.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// How many hex characters an id shows.
const ID_LEN: usize = 7;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub id: String,
    pub path: String,
    /// 1-based, inclusive on both ends -- the `file:line-line` form section
    /// I prints.
    pub line_start: usize,
    pub line_end: usize,
    pub text: String,
}

/// A stable digest.
///
/// FNV-1a 64, implemented rather than imported, because the requirement is
/// a FIXED algorithm and not a good one: ids are recorded in the ledger and
/// compared across machines and across years, so the function must never
/// change. `std::hash::DefaultHasher` is documented as unstable across Rust
/// releases and would silently repoint every id in a ledger on a toolchain
/// bump -- which V13 forbids in the one way nothing would detect.
///
/// Correctness is asserted against the algorithm's published test vectors
/// rather than trusted, since a wrong implementation here fails silently.
#[must_use]
pub fn digest(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Normalize a statement for hashing.
///
/// Collapses whitespace and drops a leading list marker, so reflowing a
/// bullet or re-indenting it does not change its id. Case and wording are
/// preserved: a reworded rule IS a new claim and must be reclassified
/// rather than inherit a verdict passed on different words.
#[must_use]
pub fn normalize(text: &str) -> String {
    let stripped = strip_marker(text.trim_start());
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Remove a leading `-`, `*`, `+` or `N.` bullet marker.
fn strip_marker(line: &str) -> &str {
    if let Some(rest) = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| line.strip_prefix("+ "))
    {
        return rest;
    }
    strip_ordered_marker(line).unwrap_or(line)
}

fn strip_ordered_marker(line: &str) -> Option<&str> {
    let dot = line.find(". ")?;
    let (digits, rest) = line.split_at(dot);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    rest.strip_prefix(". ")
}

/// True when a statement was written as a LIST ITEM.
///
/// The FORM is evidence about the statement (V40): a corpus states its
/// rules as bullets and its context as paragraphs. It has to be read from
/// the RAW text and carried to the classifier, because `normalize` drops
/// the marker -- an id must survive a reflow -- and the fact is gone by
/// the time the classifier sees the words.
#[must_use]
pub fn is_list_item(text: &str) -> bool {
    text.lines().next().is_some_and(starts_statement)
}

/// True when a line opens a new statement rather than continuing one.
fn starts_statement(line: &str) -> bool {
    let trimmed = line.trim_start();
    strip_marker(trimmed) != trimmed
}

/// A line that makes no claim of its own.
///
/// A HEADING names a section, and a POINTER is this tool's own mark: a
/// statement it already extracted. Reading a pointer back as prose feeds
/// the output in as input -- phantom `U` rows inflating the rate `init`
/// diagnoses, and an id that MOVES when a neighbour is extracted, because
/// two adjacent pointers merge into one block.
fn is_structure(line: &str) -> bool {
    is_heading(line) || crate::apply::is_pointer(line)
}

fn is_heading(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn is_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// A run of lines that will become one statement.
struct Block {
    start: usize,
    lines: Vec<String>,
}

impl Block {
    fn text(&self) -> String {
        self.lines.join("\n")
    }

    fn end(&self) -> usize {
        self.start
            .saturating_add(self.lines.len().saturating_sub(1))
    }
}

/// Split a corpus file into statements.
///
/// Headings and fenced code are STRUCTURE, not statements: a heading makes
/// no claim to classify, and code inside a fence is an example of a rule
/// rather than a statement of one. Extracting either would produce an
/// artifact nothing could trigger on.
#[must_use]
pub fn split(text: &str, path: &str) -> Vec<Statement> {
    let blocks = blocks(text);
    let mut seen: Vec<(String, usize)> = Vec::new();
    blocks
        .into_iter()
        .map(|block| build(block, path, &mut seen))
        .collect()
}

/// Line-by-line splitting state. Extracted from `blocks` when the 15-line
/// limit fired on it: the loop body was carrying fence tracking, block
/// accumulation and flushing at once, and naming those as methods made each
/// one testable in isolation. The limit found a real seam rather than an
/// arbitrary one.
struct Split {
    out: Vec<Block>,
    current: Option<Block>,
    in_fence: bool,
}

impl Split {
    fn new() -> Self {
        Self {
            out: Vec::new(),
            current: None,
            in_fence: false,
        }
    }

    fn feed(&mut self, index: usize, line: &str) {
        if is_fence(line) {
            self.in_fence = !self.in_fence;
            let pending = self.current.take();
            push(&mut self.out, pending);
            return;
        }
        let at = Line {
            index,
            line,
            in_fence: self.in_fence,
        };
        self.current = step(self.current.take(), &mut self.out, at);
    }

    fn finish(mut self) -> Vec<Block> {
        let pending = self.current.take();
        push(&mut self.out, pending);
        self.out
    }
}

fn blocks(text: &str) -> Vec<Block> {
    let mut state = Split::new();
    for (index, line) in text.lines().enumerate() {
        state.feed(index, line);
    }
    state.finish()
}

struct Line<'a> {
    index: usize,
    line: &'a str,
    in_fence: bool,
}

fn step(
    current: Option<Block>,
    out: &mut Vec<Block>,
    at: Line<'_>,
) -> Option<Block> {
    if at.in_fence || at.line.trim().is_empty() || is_structure(at.line) {
        push(out, current);
        return None;
    }
    if starts_statement(at.line) {
        push(out, current);
        return Some(Block {
            start: at.index.saturating_add(1),
            lines: vec![at.line.to_string()],
        });
    }
    Some(extend(current, at))
}

fn extend(current: Option<Block>, at: Line<'_>) -> Block {
    match current {
        Some(mut block) => {
            block.lines.push(at.line.to_string());
            block
        }
        None => Block {
            start: at.index.saturating_add(1),
            lines: vec![at.line.to_string()],
        },
    }
}

fn push(out: &mut Vec<Block>, block: Option<Block>) {
    if let Some(block) = block {
        out.push(block);
    }
}

/// Give one block its id, disambiguating exact repeats with `.2`, `.3`.
fn build(
    block: Block,
    path: &str,
    seen: &mut Vec<(String, usize)>,
) -> Statement {
    let text = block.text();
    let base = hash_id(path, &normalize(&text));
    let id = disambiguate(&base, seen);
    Statement {
        id,
        path: path.to_string(),
        line_start: block.start,
        line_end: block.end(),
        text,
    }
}

/// The id is scoped by PATH: identical prose in two files is two
/// statements, because extracting one must not claim to have extracted the
/// other.
fn hash_id(path: &str, normalized: &str) -> String {
    let mut bytes = Vec::from(path.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(normalized.as_bytes());
    let hex = format!("{:016x}", digest(&bytes));
    hex.get(..ID_LEN).unwrap_or(&hex).to_string()
}

fn disambiguate(base: &str, seen: &mut Vec<(String, usize)>) -> String {
    if let Some(entry) = seen.iter_mut().find(|(id, _)| id == base) {
        entry.1 = entry.1.saturating_add(1);
        return format!("{base}.{}", entry.1);
    }
    seen.push((base.to_string(), 1));
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published FNV-1a 64 vectors. These assert that the digest IS
    /// FNV-1a and not merely deterministic -- a subtly wrong hash would
    /// still produce stable ids, so nothing else in this file would notice.
    #[test]
    fn digest_matches_the_published_fnv1a64_vectors() {
        assert_eq!(digest(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(digest(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(digest(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn normalize_collapses_whitespace_and_drops_the_bullet() {
        assert_eq!(
            normalize("-   never   commit\n  to main"),
            "never commit to main"
        );
        assert_eq!(normalize("3. third rule"), "third rule");
        assert_eq!(normalize("* starred"), "starred");
    }

    #[test]
    fn normalize_keeps_case_because_rewording_is_a_new_claim() {
        assert_ne!(normalize("Never Commit"), normalize("never commit"));
    }

    #[test]
    fn headings_are_structure_not_statements() {
        let found = split("# Title\n\n- a rule\n", "CLAUDE.md");
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().map(|s| s.text.as_str()), Some("- a rule"));
    }

    #[test]
    fn fenced_code_is_not_a_statement() {
        let text = "- a rule\n\n```sh\nrm -rf /\n```\n\n- another\n";
        let found = split(text, "CLAUDE.md");
        let texts: Vec<&str> = found.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["- a rule", "- another"]);
    }

    #[test]
    fn a_bullet_carries_its_continuation_lines() {
        let found = split("- first line\n  continued here\n", "CLAUDE.md");
        assert_eq!(found.len(), 1);
        let only = found.first().map(|s| (s.line_start, s.line_end));
        assert_eq!(only, Some((1, 2)));
    }

    #[test]
    fn a_blank_line_ends_a_statement() {
        let found = split("para one\n\npara two\n", "notes.md");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn spans_are_one_based_and_inclusive() {
        let found = split("# H\n\n- rule\n", "CLAUDE.md");
        let span = found.first().map(|s| (s.line_start, s.line_end));
        assert_eq!(span, Some((3, 3)));
    }

    /// The property V13 actually requires: editing one statement must not
    /// move another's id. This is what `file:line` fails.
    #[test]
    fn an_edit_elsewhere_does_not_move_an_id() {
        let before = split("- keep me\n\n- edit me\n", "CLAUDE.md");
        let after =
            split("- keep me\n\n- edited\n\n- and a third\n", "CLAUDE.md");
        let first_before = before.first().map(|s| s.id.clone());
        let first_after = after.first().map(|s| s.id.clone());
        assert_eq!(first_before, first_after);
    }

    #[test]
    fn inserting_lines_above_does_not_move_an_id() {
        let before = split("- rule\n", "CLAUDE.md");
        let after =
            split("# New heading\n\nsome new prose\n\n- rule\n", "CLAUDE.md");
        assert_eq!(
            before.first().map(|s| s.id.clone()),
            after.last().map(|s| s.id.clone()),
            "an id must survive lines being inserted above it"
        );
    }

    #[test]
    fn rewording_a_statement_changes_its_id() {
        let before = split("- never commit to main\n", "CLAUDE.md");
        let after = split("- never push to main\n", "CLAUDE.md");
        assert_ne!(
            before.first().map(|s| s.id.clone()),
            after.first().map(|s| s.id.clone())
        );
    }

    #[test]
    fn reflowing_a_statement_does_not_change_its_id() {
        let flat = split("- never commit to main\n", "CLAUDE.md");
        let wrapped = split("- never   commit\n  to main\n", "CLAUDE.md");
        assert_eq!(
            flat.first().map(|s| s.id.clone()),
            wrapped.first().map(|s| s.id.clone())
        );
    }

    #[test]
    fn the_same_prose_in_two_files_gets_two_ids() {
        let here = split("- shared rule\n", "CLAUDE.md");
        let there = split("- shared rule\n", "AGENTS.md");
        assert_ne!(
            here.first().map(|s| s.id.clone()),
            there.first().map(|s| s.id.clone())
        );
    }

    #[test]
    fn exact_repeats_within_one_file_are_disambiguated() {
        let found = split("- same\n\n- same\n\n- same\n", "CLAUDE.md");
        let ids: Vec<String> = found.iter().map(|s| s.id.clone()).collect();
        let base = ids.first().cloned().unwrap_or_default();
        assert_eq!(ids.get(1), Some(&format!("{base}.2")));
        assert_eq!(ids.get(2), Some(&format!("{base}.3")));
    }

    #[test]
    fn ids_are_seven_hex_characters() {
        let found = split("- a rule\n", "CLAUDE.md");
        let id = found.first().map(|s| s.id.clone()).unwrap_or_default();
        assert_eq!(id.len(), ID_LEN);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "id was {id}");
    }

    /// V13: `scan(scan(x))` is identical.
    #[test]
    fn splitting_is_idempotent() {
        let text = "# H\n\n- one\n\npara\n\n- two\n";
        assert_eq!(split(text, "CLAUDE.md"), split(text, "CLAUDE.md"));
    }
    /// `strip_ordered_marker` must reject a non-numeric prefix. "e.g. never
    /// commit" is prose, not a numbered list item, and stripping "e." from
    /// the front would change the statement's normalized text and so its
    /// ID -- silently, for one sentence shape.
    #[test]
    fn a_non_numeric_dotted_prefix_is_not_a_list_marker() {
        assert_eq!(normalize("e.g. never commit"), "e.g. never commit");
        assert_eq!(normalize("Fig. 3 shows it"), "Fig. 3 shows it");
    }

    #[test]
    fn a_numeric_prefix_without_a_space_is_not_a_marker() {
        assert_eq!(normalize("3.5 releases"), "3.5 releases");
    }

    /// V40 reads the FORM off the raw text, so every marker `normalize`
    /// strips has to be one `is_list_item` recognizes -- otherwise a rule
    /// written with a `*` is classified as prose and one written with a
    /// `-` is not, for no reason a reader could find.
    #[test]
    fn every_marker_normalize_strips_is_a_list_item() {
        for text in ["- a rule", "* a rule", "+ a rule", "1. a rule"] {
            assert!(is_list_item(text), "{text}");
        }
    }

    #[test]
    fn a_paragraph_is_not_a_list_item() {
        assert!(!is_list_item("Read that as a warning, not a claim."));
        assert!(!is_list_item("e.g. never commit"));
    }

    /// The form belongs to the statement, and a statement is its FIRST
    /// line plus what continues it. Reading any other line would call a
    /// wrapped bullet prose.
    #[test]
    fn a_wrapped_bullet_is_still_a_list_item() {
        assert!(is_list_item(
            "- the coverage floor only ever rises\n  and never falls"
        ));
    }
    /// T55. A pointer is this tool's OWN mark, so reading it back as
    /// prose feeds the output in as input.
    #[test]
    fn a_pointer_is_not_a_statement() {
        let text = format!(
            "# Rules\n\n{}\n\n- a real rule\n",
            crate::apply::pointer_of("abc1234")
        );
        let found = split(&text, "CLAUDE.md");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found.first().is_some_and(|one| one.text.contains("real")));
    }

    /// TWO ADJACENT pointers, which is what CLAUDE.md actually looked like
    /// after three extractions: they merged into ONE block, so the block's
    /// id moved whenever a neighbour was extracted.
    #[test]
    fn adjacent_pointers_do_not_merge_into_a_statement() {
        let text = format!(
            "{}\n{}\n- a real rule\n",
            crate::apply::pointer_of("abc1234"),
            crate::apply::pointer_of("def5678")
        );
        let found = split(&text, "CLAUDE.md");
        assert_eq!(found.len(), 1, "{found:?}");
    }

    /// An indented pointer is still a pointer. `revert` finds one without
    /// regard to whitespace, and the two directions have to agree.
    #[test]
    fn an_indented_pointer_is_still_structure() {
        let found = split("  <!-- rekall abc1234 -->\n", "CLAUDE.md");
        assert!(found.is_empty(), "{found:?}");
    }
}
