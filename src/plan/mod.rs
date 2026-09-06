//! `rekall plan` -- the diff of an extraction.
//!
//! Report-only. It writes nothing except, with `--out`, the plan file
//! itself. What it produces is a reviewable ARTIFACT: the spans that would
//! be deleted, the files that would be written, and a FINGERPRINT of every
//! source it read.
//!
//! The fingerprint is not bookkeeping. Spans are `file:line-line`, and this
//! corpus is live prose a human edits -- so a plan reviewed at lunch and
//! applied after is a plan whose line numbers may have moved. V19 makes a
//! stale plan a refusal rather than a silent wrong edit to someone's memory.

use crate::{classify, statement};

/// Bumped when the plan format changes in a way an older `apply` would
/// misread. A plan file outlives the command that made it, so the version
/// is what lets a mismatch be REPORTED instead of misparsed.
pub const FORMAT: u32 = 1;

/// A source file as it stood when the plan was made.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Fingerprint {
    pub src: String,
    pub digest: String,
}

/// One statement's extraction.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Step {
    pub id: String,
    pub src: String,
    pub line_start: usize,
    pub line_end: usize,
    /// Verbatim, so the ledger row `apply` writes needs no second read.
    pub text: String,
    pub label: String,
    /// What would be written.
    pub artifact: String,
    /// NET tokens this extraction would reclaim: the statement, less the
    /// pointer left in its place (V39). Filled by the caller, which owns
    /// the `itok` delegation; `None` when the sibling is absent.
    ///
    /// In the plan FILE on purpose. A plan is a reviewable artifact, and
    /// "this move costs more than it saves" is exactly what review is for.
    /// Staleness is already handled: V19 refuses a plan whose corpus moved.
    #[serde(default)]
    pub net: Option<i64>,
    /// An EXISTING gate step this rule is already enforced by (V41).
    ///
    /// EMPTY is the default and means "write the inert stub". A NAME means
    /// the gate already checks this rule, so the extraction MOVES the
    /// check into the artifact and leaves the step as a caller -- one
    /// definition, many callers (V23), instead of the same rule stated in
    /// two runners.
    ///
    /// It is a field in the PLAN FILE and not a flag because only a human
    /// knows which step enforces which rule, a per-id flag does not
    /// survive past two ids, and the plan is already the artifact V19 has
    /// someone review before a byte moves. Editing it is not tampering:
    /// the fingerprint covers the CORPUS, not the plan's own fields.
    #[serde(default)]
    pub runner: String,
    /// What has to be wired for the artifact to do anything: a runner for
    /// `M` (V2), a trigger for `S` (V3). Named here so review can see the
    /// obligation before it exists rather than after.
    ///
    /// DERIVED from `runner`, and recomputed when a plan file is read
    /// (`retire_stale_wiring`). A human edits `runner`; if the sentence
    /// beside it kept whatever `plan` wrote first, the file would state
    /// one obligation and `apply` would perform another.
    #[serde(default)]
    pub wiring: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plan {
    pub format: u32,
    pub fingerprint: Vec<Fingerprint>,
    pub steps: Vec<Step>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// The id matched no statement in the corpus.
    Unknown(String),
    /// The id matched more than one statement.
    Ambiguous(String, Vec<String>),
    /// `U` carries no class, so there is nothing to extract INTO. Refusing
    /// beats guessing: V10 makes `U` a legitimate answer, and turning one
    /// into a rule would invent the verdict the classifier declined to give.
    Unclassified(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown(id) => write!(f, "no statement matches `{id}`"),
            Self::Ambiguous(id, ids) => write!(
                f,
                "`{id}` matches {} statements: {}. Use more characters.",
                ids.len(),
                ids.join(", ")
            ),
            Self::Unclassified(id) => write!(
                f,
                "`{id}` is unclassified (U) -- there is nothing to extract it into. \
                 Run `rekall show {id}` to see which signals fired, and reword the \
                 statement or leave it as prose"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// The digest of a whole file, as the fingerprint records it.
#[must_use]
pub fn digest_of(text: &str) -> String {
    format!("{:016x}", statement::digest(text.as_bytes()))
}

/// A filesystem-safe name for an artifact, from the statement's own words.
///
/// Derived from the text rather than the id, because a directory of
/// `95bae35.sh` files tells a reader nothing and the whole point of
/// extraction is that the artifact is legible where it lands.
#[must_use]
pub fn slug(text: &str) -> String {
    let words: Vec<String> = statement::normalize(text)
        .split_whitespace()
        .take(6)
        .map(clean_word)
        .filter(|word| !word.is_empty())
        .collect();
    if words.is_empty() {
        return "statement".to_string();
    }
    words.join("-")
}

fn clean_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Where an artifact lands.
///
/// BOTH kinds under `.rekall/`: one store, one reversal. A host-native
/// directory is not general -- Codex has no skills directory at all -- and
/// publishing into one is a SYMLINK laid beside this, never a second file
/// (V48).
fn destination(label: &str, slug: &str) -> String {
    if label.starts_with('M') {
        return format!(".rekall/rules/{slug}.sh");
    }
    format!(".rekall/skills/{slug}/SKILL.md")
}

/// What has to be wired for the artifact to matter.
///
/// Four cases now, and the third is V41: a rule the gate ALREADY enforces
/// is not wired, it is MOVED. Writing a second runner beside the step that
/// already checks it would leave one rule stated twice, which is the
/// defect extraction exists to remove, one layer below the prose.
///
/// The fourth is V57. A skill's trigger is not the only thing standing
/// between it and doing something: `src/apply:V52` turns the host's own
/// loading OFF, so with no `rekall hook` wired the artifact reaches no
/// reader at all. B13 is what that costs when it goes unsaid -- `apply`
/// wrote a guarded skill and `check` refused the same tree seconds later,
/// and the user met the disagreement as a red gate on their first run.
#[must_use]
pub fn wiring_for(
    label: &str,
    artifact: &str,
    runner: &str,
    delivered: bool,
) -> String {
    if !label.starts_with('M') {
        return skill_wiring(delivered);
    }
    if runner.is_empty() {
        return "add the script to the gate so it EXITS NONZERO on a violation (V2, V22)".to_string();
    }
    format!(
        "MOVE the body of gate step `{runner}` into {artifact}, then leave that step calling `sh {artifact}` -- ONE definition, many callers (V41, V23)"
    )
}

/// A skill's obligations, which are two where nothing delivers it.
///
/// Silent about the hook where one IS wired. A sentence telling someone to
/// do what they have already done is the noise people learn to read past,
/// and then read past on the line that mattered.
fn skill_wiring(delivered: bool) -> String {
    let trigger =
        "give the skill a trigger AND an explicit do-not-fire clause (V3, V4)";
    if delivered {
        return trigger.to_string();
    }
    format!(
        "{trigger}, AND wire `rekall hook` -- the head disables the host's own \
         loading, so nothing loads this skill until something delivers it (V57)"
    )
}

/// Recompute every step's wiring from its runner.
///
/// Called after a plan file is READ. The human edits `runner`; the
/// sentence beside it was written by `plan` before that edit, and a file
/// whose two halves disagree is worse than one that carries neither.
///
/// `delivered` is re-read too: a plan written before the hook was wired
/// should not still be telling you to wire it.
pub fn retire_stale_wiring(plan: &mut Plan, delivered: bool) {
    for step in &mut plan.steps {
        step.wiring =
            wiring_for(&step.label, &step.artifact, &step.runner, delivered);
    }
}

/// Turn one statement into a step.
fn step_for(
    found: &statement::Statement,
    weights: &classify::Weights,
    delivered: bool,
) -> Result<Step, Error> {
    let verdict = classify::classify(
        &statement::normalize(&found.text),
        found.form(),
        weights,
    );
    let label = verdict.label();
    if label == "U" {
        return Err(Error::Unclassified(found.id.clone()));
    }
    Ok(assemble(found, label, delivered))
}

fn assemble(
    found: &statement::Statement,
    label: String,
    delivered: bool,
) -> Step {
    let artifact = destination(&label, &slug(&found.text));
    let wiring = wiring_for(&label, &artifact, "", delivered);
    Step {
        runner: String::new(),
        id: found.id.clone(),
        src: found.path.clone(),
        line_start: found.line_start,
        line_end: found.line_end,
        text: found.text.clone(),
        label,
        artifact,
        wiring,
        // Filled by the caller, which owns the `itok` delegation (V8).
        net: None,
    }
}

/// Build a plan from statements already located in the corpus.
///
/// Takes the statements rather than looking them up, so the whole of this
/// module is pure: the same inputs produce the same plan on any machine,
/// which is what makes a plan file comparable in review.
pub fn build(
    chosen: &[statement::Statement],
    sources: &[(String, String)],
    weights: &classify::Weights,
    delivered: bool,
) -> Result<Plan, Error> {
    let mut steps = Vec::new();
    for found in chosen {
        steps.push(step_for(found, weights, delivered)?);
    }
    Ok(Plan {
        format: FORMAT,
        fingerprint: fingerprints_for(&steps, sources),
        steps,
    })
}

/// Fingerprint only the files the plan actually touches. Hashing the whole
/// corpus would make every plan stale the moment any unrelated note changed.
fn fingerprints_for(
    steps: &[Step],
    sources: &[(String, String)],
) -> Vec<Fingerprint> {
    let mut out: Vec<Fingerprint> = Vec::new();
    for (src, text) in sources {
        if !steps.iter().any(|step| &step.src == src) {
            continue;
        }
        if out.iter().any(|held| &held.src == src) {
            continue;
        }
        out.push(Fingerprint {
            src: src.clone(),
            digest: digest_of(text),
        });
    }
    out
}

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
                "this plan is format {found}, this build reads format {FORMAT}"
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

/// Human rendering. The JSON carries the SAME anatomy (V17).
#[must_use]
pub fn render_human(plan: &Plan) -> String {
    let mut out = String::new();
    for step in &plan.steps {
        out.push_str(&render_step(step));
    }
    out
}

fn render_step(step: &Step) -> String {
    format!(
        "{}  {}\n  delete  {}:{}-{}\n  write   {}\n  wire    {}\n  net     {}\n",
        step.id,
        step.label,
        step.src,
        step.line_start,
        step.line_end,
        step.artifact,
        step.wiring,
        net_line(step)
    )
}

/// The NET line, and it is a WARNING when the number is not positive.
///
/// V39: the pointer is always-on, so an extraction can cost more than it
/// saves -- and the statements most likely to lose are the sharpest, which
/// are one-liners. Saying so BEFORE the move is the whole point; `apply`
/// is the wrong place to learn it.
fn net_line(step: &Step) -> String {
    match step.net {
        None => "- (itok absent)".to_string(),
        Some(net) if net > 0 => format!("{net:+} tokens"),
        Some(net) => format!(
            "{net:+} tokens -- this extraction COSTS more than it saves (V39)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn statements(text: &str) -> Vec<statement::Statement> {
        statement::split(text, "CLAUDE.md")
    }

    fn sources(text: &str) -> Vec<(String, String)> {
        vec![("CLAUDE.md".to_string(), text.to_string())]
    }

    fn plan_of(text: &str) -> Result<Plan, Error> {
        build(
            &statements(text),
            &sources(text),
            &classify::Weights::default(),
            true,
        )
    }

    fn steps_of(text: &str) -> Vec<Step> {
        plan_of(text).map(|plan| plan.steps).unwrap_or_default()
    }

    /// V57: a skill's trigger is not the only thing between it and doing
    /// something. B13 -- `apply` wrote a guarded skill and `check` refused
    /// the same tree seconds later, and nothing said so at plan time.
    #[test]
    fn an_undelivered_skill_is_told_to_wire_the_hook() {
        let said = wiring_for("S2", ".rekall/skills/x/SKILL.md", "", false);
        assert!(said.contains("do-not-fire clause"), "{said}");
        assert!(said.contains("rekall hook"), "{said}");
    }

    /// And silent about it where one IS wired. A sentence telling someone
    /// to do what they have done is the noise people read past.
    #[test]
    fn a_delivered_skill_hears_only_about_its_trigger() {
        let said = wiring_for("S2", ".rekall/skills/x/SKILL.md", "", true);
        assert!(said.contains("do-not-fire clause"), "{said}");
        assert!(!said.contains("rekall hook"), "{said}");
    }

    /// An `M` rule is delivered by the GATE, by path, so the hook has
    /// nothing to do with it either way.
    #[test]
    fn a_rule_is_never_told_about_the_hook() {
        let said = wiring_for("M1", ".rekall/rules/x.sh", "", false);
        assert!(said.contains("add the script to the gate"), "{said}");
        assert!(!said.contains("rekall hook"), "{said}");
    }

    /// A plan file written before the hook was wired should not still be
    /// telling you to wire it -- the sentence is DERIVED, and reading the
    /// file recomputes it.
    #[test]
    fn reading_a_plan_recomputes_the_delivery_sentence() {
        let mut plan = plan_of("- when editing Rust, run clippy\n")
            .unwrap_or_else(|_| empty_plan());
        retire_stale_wiring(&mut plan, false);
        let before = plan.steps.first().map(|s| s.wiring.clone());
        retire_stale_wiring(&mut plan, true);
        let after = plan.steps.first().map(|s| s.wiring.clone());
        assert_ne!(before, after, "the sentence follows the wiring");
    }

    #[test]
    fn a_mechanical_statement_plans_a_script_and_a_runner() {
        let steps = steps_of("- never commit to `main`\n");
        let step = steps.first().cloned().unwrap_or_else(empty_step);
        assert_eq!(step.label, "M1");
        assert!(
            step.artifact.starts_with(".rekall/rules/"),
            "{}",
            step.artifact
        );
        assert!(step.wiring.contains("EXITS NONZERO"), "{}", step.wiring);
    }

    #[test]
    fn a_situational_statement_plans_a_skill_and_a_trigger() {
        let steps = steps_of("- when writing tests, prefer tables\n");
        let step = steps.first().cloned().unwrap_or_else(empty_step);
        assert!(
            step.artifact.starts_with(".rekall/skills/"),
            "{}",
            step.artifact
        );
        assert!(step.artifact.ends_with("/SKILL.md"), "{}", step.artifact);
        assert!(step.wiring.contains("do-not-fire"), "{}", step.wiring);
    }

    fn first_step(plan: &Plan) -> Step {
        plan.steps.first().cloned().unwrap_or_else(empty_step)
    }

    fn empty_step() -> Step {
        Step {
            id: String::new(),
            src: String::new(),
            line_start: 0,
            line_end: 0,
            text: String::new(),
            label: String::new(),
            artifact: String::new(),
            runner: String::new(),
            wiring: String::new(),
            net: None,
        }
    }

    /// V10 makes `U` a legitimate answer. Turning one into a rule would
    /// invent the verdict the classifier declined to give.
    #[test]
    fn an_unclassified_statement_cannot_be_planned() {
        let error = plan_of("- always prefer the simpler option\n").err();
        assert!(matches!(error, Some(Error::Unclassified(_))), "{error:?}");
    }

    #[test]
    fn the_refusal_points_at_show() {
        let message = plan_of("- always prefer the simpler option\n")
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(message.contains("rekall show"), "message was {message}");
    }

    /// A directory of `95bae35.sh` files tells a reader nothing. The whole
    /// point of extraction is that the artifact is legible where it lands.
    #[test]
    fn the_artifact_name_comes_from_the_statement_not_the_id() {
        let steps = steps_of("- never commit to `main`\n");
        let artifact = steps
            .first()
            .map(|step| step.artifact.clone())
            .unwrap_or_default();
        assert!(artifact.contains("never-commit-to-main"), "{artifact}");
    }

    #[test]
    fn a_slug_drops_punctuation_and_case() {
        assert_eq!(slug("- Never commit to `main`!"), "never-commit-to-main");
    }

    #[test]
    fn a_slug_is_bounded_so_a_long_rule_does_not_become_a_long_path() {
        let long =
            "- never ever under any circumstances at all commit to main branch";
        assert!(slug(long).split('-').count() <= 6, "{}", slug(long));
    }

    #[test]
    fn a_statement_of_only_punctuation_still_gets_a_name() {
        assert_eq!(slug("- ---"), "statement");
    }

    #[test]
    fn a_plan_carries_the_format_version() {
        assert_eq!(
            plan_of("- never commit to `main`\n").map(|p| p.format).ok(),
            Some(FORMAT)
        );
    }

    /// Only the files the plan TOUCHES are fingerprinted. Hashing the whole
    /// corpus would make every plan stale the moment any unrelated note
    /// changed.
    #[test]
    fn only_touched_files_are_fingerprinted() {
        let text = "- never commit to `main`\n";
        let mut with_extra = sources(text);
        with_extra.push(("NOTES.md".to_string(), "unrelated".to_string()));
        let names = fingerprinted(&statements(text), &with_extra);
        assert_eq!(names, vec!["CLAUDE.md".to_string()]);
    }

    fn fingerprinted(
        found: &[statement::Statement],
        sources: &[(String, String)],
    ) -> Vec<String> {
        build(found, sources, &classify::Weights::default(), true)
            .map(|plan| {
                plan.fingerprint.iter().map(|f| f.src.clone()).collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_plan_against_an_unchanged_corpus_is_fresh() {
        let text = "- never commit to `main`\n";
        let plan = plan_of(text).ok().unwrap_or_else(empty_plan);
        let now = vec![("CLAUDE.md".to_string(), Some(text.to_string()))];
        assert!(staleness(&plan, &now).is_empty());
    }

    fn empty_plan() -> Plan {
        Plan {
            format: FORMAT,
            fingerprint: Vec::new(),
            steps: Vec::new(),
        }
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

    #[test]
    fn human_output_names_the_delete_the_write_and_the_wiring() {
        let plan = plan_of("- never commit to `main`\n")
            .ok()
            .unwrap_or_else(empty_plan);
        let text = render_human(&plan);
        assert!(text.contains("delete  CLAUDE.md:1-1"), "{text}");
        assert!(text.contains("write   .rekall/rules/"), "{text}");
        assert!(text.contains("wire    "), "{text}");
    }

    #[test]
    fn a_plan_round_trips_through_toml() {
        let plan = plan_of("- never commit to `main`\n")
            .ok()
            .unwrap_or_else(empty_plan);
        let encoded = toml::to_string_pretty(&plan).unwrap_or_default();
        assert_eq!(toml::from_str::<Plan>(&encoded).ok(), Some(plan));
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
        let format = Stale::Format(99).to_string();
        assert!(format.contains("format 99"), "{format}");
    }

    #[test]
    fn the_ambiguity_message_names_every_candidate() {
        let error = Error::Ambiguous(
            "ab".to_string(),
            vec!["ab111".to_string(), "ab222".to_string()],
        );
        let message = error.to_string();
        assert!(message.contains("matches 2 statements"), "{message}");
        assert!(message.contains("ab111, ab222"), "{message}");
    }

    #[test]
    fn the_unknown_message_names_the_id() {
        assert!(
            Error::Unknown("zz".to_string())
                .to_string()
                .contains("`zz`")
        );
    }

    /// A file listed twice is fingerprinted ONCE. Two entries for one
    /// source would make a plan compare it against itself twice and report
    /// the same staleness twice.
    #[test]
    fn a_source_listed_twice_is_fingerprinted_once() {
        let text = "- never commit to `main`\n";
        let mut doubled = sources(text);
        doubled.push(("CLAUDE.md".to_string(), text.to_string()));
        let count = build(
            &statements(text),
            &doubled,
            &classify::Weights::default(),
            true,
        )
        .map(|plan| plan.fingerprint.len())
        .unwrap_or_default();
        assert_eq!(count, 1);
    }
    /// The fallback every test above relies on: a plan that failed to build
    /// yields an EMPTY step, so the assertion fails on content rather than
    /// panicking in a helper.
    #[test]
    fn the_empty_step_fallback_is_empty() {
        let step = empty_step();
        assert!(step.id.is_empty() && step.artifact.is_empty());
        assert_eq!(step.line_start, 0);
    }
    /// V41. A rule the gate already enforces is MOVED, not wired a second
    /// time -- and the sentence says which step the body comes from.
    #[test]
    fn a_named_runner_turns_the_wiring_into_a_move() {
        let wiring = wiring_for("M1", ".rekall/rules/ascii.sh", "ascii", true);
        assert!(wiring.contains("MOVE"), "{wiring}");
        assert!(wiring.contains("`ascii`"), "{wiring}");
        assert!(wiring.contains("sh .rekall/rules/ascii.sh"), "{wiring}");
    }

    #[test]
    fn an_empty_runner_still_asks_for_a_new_one() {
        let wiring = wiring_for("M1", ".rekall/rules/ascii.sh", "", true);
        assert!(wiring.contains("add the script to the gate"), "{wiring}");
    }

    /// A skill has no gate step to move, so its obligation is unchanged.
    #[test]
    fn a_skill_wiring_ignores_a_runner() {
        let named =
            wiring_for("S1", ".rekall/skills/x/SKILL.md", "ascii", true);
        let bare = wiring_for("S1", ".rekall/skills/x/SKILL.md", "", true);
        assert_eq!(named, bare);
    }

    /// V41's staleness case: the sentence in the file was written BEFORE
    /// the human named a runner, so reading it back has to recompute it.
    /// A plan whose two halves disagree would state one obligation and
    /// perform another.
    #[test]
    fn reading_a_plan_recomputes_the_wiring_from_the_runner() {
        let mut plan = Plan {
            format: FORMAT,
            fingerprint: Vec::new(),
            steps: vec![Step {
                label: "M1".to_string(),
                artifact: ".rekall/rules/ascii.sh".to_string(),
                runner: "ascii".to_string(),
                wiring: "add the script to the gate".to_string(),
                ..empty_step()
            }],
        };
        retire_stale_wiring(&mut plan, true);
        let step = first_step(&plan);
        assert!(step.wiring.contains("MOVE"), "{step:?}");
    }

    /// The field survives the file. A runner a human wrote and a parser
    /// dropped is a decision silently discarded.
    #[test]
    fn a_runner_round_trips_through_the_plan_file() {
        let plan = Plan {
            format: FORMAT,
            fingerprint: Vec::new(),
            steps: vec![Step {
                runner: "ascii".to_string(),
                ..empty_step()
            }],
        };
        let encoded = toml::to_string(&plan).unwrap_or_default();
        let back: Plan = toml::from_str(&encoded).unwrap_or(Plan {
            format: FORMAT,
            fingerprint: Vec::new(),
            steps: Vec::new(),
        });
        assert_eq!(first_step(&back).runner, "ascii");
    }
}
