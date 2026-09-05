//! What an `S` artifact's FRONTMATTER has to carry.
//!
//! Two obligations that both live in four lines of YAML, split here when
//! the audit hit its size cap. They belong together: `no-head` and
//! `no-guard` are read from the same block, and the second is deliberately
//! silent when the first fires -- one missing block should name one fix.

use crate::{issue, ledger};

/// V43: the HEAD is what the HOST indexes.
///
/// A skill lands in the host's own directory, so the host reads it and
/// decides by its frontmatter. MEASURED before this existed: the first
/// extraction listed under its ID, because the template wrote `# <hash>`
/// as a heading and no frontmatter at all -- so the host's own trigger,
/// the description it loads by, was seven hex characters.
///
/// `M` artifacts are exempt: nothing indexes `.rekall/rules`, and inventing
/// a head for a shell script would be scaffold with no reader.
pub(super) fn head_drift(
    row: &ledger::Extracted,
    text: &str,
) -> Vec<super::Drift> {
    if has_head(text) {
        return Vec::new();
    }
    vec![super::drift(
        super::NO_HEAD,
        &row.id,
        &row.artifact,
        format!(
            "{} has no frontmatter, so the host that indexes this directory has nothing to \
             name or describe it by (V43). Give it a `---` block with `name` and \
             `description` -- by hand, because reverting and re-applying would \
             discard the triggers you filled in, which the ledger does not keep",
            row.artifact
        ),
    )]
}

/// V52: the head DISABLES the host's own loading.
///
/// Without it the host auto-loads the skill on its own judgement, and the
/// do-not-fire clause -- the one thing the host cannot express -- is
/// bypassed on the path this crate does not control. MEASURED: every `S`
/// artifact written before this check existed was model-invocable (B10).
///
/// Only where there IS a head. An artifact with no frontmatter is
/// `no-head`, and reporting both would name one fix twice.
pub(super) fn guard_drift(
    row: &ledger::Extracted,
    text: &str,
) -> Vec<super::Drift> {
    if !has_head(text) || issue::has_guard(text) {
        return Vec::new();
    }
    vec![super::drift(
        super::NO_GUARD,
        &row.id,
        &row.artifact,
        said_unguarded(row),
    )]
}

pub(super) fn said_unguarded(row: &ledger::Extracted) -> String {
    format!(
        "{} does not carry `{}`, so the host loads it on its own judgement \
         and the do-not-fire clause is bypassed (V52). Run \
         `rekall issue {}` to put the line in place",
        row.artifact,
        issue::GUARD,
        row.id
    )
}

pub(super) fn has_head(text: &str) -> bool {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return false;
    }
    let head: Vec<&str> =
        lines.take_while(|line| line.trim() != "---").collect();
    let names =
        |key: &str| head.iter().any(|line| line.trim_start().starts_with(key));
    names("name:") && names("description:")
}
