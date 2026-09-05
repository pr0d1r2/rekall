//! The gate's own fixtures, shared by the audit's tests and the artifact
//! module's.
//!
//! Declared HERE rather than at the top of `mod.rs`: the module-size gate
//! counts lines before the first `#[cfg(test)]`, so a test-only module
//! declared above the code would truncate that count and hide however much
//! followed it -- the same reason `cli::testing` sits where it does.

use super::Drift;
use crate::{apply, ledger};

pub(super) fn row(label: &str) -> ledger::Extracted {
    ledger::Extracted {
        id: "abc1234".to_string(),
        src: "CLAUDE.md".to_string(),
        line_start: 3,
        line_end: 3,
        text: "- never commit to `main`".to_string(),
        label: label.to_string(),
        artifact: ".rekall/rules/no-main.sh".to_string(),
        fires: 0,
        at: 0,
        issued_to: String::new(),
    }
}

/// A source file in the state `apply` leaves it: pointer present,
/// statement gone.
pub(super) fn extracted_source(held: &ledger::Extracted) -> String {
    format!(
        "# Rules\n\n{}\n\n- other prose\n",
        apply::pointer_of(&held.id)
    )
}

/// A runner that really checks something, with its payload MARKED as
/// the template writes it (V43).
pub(super) const RUNNER: &str = "#!/bin/sh\n# rekall:payload\n# - never commit to `main`\n# rekall:/payload\ngrep -q main .git/HEAD && exit 1\n";
/// A skill whose BLOCKS are filled -- the state V29 asks for. The
/// prose is deliberately absent: it is never read, so a fixture that
/// carried some would test nothing.
pub(super) const SKILL: &str = "---\nname: s\ndescription: \"- never commit to `main`\"\ndisable-model-invocation: true\n---\n\n<!-- rekall:payload -->\n- never commit to `main`\n<!-- rekall:/payload -->\n\n## Fires when\n\n```rekall\npath = [\"**/*.rs\"]\n```\n\n\
                     ## Does NOT fire when\n\n```rekall\npath = [\"**/tests/**\"]\n```\n";

/// A skill in the shape the OLD template wrote: both headings, words
/// under each, no block anywhere.
pub(super) fn prose_skill() -> String {
    format!(
        "---\nname: s\ndescription: \"- never commit to `main`\"\ndisable-model-invocation: true\n---\n\n<!-- rekall:payload -->\n- never commit to `main`\n<!-- rekall:/payload -->\n\n{}\n\nEditing any `*.rs` file.\n\n{}\n\nReading, or in a test.\n",
        apply::FIRES,
        apply::NOT_FIRES
    )
}

pub(super) fn blocks(fire: &str, refuse: &str) -> String {
    format!(
        "---\nname: s\ndescription: \"- never commit to `main`\"\ndisable-model-invocation: true\n---\n\n<!-- rekall:payload -->\n- never commit to `main`\n<!-- rekall:/payload -->\n\n{}\n\n```rekall\n{fire}\n```\n\n{}\n\n```rekall\n{refuse}\n```\n",
        apply::FIRES,
        apply::NOT_FIRES
    )
}

pub(super) fn kinds(found: &[Drift]) -> Vec<&str> {
    found.iter().map(|one| one.kind).collect()
}
