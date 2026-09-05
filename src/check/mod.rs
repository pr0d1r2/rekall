//! `rekall check` -- the gate.
//!
//! It answers one question: is every extraction still HONEST? An artifact
//! that vanished, a rule whose runner was never written (V2), a skill with
//! no trigger (V3) or no explicit refusal clause (V4), a statement quietly
//! pasted back beside the artifact it was moved into (V1), a file nothing
//! in the ledger claims.
//!
//! CPU-only, no key, no network (V6). A gate that needs a model runs
//! nowhere it is needed, and this one is meant to run in `hk` on every
//! commit and in a locked-down CI.
//!
//! The judgment is a PURE function over bytes already read. The caller
//! does the filesystem; everything that decides what counts as drift is
//! testable without one.

use crate::{apply, issue, ledger, revert, trigger};

/// Stable machine names for each kind of drift.
///
/// `--format json` carries the kind as its OWN field so an agent can
/// branch on it without parsing the sentence (V17), and the sentence can
/// then be rewritten for humans without breaking a consumer.
pub const MISSING_SOURCE: &str = "missing-source";
pub const POINTER: &str = "pointer";
pub const STATEMENT_RESTORED: &str = "statement-restored";
pub const MISSING_ARTIFACT: &str = "missing-artifact";
pub const NO_RUNNER: &str = "no-runner";
pub const NO_TRIGGER: &str = "no-trigger";
pub const NO_REFUSAL_CLAUSE: &str = "no-refusal-clause";
pub const ORPHAN_ARTIFACT: &str = "orphan-artifact";
pub const BAD_TRIGGER_BLOCK: &str = "bad-trigger-block";
pub const LADDER: &str = "ladder";
pub const LOOSE_QUOTE: &str = "loose-quote";
pub const NO_PAYLOAD: &str = "no-payload";
pub const NO_HEAD: &str = "no-head";
pub const NO_GUARD: &str = "no-guard";
pub const UNDELIVERED: &str = "undelivered";
/// `src/apply:V48` calls a dangling published link an ORPHAN, and it is
/// one -- but it gets its own KIND rather than reusing `orphan-artifact`.
/// That one means a file no row claims; this means a POINTER to a file that
/// is not there, and the fixes differ: delete the link, versus account for
/// the artifact. A consumer switching on `kind` should not have to read the
/// sentence to tell them apart.
pub const DANGLING_LINK: &str = "dangling-link";

/// The one NOTE kind: an issued extraction whose local copy still
/// stands. `src/issue:V54` says that overlap is INTENDED.
pub const HANDOVER_OPEN: &str = "handover-open";

/// One thing wrong.
///
/// A struct rather than an enum because every finding answers the same
/// three questions -- what kind, which id, which file -- and an agent
/// should read those as fields rather than reconstruct them from prose.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Drift {
    pub kind: &'static str,
    pub id: String,
    pub path: String,
    /// What a human reads. It NAMES THE FIX, not only the breach (V28) --
    /// a gate that says what is wrong and not what to do about it is a
    /// gate people learn to route around.
    pub said: String,
}

pub(super) fn drift(
    kind: &'static str,
    id: &str,
    path: &str,
    said: String,
) -> Drift {
    Drift {
        kind,
        id: id.to_string(),
        path: path.to_string(),
        said,
    }
}

/// One extracted row, with the bytes the gate needs to judge it.
///
/// `None` means the file could not be read, which is itself a finding --
/// not a reason to skip the row. A check that quietly passes over what it
/// could not open is the silent skip V26 forbids.
pub struct Seen<'a> {
    pub row: &'a ledger::Extracted,
    pub source: Option<&'a str>,
    pub artifact: Option<&'a str>,
}

mod delivery;
mod head;
mod notes;
pub use delivery::{dangling, undelivered};
pub use notes::{Note, notes, render_notes};

/// Every way the corpus has drifted from what the ledger claims.
///
/// Deterministic and order-stable: rows in ledger order, then orphans in
/// the order the walk found them. A gate whose output reshuffles between
/// runs is a gate whose diff nobody can read.
#[must_use]
pub fn audit(seen: &[Seen<'_>], on_disk: &[String]) -> Vec<Drift> {
    let mut out: Vec<Drift> = Vec::new();
    for one in seen {
        out.extend(source_drift(one));
        out.extend(artifact_drift(one));
    }
    out.extend(orphans(seen, on_disk));
    out
}

/// What the SOURCE file should look like after an extraction: the pointer
/// standing where the statement was, and the statement itself gone.
fn source_drift(seen: &Seen<'_>) -> Vec<Drift> {
    let row = seen.row;
    let Some(text) = seen.source else {
        return vec![drift(
            MISSING_SOURCE,
            &row.id,
            &row.src,
            said_missing_source(row),
        )];
    };
    let mut out = pointer_drift(seen, text);
    out.extend(copy_drift(seen, text));
    out
}

fn said_missing_source(row: &ledger::Extracted) -> String {
    format!(
        "{} is named by the ledger but cannot be read. Restore the file, or run \
         `rekall revert {}` from a checkout that still has it",
        row.src, row.id
    )
}

fn pointer_drift(seen: &Seen<'_>, text: &str) -> Vec<Drift> {
    let Err(fault) = revert::locate(text, seen.row) else {
        return Vec::new();
    };
    vec![drift(
        POINTER,
        &seen.row.id,
        &seen.row.src,
        fault.to_string(),
    )]
}

/// V1: extraction is a MOVE, not a copy. The statement standing in the
/// source WHILE its artifact exists is two hand-maintained statements of
/// one rule -- and the context cost the extraction was supposed to pay is
/// being paid again on every turn.
fn copy_drift(seen: &Seen<'_>, text: &str) -> Vec<Drift> {
    let row = seen.row;
    if !text.contains(row.text.trim()) {
        return Vec::new();
    }
    vec![drift(
        STATEMENT_RESTORED,
        &row.id,
        &row.src,
        format!(
            "{} still holds the statement extracted to {}, so the rule is stated \
             TWICE and the context cost is being paid again. Delete it from {}, \
             or run `rekall revert {}` to undo the extraction properly",
            row.src, row.artifact, row.src, row.id
        ),
    )]
}

/// What the ARTIFACT must carry, which depends on the class the extraction
/// was made under.
fn artifact_drift(seen: &Seen<'_>) -> Vec<Drift> {
    let row = seen.row;
    let Some(text) = seen.artifact else {
        return absent_artifact(row);
    };
    let mut out = payload_drift(row, text);
    out.extend(by_class(row, text));
    out
}

fn by_class(row: &ledger::Extracted, text: &str) -> Vec<Drift> {
    if row.label.starts_with('M') {
        return runner_drift(row, text);
    }
    let mut out = head::head_drift(row, text);
    out.extend(head::guard_drift(row, text));
    out.extend(skill_drift(row, text));
    out
}

/// An artifact that is not there.
///
/// RETIRED is the end state, not drift: the rule lives in the registry the
/// row names, and this repository is done with it (`src/issue:V54`). The
/// ledger that keeps the audit trail cannot be the ledger that fails the
/// gate over the trail it kept. Every other absence is still a finding.
fn absent_artifact(row: &ledger::Extracted) -> Vec<Drift> {
    if issue::stage(row, false) == issue::Stage::Retired {
        return Vec::new();
    }
    vec![drift(
        MISSING_ARTIFACT,
        &row.id,
        &row.artifact,
        said_missing_artifact(row),
    )]
}

/// V43: an artifact with no PAYLOAD mark has nothing a reader can take but
/// the whole file.
///
/// For an `S` that is measurable and was measured: 318 tokens delivered to
/// say 36, at the fire point, which is the one cost this crate exists to
/// remove. For an `M` the mark earns less today -- `hook` injects what the
/// RUNNER said -- but the rule is one rule, and an artifact that carries
/// its statement unmarked is one nobody can quote from later.
fn payload_drift(row: &ledger::Extracted, text: &str) -> Vec<Drift> {
    if apply::artifact::payload_of(text).is_some() {
        return Vec::new();
    }
    vec![drift(
        NO_PAYLOAD,
        &row.id,
        &row.artifact,
        said_unmarked(row),
    )]
}

fn said_unmarked(row: &ledger::Extracted) -> String {
    let (open, close) = marks_for(&row.artifact);
    format!(
        "{} does not mark its PAYLOAD, so the only thing a reader can take is the whole \
         file -- scaffold included, at the fire point (V43). Put `{open}` and `{close}` \
         around the quoted statement, or regenerate it with `rekall revert {}` then \
         `rekall apply {}`",
        row.artifact, row.id, row.id
    )
}

/// The mark spelled for the file it goes in: a shell comment in a runner,
/// an HTML comment in a skill. Naming the wrong one in the advice sends
/// someone to paste a line their file will execute.
fn marks_for(artifact: &str) -> (&'static str, &'static str) {
    let markdown = artifact.ends_with(".md");
    let at = usize::from(markdown);
    (
        apply::artifact::PAYLOAD_OPEN
            .get(at)
            .copied()
            .unwrap_or_default(),
        apply::artifact::PAYLOAD_CLOSE
            .get(at)
            .copied()
            .unwrap_or_default(),
    )
}

fn said_missing_artifact(row: &ledger::Extracted) -> String {
    format!(
        "{} is gone, but the ledger says {} was extracted into it. Write it back, \
         or run `rekall revert {}` to put the statement in {}",
        row.artifact, row.id, row.id, row.src
    )
}

/// V2: a rule with no runner gates nothing. The generated script announces
/// itself unimplemented and exits nonzero, so it is a PLACEHOLDER until
/// someone writes the check -- and a placeholder wired into a gate is the
/// wish V22 forbids.
fn runner_drift(row: &ledger::Extracted, text: &str) -> Vec<Drift> {
    if let Some(loose) = loose_quote(row, text) {
        return vec![loose];
    }
    if !text.trim().is_empty() && !text.contains(apply::artifact::UNIMPLEMENTED)
    {
        return Vec::new();
    }
    vec![drift(
        NO_RUNNER,
        &row.id,
        &row.artifact,
        said_placeholder(row),
    )]
}

fn said_placeholder(row: &ledger::Extracted) -> String {
    format!(
        "{} is still the generated placeholder, so the rule extracted from {} \
         gates nothing (V2). Write the check in that script and remove the \
         line that says it is unimplemented",
        row.artifact, row.src
    )
}

/// V42: a line of the statement that reaches the script UNCOMMENTED is a
/// COMMAND.
///
/// Judged against the LEDGER's verbatim text rather than by parsing the
/// script, because the ledger is what the statement actually said (V9) and
/// a parser would be guessing where the quote ends. The teeth live here
/// rather than in the template on purpose: the template is edited by the
/// same hand that will forget (V22, B6).
fn loose_quote(row: &ledger::Extracted, text: &str) -> Option<Drift> {
    let loose = row.text.lines().map(str::trim).find(|line| {
        !line.is_empty() && text.lines().any(|written| written.trim() == *line)
    })?;
    Some(drift(
        LOOSE_QUOTE,
        &row.id,
        &row.artifact,
        format!(
            "{} quotes the statement from {} on a line the shell would RUN: `{loose}`. \
             Every line of a quoted statement is a comment (V42) -- put `# ` in front \
             of it, or regenerate the artifact with `rekall revert {}` then `rekall apply {}`",
            row.artifact, row.src, row.id, row.id
        ),
    ))
}

/// V3, V4 and V29. The BLOCK is the trigger; the prose beside it is not
/// read, so this reads the block or reports that it cannot.
///
/// An `S3` is judged by the OPPOSITE rule. Its trigger is semantic and V5
/// forbids the model that would notice it, so an empty block is CORRECT
/// there -- and a full one is a LADDER defect, a statement filed at `3`
/// that turns out to admit a machine trigger after all (V11).
fn skill_drift(row: &ledger::Extracted, text: &str) -> Vec<Drift> {
    if row.label == "S3" {
        return semantic_drift(row, text);
    }
    let mut out = block_drift(row, text, apply::artifact::FIRES, NO_TRIGGER);
    out.extend(block_drift(
        row,
        text,
        apply::artifact::NOT_FIRES,
        NO_REFUSAL_CLAUSE,
    ));
    out
}

/// One heading's block: unreadable is its OWN finding, empty is the
/// obligation still outstanding.
///
/// The two are separated because they send the reader to different places
/// -- a block that will not parse is a typo, an empty one is work nobody
/// has done yet -- and V28 asks the message to name the fix rather than
/// the breach.
fn block_drift(
    row: &ledger::Extracted,
    text: &str,
    heading: &str,
    empty_kind: &'static str,
) -> Vec<Drift> {
    let empty =
        || drift(empty_kind, &row.id, &row.artifact, said_empty(row, heading));
    match trigger::parse_block(text, heading) {
        Ok(held) if held.is_empty() => vec![empty()],
        Ok(_) => Vec::new(),
        // No block at all is NO TRIGGER, not a broken one. The block IS
        // the trigger (V29), so its absence is the obligation outstanding
        // -- and that is the message a skill written before this format
        // should get, rather than being told its syntax is wrong.
        Err(trigger::Fault::Missing) => vec![empty()],
        Err(fault) => vec![drift(
            BAD_TRIGGER_BLOCK,
            &row.id,
            &row.artifact,
            said_bad(row, heading, &fault),
        )],
    }
}

/// V29's other direction: an `S3` that grew a real trigger is misfiled.
fn semantic_drift(row: &ledger::Extracted, text: &str) -> Vec<Drift> {
    let filled = [apply::artifact::FIRES, apply::artifact::NOT_FIRES]
        .iter()
        .any(|heading| {
            trigger::parse_block(text, heading)
                .is_ok_and(|held| !held.is_empty())
        });
    if !filled {
        return Vec::new();
    }
    vec![drift(LADDER, &row.id, &row.artifact, said_ladder(row))]
}

fn said_empty(row: &ledger::Extracted, heading: &str) -> String {
    format!(
        "{} has an empty or absent `{}` block, so nothing can match it -- and an \
         unfilled trigger leaves the statement as always-on prose, which is what \
         it was extracted FROM (V3, V4). Fill the {} block under it: `tool` for \
         exact names, `path` for globs, `word` for literals",
        row.artifact,
        heading,
        trigger::FENCE
    )
}

fn said_bad(
    row: &ledger::Extracted,
    heading: &str,
    fault: &trigger::Fault,
) -> String {
    format!(
        "{} has a `{heading}` block this crate REFUSES to read rather than half \
         understand (V29) -- {fault}",
        row.artifact
    )
}

/// V11 names both directions: a `1` gone dead is a classifier defect, a
/// `3` that fires is a LADDER defect. This is the second one, caught at
/// the only place it is visible.
fn said_ladder(row: &ledger::Extracted) -> String {
    format!(
        "{} is filed `S3` -- semantic, so V5 forbids the matcher that would read \
         it and it can never fire -- yet it carries a real trigger block. One of \
         the two is wrong: reclassify the statement to `S1` or `S2` by rewording \
         it and re-extracting, or empty the block",
        row.artifact
    )
}

/// An artifact no ledger row claims.
///
/// The ledger is the record of what was extracted, so a file sitting in an
/// artifact directory with no row behind it is either a revert that half
/// finished or a rule someone hand-wrote where a generated one goes.
/// Either way nothing records where it came from, which is the state V9
/// exists to prevent.
fn orphans(seen: &[Seen<'_>], on_disk: &[String]) -> Vec<Drift> {
    on_disk
        .iter()
        .filter(|path| !seen.iter().any(|one| &&one.row.artifact == path))
        .map(|path| drift(ORPHAN_ARTIFACT, "", path, said_orphan(path)))
        .collect()
}

fn said_orphan(path: &str) -> String {
    format!(
        "{path} is an artifact no ledger row claims, so nothing records which \
         statement it came from or how to undo it (V9). Delete it, or extract \
         the statement properly with `rekall apply`"
    )
}

/// Human output. SILENT when clean (V28): output that always appears is
/// output nobody reads, and the one real failure then hides in noise
/// everyone learned to scroll past.
#[must_use]
pub fn render_human(found: &[Drift]) -> String {
    let mut out = String::new();
    for one in found {
        out.push_str(&format!("{:<19} {}\n", one.kind, one.said));
    }
    out
}

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    /// The clean case. Everything the ledger claims is true, so the gate
    /// says NOTHING -- success is silence (V28).
    #[test]
    fn an_honest_extraction_reports_no_drift() {
        let held = row("M1");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(RUNNER),
        }];
        let found = audit(&seen, std::slice::from_ref(&held.artifact));
        assert!(found.is_empty(), "{found:?}");
        assert_eq!(render_human(&found), "");
    }

    /// V2: the generated script announces itself unimplemented, so it is a
    /// placeholder. A rule with no runner gates nothing.
    /// The artifact `apply` would actually write for a row -- generated
    /// here rather than hand-copied, so the gate is tested against the
    /// bytes it will really meet.
    fn generated(held: &ledger::Extracted) -> String {
        apply::artifact::text(&crate::plan::Step {
            id: held.id.clone(),
            src: held.src.clone(),
            line_start: held.line_start,
            line_end: held.line_end,
            text: held.text.clone(),
            label: held.label.clone(),
            artifact: held.artifact.clone(),
            runner: String::new(),
            wiring: String::new(),
            net: None,
        })
    }

    #[test]
    fn an_unimplemented_runner_is_drift() {
        let held = row("M1");
        let source = extracted_source(&held);
        let placeholder = generated(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(&placeholder),
        }];
        // The GENERATED artifact marks its payload (V43), so the only
        // thing wrong with it is the check nobody has written yet.
        assert_eq!(kinds(&audit(&seen, &[])), vec![NO_RUNNER]);
    }

    /// THE PAIRING THAT MATTERS. What `apply` writes for an `S` row is
    /// exactly what this gate must reject -- if the template ever landed
    /// in a state the gate accepts, every extraction would pass having
    /// done none of the work V3 and V4 require.
    #[test]
    fn the_skill_apply_generates_does_not_pass_the_gate() {
        let held = row("S2");
        let source = extracted_source(&held);
        let template = generated(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(&template),
        }];
        assert_eq!(
            kinds(&audit(&seen, &[])),
            vec![NO_TRIGGER, NO_REFUSAL_CLAUSE]
        );
    }

    #[test]
    fn an_empty_runner_is_drift_too() {
        let held = row("M2");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some("   \n"),
        }];
        // An empty file marks no payload either, and says both (V43).
        assert_eq!(kinds(&audit(&seen, &[])), vec![NO_PAYLOAD, NO_RUNNER]);
    }

    /// V4 alone. A skill can name what fires it and still say nothing
    /// about what does not -- the omission a positive description hides.
    #[test]
    fn a_trigger_without_a_refusal_clause_is_still_drift() {
        let held = row("S1");
        let source = extracted_source(&held);
        let half = blocks("tool = [\"Edit\"]", "");
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(&half),
        }];
        assert_eq!(kinds(&audit(&seen, &[])), vec![NO_REFUSAL_CLAUSE]);
    }

    /// B10: every `S` artifact this crate wrote before V52 was
    /// model-invocable, so the host loaded it on its own judgement and the
    /// do-not-fire clause was bypassed on the path this crate does not
    /// control. The gate names the verb that repairs it.
    #[test]
    fn a_skill_whose_head_does_not_disable_host_loading_is_drift() {
        let held = row("S1");
        let source = extracted_source(&held);
        let bare = SKILL.replace("disable-model-invocation: true\n", "");
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(&bare),
        }];
        let found = audit(&seen, &[]);
        assert_eq!(kinds(&found), vec![NO_GUARD]);
        assert!(found.iter().any(|one| one.said.contains("rekall issue")));
    }

    /// An artifact with NO head gets `no-head` and NOT also `no-guard`.
    /// Two findings for one missing block would name the same fix twice.
    #[test]
    fn a_headless_skill_is_named_once() {
        let held = row("S1");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some("no head at all\n"),
        }];
        assert!(!kinds(&audit(&seen, &[])).contains(&NO_GUARD));
    }

    /// `src/issue:V54`: a RETIRED row's artifact is gone on purpose. The
    /// rule lives in the registry the row names, and the ledger that keeps
    /// the audit trail cannot be the ledger that fails the gate over it.
    #[test]
    fn a_retired_extraction_is_not_a_missing_artifact() {
        let mut held = row("S1");
        held.issued_to = "../set-and-setting".to_string();
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: None,
        }];
        assert!(audit(&seen, &[]).is_empty());
    }

    /// And a row that was NOT issued is still missing its artifact. The
    /// exemption is the stage, not the absence.
    #[test]
    fn an_unissued_row_without_its_artifact_is_still_drift() {
        let held = row("S1");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: None,
        }];
        assert_eq!(kinds(&audit(&seen, &[])), vec![MISSING_ARTIFACT]);
    }

    #[test]
    fn a_filled_skill_passes() {
        let held = row("S1");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(SKILL),
        }];
        assert!(audit(&seen, &[]).is_empty(), "{:?}", audit(&seen, &[]));
    }

    /// PROSE IS NOT A TRIGGER (V29). A skill written before this format --
    /// or by someone who filled in the words and not the block -- reports
    /// NO TRIGGER, which is accurate: nothing can match it.
    #[test]
    fn prose_under_the_heading_is_not_a_trigger() {
        let held = row("S1");
        let source = extracted_source(&held);
        let prose = prose_skill();
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(&prose),
        }];
        assert_eq!(
            kinds(&audit(&seen, &[])),
            vec![NO_TRIGGER, NO_REFUSAL_CLAUSE]
        );
    }

    /// A block that will not parse is its OWN finding, separate from an
    /// empty one: a typo and unfinished work send the reader elsewhere.
    #[test]
    fn a_block_that_will_not_parse_is_reported_as_such() {
        let held = row("S2");
        let source = extracted_source(&held);
        let broken = blocks("tolo = [\"Edit\"]", "word = [\"x\"]");
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(&broken),
        }];
        let found = audit(&seen, &[]);
        assert_eq!(kinds(&found), vec![BAD_TRIGGER_BLOCK]);
        assert!(said_of(&found).contains("REFUSES"), "{found:?}");
    }

    fn said_of(found: &[Drift]) -> String {
        found
            .first()
            .map(|one| one.said.clone())
            .unwrap_or_default()
    }

    /// V29: an `S3` trigger is SEMANTIC, V5 forbids the matcher that would
    /// read it, so an EMPTY block is correct and the skill never fires.
    /// That is the ladder being honest, not a gap.
    #[test]
    fn an_s3_with_empty_blocks_passes() {
        let held = row("S3");
        let source = extracted_source(&held);
        let empty = blocks("", "");
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(&empty),
        }];
        assert!(audit(&seen, &[]).is_empty(), "{:?}", audit(&seen, &[]));
    }

    /// The OTHER direction, and the one V11 predicts: a `3` that turns out
    /// to admit a real trigger is a LADDER defect, visible only here.
    #[test]
    fn an_s3_carrying_a_real_trigger_is_a_ladder_defect() {
        let held = row("S3");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(SKILL),
        }];
        let found = audit(&seen, &[]);
        assert_eq!(kinds(&found), vec![LADDER]);
        assert!(
            found
                .first()
                .is_some_and(|one| one.said.contains("reclassify")),
            "{found:?}"
        );
    }

    #[test]
    fn a_missing_artifact_is_drift() {
        let held = row("M1");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: None,
        }];
        let found = audit(&seen, &[]);
        assert_eq!(kinds(&found), vec![MISSING_ARTIFACT]);
        assert!(
            found
                .first()
                .is_some_and(|one| one.said.contains("rekall revert")),
            "{found:?}"
        );
    }

    #[test]
    fn a_source_that_cannot_be_read_is_drift() {
        let held = row("M1");
        let seen = vec![Seen {
            row: &held,
            source: None,
            artifact: Some(RUNNER),
        }];
        assert_eq!(kinds(&audit(&seen, &[])), vec![MISSING_SOURCE]);
    }

    /// The pointer is gone from the source. Reported through the SAME
    /// fault `revert` raises, so the two verbs cannot disagree about where
    /// a statement lives.
    #[test]
    fn a_missing_pointer_is_drift() {
        let held = row("M1");
        let seen = vec![Seen {
            row: &held,
            source: Some("# Rules\n\n- other prose\n"),
            artifact: Some(RUNNER),
        }];
        let found = audit(&seen, &[]);
        assert_eq!(kinds(&found), vec![POINTER]);
        assert!(
            found
                .first()
                .is_some_and(|one| one.said.contains("edited by hand")),
            "{found:?}"
        );
    }

    /// V1: the statement pasted BACK beside the artifact it was moved
    /// into. Two hand-maintained statements of one rule, and the context
    /// cost being paid again on every turn.
    #[test]
    fn a_statement_restored_beside_its_artifact_is_drift() {
        let held = row("M1");
        let source = format!(
            "# Rules\n\n{}\n\n- never commit to `main`\n",
            apply::pointer_of(&held.id)
        );
        let found = audit(&[with_runner(&held, &source)], &[]);
        assert_eq!(kinds(&found), vec![STATEMENT_RESTORED]);
        assert!(
            found.first().is_some_and(|one| one.said.contains("TWICE")),
            "{found:?}"
        );
    }

    /// A row whose runner is real, so only the SOURCE side is under test.
    fn with_runner<'a>(
        held: &'a ledger::Extracted,
        source: &'a str,
    ) -> Seen<'a> {
        Seen {
            row: held,
            source: Some(source),
            artifact: Some(RUNNER),
        }
    }

    /// An artifact with no ledger row behind it. Nothing records which
    /// statement it came from, so nothing can undo it (V9).
    #[test]
    fn an_artifact_no_row_claims_is_an_orphan() {
        let held = row("M1");
        let source = extracted_source(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(RUNNER),
        }];
        let on_disk =
            vec![held.artifact.clone(), ".rekall/rules/stray.sh".to_string()];
        let found = audit(&seen, &on_disk);
        assert_eq!(kinds(&found), vec![ORPHAN_ARTIFACT]);
        assert_eq!(
            found.first().map(|one| one.path.clone()),
            Some(".rekall/rules/stray.sh".to_string())
        );
    }

    /// An empty ledger with an empty disk is the state of a repo that has
    /// extracted nothing. It is CLEAN, not suspicious -- a gate that fails
    /// before the tool has been used is a gate nobody adopts.
    #[test]
    fn nothing_extracted_is_clean() {
        assert!(audit(&[], &[]).is_empty());
    }

    /// EVERY finding names a fix, not only the breach (V28).
    #[test]
    fn every_finding_says_what_to_do_about_it() {
        let held = row("S1");
        let seen = vec![Seen {
            row: &held,
            source: None,
            artifact: Some("# s\n"),
        }];
        let found = audit(&seen, &[stray()]);
        assert_eq!(found.len(), 6, "{found:?}");
        for one in &found {
            assert!(names_a_fix(&one.said), "no fix named: {}", one.said);
        }
    }

    /// A fix is an IMPERATIVE. Matching on the verbs is coarse, but the
    /// failure it guards against is a sentence that describes the breach
    /// and stops -- and that sentence has none of these.
    fn names_a_fix(said: &str) -> bool {
        [
            "run ",
            "Name ",
            "state ",
            "Write ",
            "Delete ",
            "Restore ",
            "Fill ",
            "Put ",
            "Give ",
            "reclassify",
        ]
        .iter()
        .any(|verb| said.contains(verb))
    }

    fn stray() -> String {
        ".rekall/rules/stray.sh".to_string()
    }

    #[test]
    fn human_output_carries_the_kind_and_the_sentence() {
        let held = row("M1");
        let seen = vec![Seen {
            row: &held,
            source: None,
            artifact: Some(RUNNER),
        }];
        let text = render_human(&audit(&seen, &[]));
        assert!(text.contains(MISSING_SOURCE), "{text}");
        assert!(text.contains("CLAUDE.md"), "{text}");
    }

    /// Order is ledger order, then orphans. A gate whose output reshuffles
    /// between runs is a gate whose diff nobody can read.
    #[test]
    fn the_report_is_order_stable() {
        let first = row("M1");
        let mut second = row("M1");
        second.id = "def5678".to_string();
        second.artifact = ".rekall/rules/other.sh".to_string();
        let seen = vec![nothing_readable(&first), nothing_readable(&second)];
        let found = audit(&seen, &[stray()]);
        assert_eq!(
            found.iter().map(|one| one.id.clone()).collect::<Vec<_>>(),
            vec!["abc1234", "abc1234", "def5678", "def5678", ""]
        );
    }

    /// A row whose source and artifact are both unreadable -- two findings
    /// each, which is what makes the ORDER visible.
    fn nothing_readable(held: &ledger::Extracted) -> Seen<'_> {
        Seen {
            row: held,
            source: None,
            artifact: None,
        }
    }
    /// A WRAPPED statement, which is what B6 was made of. Every fixture in
    /// this file was one line, which is exactly why 509 green tests said
    /// nothing about the first statement a human actually wrote.
    fn wrapped_row() -> ledger::Extracted {
        ledger::Extracted {
            text: "- Rust source is ASCII only. `SPEC.md` symbols are FORMAT\n  and do not apply here."
                .to_string(),
            line_end: 4,
            ..row("M1")
        }
    }

    /// V42. A line of the statement reaching the script uncommented is a
    /// COMMAND, and the gate says so before someone writes a real check
    /// and the line starts running.
    #[test]
    fn a_statement_line_the_shell_would_run_is_drift() {
        let held = wrapped_row();
        let source = extracted_source(&held);
        let loose = "#!/bin/sh\n# - Rust source is ASCII only. `SPEC.md` symbols are FORMAT\nand do not apply here.\nexit 1\n";
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(loose),
        }];
        let found = audit(&seen, std::slice::from_ref(&held.artifact));
        assert!(kinds(&found).contains(&LOOSE_QUOTE), "{found:?}");
        assert!(found.iter().any(|one| one.said.contains("# ")), "{found:?}");
    }

    /// The round trip: what `apply` writes for a wrapped statement must
    /// pass the check that judges it. Two halves of one rule, tested
    /// against each other rather than against a hand-typed fixture.
    #[test]
    fn what_apply_writes_for_a_wrapped_statement_is_not_loose() {
        let held = wrapped_row();
        let source = extracted_source(&held);
        let written = generated(&held);
        let seen = vec![Seen {
            row: &held,
            source: Some(&source),
            artifact: Some(&written),
        }];
        let found = audit(&seen, std::slice::from_ref(&held.artifact));
        assert!(
            !kinds(&found).contains(&LOOSE_QUOTE),
            "{found:?}\n{written}"
        );
    }
}
