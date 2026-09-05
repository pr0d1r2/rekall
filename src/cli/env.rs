//! The process facts, and the one decision about them.
//!
//! Split out of `cli` when that file passed its size cap, along a seam the
//! module doc already drew: `cwd` and `HOME` are globals the library refuses
//! to reach for, so they arrive as arguments. `corpus_home` is the only
//! judgement among them, and it decides where a destructive verb will look.

use std::path::PathBuf;

/// The process facts the library needs but must not READ for itself.
///
/// `current_dir` and `HOME` are process globals. A library that reaches for
/// them is a library whose behaviour depends on state no caller passed and
/// no test can set -- `std::env::set_var` is unsafe in edition 2024, and
/// mutating it would race other tests anyway. The binary reads them once,
/// at the edge, and hands them in.
#[derive(Debug, Clone)]
pub struct Env {
    pub cwd: PathBuf,
    pub home: Option<String>,
}

/// The name of the override, so the docs and the code cannot disagree.
pub const HOME_OVERRIDE: &str = "REKALL_HOME";

/// Which home the CORPUS resolves a `~` root against (V63).
///
/// `REKALL_HOME` wins where it is set to something; otherwise `HOME`. The
/// DECISION lives here rather than in `main` because `fn main` cannot be
/// called by a test -- every line kept there is permanently unverifiable, and
/// this one decides where a destructive verb will look.
///
/// EMPTY IS UNSET, which is `config::non_empty_var`'s lesson one module over:
/// `XDG_CONFIG_HOME=""` reaching that code unguarded resolved a user config
/// to `/rekall/rekall.toml`. Here an empty override would resolve `~/x` to
/// `/x` -- a path outside anything the caller named.
///
/// It does NOT make the real corpus safe. Forgetting the variable gives
/// exactly today's behaviour, so the guard that matters is still `.:V7` --
/// `plan` naming every file before `apply` touches one. What this buys is a
/// SCRATCH corpus to point a dangerous experiment at.
#[must_use]
pub fn corpus_home(
    overridden: Option<String>,
    home: Option<String>,
) -> Option<String> {
    overridden.filter(|value| !value.is_empty()).or(home)
}
