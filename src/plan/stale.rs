//! Whether a plan still describes the corpus it was made from (V19).
//!
//! Split out of `plan` when the module reached its size cap and a slug fix
//! (`src/apply:T50`) had nowhere to land. Staleness has one reader outside
//! this node -- `apply <PLAN>` refuses on it -- and nothing in `build` or
//! the renderers calls it, so it is the seam that cuts nothing.
//!
//! A FILE and not a module directory, for the reason `src/check` gives for
//! `head.rs`: a new federated node costs more nav than it relieves, so the
//! rules for this file stay in `src/plan/SPEC.md`.

use super::{FORMAT, Fingerprint, Plan, digest_of};

/// Why a plan cannot be applied.
#[derive(Debug, PartialEq, Eq)]
pub enum Stale {
    /// A source file changed since the plan was made. Its spans may now
    /// point at different lines, so applying it would delete the wrong text.
    Changed(String),
    /// A source file the plan depends on is gone.
    Vanished(String),
    /// The plan was written by a different version of this format.
    Format(u32),
}

impl std::fmt::Display for Stale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Changed(src) => write!(
                f,
                "{src} changed since this plan was made -- its line spans may now \
                 point at different text. Re-run `rekall plan`"
            ),
            Self::Vanished(src) => {
                write!(f, "{src} no longer exists. Re-run `rekall plan`")
            }
            Self::Format(found) => write!(
                f,
                "this plan is format {found}, this build reads format \
                 {FORMAT}. Re-run `rekall plan`"
            ),
        }
    }
}

/// Check a plan against the corpus as it is NOW (V19).
///
/// `current` maps each source name to its text today, or `None` if it is
/// gone. Returns every reason the plan is stale rather than the first: a
/// re-plan fixes all of them at once, so reporting one at a time would
/// just make the user run it repeatedly.
#[must_use]
pub fn staleness(
    plan: &Plan,
    current: &[(String, Option<String>)],
) -> Vec<Stale> {
    let mut out = Vec::new();
    if plan.format != FORMAT {
        out.push(Stale::Format(plan.format));
    }
    for held in &plan.fingerprint {
        out.extend(check_one(held, current));
    }
    out
}

fn check_one(
    held: &Fingerprint,
    current: &[(String, Option<String>)],
) -> Option<Stale> {
    let found = current.iter().find(|(src, _)| src == &held.src);
    match found.map(|(_, text)| text) {
        None | Some(None) => Some(Stale::Vanished(held.src.clone())),
        Some(Some(text)) if digest_of(text) != held.digest => {
            Some(Stale::Changed(held.src.clone()))
        }
        Some(Some(_)) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{classify, plan, statement};

    fn plan_of(text: &str) -> Result<Plan, plan::Error> {
        plan::build(
            &statement::split(text, "CLAUDE.md"),
            &[("CLAUDE.md".to_string(), text.to_string())],
            &classify::Weights::default(),
            true,
        )
    }

    fn empty_plan() -> Plan {
        Plan {
            format: FORMAT,
            ..Plan::default()
        }
    }

    #[test]
    fn a_plan_against_an_unchanged_corpus_is_fresh() {
        let text = "- never commit to `main`\n";
        let plan = plan_of(text).ok().unwrap_or_else(empty_plan);
        let now = vec![("CLAUDE.md".to_string(), Some(text.to_string()))];
        assert!(staleness(&plan, &now).is_empty());
    }

    /// V19, the whole reason the fingerprint exists: two lines inserted
    /// above shift every span below, and applying the old plan would delete
    /// the wrong text out of someone's memory.
    #[test]
    fn a_changed_source_makes_the_plan_stale() {
        let text = "- never commit to `main`\n";
        let plan = plan_of(text).ok().unwrap_or_else(empty_plan);
        let edited = format!("# a new heading\n\nsome new prose\n\n{text}");
        let now = vec![("CLAUDE.md".to_string(), Some(edited))];
        assert_eq!(
            staleness(&plan, &now),
            vec![Stale::Changed("CLAUDE.md".to_string())]
        );
    }

    #[test]
    fn a_vanished_source_makes_the_plan_stale() {
        let text = "- never commit to `main`\n";
        let plan = plan_of(text).ok().unwrap_or_else(empty_plan);
        let now = vec![("CLAUDE.md".to_string(), None)];
        assert_eq!(
            staleness(&plan, &now),
            vec![Stale::Vanished("CLAUDE.md".to_string())]
        );
    }

    #[test]
    fn a_source_missing_from_the_corpus_entirely_is_vanished() {
        let text = "- never commit to `main`\n";
        let plan = plan_of(text).ok().unwrap_or_else(empty_plan);
        assert_eq!(
            staleness(&plan, &[]),
            vec![Stale::Vanished("CLAUDE.md".to_string())]
        );
    }

    #[test]
    fn a_plan_from_another_format_is_stale() {
        let mut plan = empty_plan();
        plan.format = FORMAT.saturating_add(1);
        assert!(
            staleness(&plan, &[])
                .contains(&Stale::Format(FORMAT.saturating_add(1)))
        );
    }

    /// Every reason at once. A re-plan fixes all of them, so reporting one
    /// at a time would just make the user run it repeatedly.
    #[test]
    fn every_reason_a_plan_is_stale_is_reported_together() {
        let text = "- never commit to `main`\n";
        let mut plan = plan_of(text).ok().unwrap_or_else(empty_plan);
        plan.format = 99;
        let now =
            vec![("CLAUDE.md".to_string(), Some("different".to_string()))];
        assert_eq!(staleness(&plan, &now).len(), 2);
    }

    #[test]
    fn whitespace_only_edits_still_count_as_a_change() {
        let text = "- never commit to `main`\n";
        let plan = plan_of(text).ok().unwrap_or_else(empty_plan);
        let now = vec![("CLAUDE.md".to_string(), Some(format!("{text}\n")))];
        assert_eq!(staleness(&plan, &now).len(), 1);
    }

    /// The messages a user meets when a plan will not apply. Never
    /// formatted until now, which is the usual fate of error text.
    #[test]
    fn the_staleness_reasons_say_what_to_do() {
        let changed = Stale::Changed("CLAUDE.md".to_string()).to_string();
        assert!(changed.contains("changed since this plan"), "{changed}");
        assert!(changed.contains("rekall plan"), "{changed}");
        let gone = Stale::Vanished("CLAUDE.md".to_string()).to_string();
        assert!(gone.contains("no longer exists"), "{gone}");
        assert!(gone.contains("rekall plan"), "{gone}");
        let format = Stale::Format(99).to_string();
        assert!(format.contains("format 99"), "{format}");
        assert!(format.contains("rekall plan"), "{format}");
    }
}
