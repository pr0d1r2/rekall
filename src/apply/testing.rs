//! The step fixture both test modules build on.
//!
//! Shared rather than duplicated, the way `check::testing` is: two copies of
//! a plan step that drift apart would let one side's tests pass against a
//! shape the other side never sees.

use crate::plan;

pub(super) fn step(
    id: &str,
    start: usize,
    end: usize,
    label: &str,
) -> plan::Step {
    plan::Step {
        id: id.to_string(),
        src: "CLAUDE.md".to_string(),
        line_start: start,
        line_end: end,
        text: "- never commit to `main`".to_string(),
        label: label.to_string(),
        artifact: ".rekall/rules/no-main.sh".to_string(),
        runner: String::new(),
        wiring: "wire it".to_string(),
        net: None,
    }
}
