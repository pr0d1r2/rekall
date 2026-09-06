//! `catch` -- the SECOND intake (T18).
//!
//! `scan` reads a corpus somebody wrote deliberately. This reads a
//! TRANSCRIPT, where a rule appears the moment a human corrects the agent
//! and is gone with the session that produced it. A violation seen at turn
//! 200 is lost tomorrow unless the candidate outlives it, which is the
//! whole reason the verb exists (section I).
//!
//! Three invariants shape everything here:
//!
//! * **V44** -- a violation is a USER turn the classifier calls a RULE,
//!   which is to say anything but `U`. The assistant's own text is not
//!   evidence, and the criterion is the CLASS rather than a signal family
//!   -- scoping it to mood shipped for one commit and missed both "never
//!   X" and "always X", which B8 measured.
//! * **V45** -- writing CANDIDATE rows to the ledger is not a breach of
//!   report-only. Report-only is about the CORPUS, not the disk.
//! * **V46** -- a transcript is someone else's document, so unknown fields
//!   are ignored and unreadable lines are skipped AND COUNTED.

use crate::classify::{self, Form, Weights};
use crate::statement;

/// One statement worth proposing, with the argument for it attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caught {
    pub id: String,
    pub src: String,
    pub text: String,
    /// `class`, `sharpness` and `label` are carried SEPARATELY, the same
    /// anatomy `scan` reports (V17). A consumer of `catch --format json`
    /// must not have to split "M2" back into its parts any more than a
    /// consumer of `scan` does.
    pub class: String,
    pub sharpness: Option<u8>,
    pub label: String,
    pub signals: Vec<String>,
}

/// What one pass over a transcript found, and what it could not read.
///
/// `skipped` is reported rather than swallowed. A transcript half of which
/// failed to parse yields few candidates, and "few candidates" and "few
/// violations" are the same output unless the count says otherwise --
/// V26's lie moved from a gate step into a verb.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub caught: Vec<Caught>,
    pub turns: usize,
    pub skipped: usize,
}

/// Read a transcript and propose every user statement that classifies as
/// a rule.
#[must_use]
pub fn catch(jsonl: &str, src: &str, weights: &Weights) -> Report {
    let (turns, skipped) = user_turns(jsonl);
    let mut caught = Vec::new();
    for turn in &turns {
        collect(turn, src, weights, &mut caught);
    }
    Report {
        caught,
        turns: turns.len(),
        skipped,
    }
}

/// Statements inside ONE user turn.
///
/// `Form::ListItem` is deliberate and is V44 doing V40's scoping in the
/// other coordinate. V40 counts mood only inside a list item because a
/// corpus states its rules as bullets and its context as paragraphs. A
/// transcript has no bullets; what separates a rule from context there is
/// WHO IS SPEAKING, and that filter has already been applied by the time
/// this is called. Passing `Paragraph` here would score every user turn as
/// context and the verb would find nothing at all.
///
/// The kept rows are everything the classifier does NOT call `U`. That is
/// section I's "classed like any other" taken literally: `U` is already the
/// answer for a request, so a transcript's ordinary "could you look at the
/// parser?" falls out without a second rule to describe it.
fn collect(turn: &str, src: &str, weights: &Weights, out: &mut Vec<Caught>) {
    for said in statement::split(turn, src) {
        let verdict = classify::classify(&said.text, Form::ListItem, weights);
        if verdict.class == classify::Class::U {
            continue;
        }
        out.push(Caught {
            id: said.id,
            src: src.to_owned(),
            text: said.text,
            class: verdict.class.to_string(),
            sharpness: verdict.sharpness,
            label: verdict.label(),
            signals: verdict.names(),
        });
    }
}

/// The user's turns, and how many lines could not be read.
fn user_turns(jsonl: &str) -> (Vec<String>, usize) {
    let mut turns = Vec::new();
    let mut skipped: usize = 0;
    for line in jsonl.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(value) => push_user(&value, &mut turns),
            Err(_) => skipped = skipped.saturating_add(1),
        }
    }
    (turns, skipped)
}

fn push_user(value: &serde_json::Value, turns: &mut Vec<String>) {
    if role_of(value) != Some("user") {
        return;
    }
    let text = text_of(value);
    if !text.trim().is_empty() {
        turns.push(text);
    }
}

/// Who spoke, read from whichever of three spellings the payload uses.
///
/// V46: this is someone else's document. A reader that insists on one
/// shape breaks on the harness's next release, which is the lesson `hook`
/// already learned the expensive way (B7).
fn role_of(value: &serde_json::Value) -> Option<&str> {
    value
        .get("role")
        .and_then(serde_json::Value::as_str)
        .or_else(|| nested_str(value, "message", "role"))
        // Codex nests the whole turn under `payload` (R16). Without this
        // the role lookup falls through to `type`, reads "response_item",
        // matches no human, and reports a confident zero.
        .or_else(|| nested_str(value, "payload", "role"))
        .or_else(|| value.get("type").and_then(serde_json::Value::as_str))
}

fn nested_str<'a>(
    value: &'a serde_json::Value,
    outer: &str,
    inner: &str,
) -> Option<&'a str> {
    value
        .get(outer)
        .and_then(|held| held.get(inner))
        .and_then(serde_json::Value::as_str)
}

/// What was said. Content is a bare string in some payloads and a list of
/// typed blocks in others; anything else yields nothing rather than an
/// error, because an unfamiliar shape is not a corrupt file.
fn text_of(value: &serde_json::Value) -> String {
    let content = value
        .get("content")
        .or_else(|| value.get("message").and_then(|held| held.get("content")))
        .or_else(|| value.get("payload").and_then(|held| held.get("content")));
    match content {
        Some(serde_json::Value::String(said)) => said.clone(),
        Some(serde_json::Value::Array(blocks)) => blocks_text(blocks),
        _ => String::new(),
    }
}

fn blocks_text(blocks: &[serde_json::Value]) -> String {
    blocks
        .iter()
        .filter_map(|block| block.get("text"))
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caught(jsonl: &str) -> Report {
        catch(jsonl, "session.jsonl", &Weights::default())
    }

    const CORRECTION: &str =
        r#"{"role":"user","content":"Never commit a .env file."}"#;

    #[test]
    fn a_user_correction_becomes_a_candidate() {
        let out = caught(CORRECTION);
        assert_eq!(out.caught.len(), 1);
        assert_eq!(out.turns, 1);
        assert_eq!(out.skipped, 0);
    }

    /// V44's load-bearing half. A model restating the rule it just broke
    /// would otherwise mint a candidate out of its own apology, and the
    /// ledger would fill with rules nobody wrote.
    #[test]
    fn the_assistant_saying_the_same_thing_is_not_evidence() {
        let said =
            r#"{"role":"assistant","content":"Never commit a .env file."}"#;
        assert!(caught(said).caught.is_empty());
        assert_eq!(caught(said).turns, 0);
    }

    #[test]
    fn an_ordinary_request_is_not_a_violation() {
        let said =
            r#"{"role":"user","content":"Could you look at the parser?"}"#;
        assert!(
            caught(said).caught.is_empty(),
            "a transcript is mostly requests; they are not rules"
        );
    }

    #[test]
    fn a_nested_message_carries_the_same_meaning() {
        let said = r#"{"message":{"role":"user","content":"Never commit a .env file."}}"#;
        assert_eq!(caught(said).caught.len(), 1);
    }

    #[test]
    fn typed_content_blocks_are_read() {
        let said = concat!(
            r#"{"role":"user","content":[{"type":"text","#,
            r#""text":"Never commit a .env file."}]}"#
        );
        assert_eq!(caught(said).caught.len(), 1);
    }

    /// V46: an unfamiliar field is not a corrupt file.
    #[test]
    fn an_unknown_field_is_ignored_not_rejected() {
        let said = concat!(
            r#"{"role":"user","content":"Never commit a .env file.","#,
            r#""uuid":"x","tokens":{"in":4}}"#
        );
        assert_eq!(caught(said).caught.len(), 1);
        assert_eq!(caught(said).skipped, 0);
    }

    /// V46's other half: skipped AND counted. Silence here would read as
    /// "no violations found".
    #[test]
    fn an_unreadable_line_is_skipped_and_counted() {
        let said = format!("{CORRECTION}\nthis is not json\n{{oh dear\n");
        let out = caught(&said);
        assert_eq!(out.caught.len(), 1, "the readable line still counts");
        assert_eq!(out.skipped, 2, "both bad lines must be reported");
    }

    #[test]
    fn a_blank_line_is_not_a_skip() {
        let said = format!("\n{CORRECTION}\n\n");
        assert_eq!(caught(&said).skipped, 0);
    }

    #[test]
    fn an_empty_transcript_finds_nothing_and_says_nothing_failed() {
        let out = caught("");
        assert!(out.caught.is_empty());
        assert_eq!(out.skipped, 0);
        assert_eq!(out.turns, 0);
    }

    #[test]
    fn a_caught_statement_carries_its_argument() {
        let out = caught(CORRECTION);
        let shape: Vec<String> = out.caught.iter().map(argument_of).collect();
        let want = "id=true src=session.jsonl class=true signals=true";
        assert_eq!(shape, vec![want.to_string()], "{out:?}");
    }

    /// Every part of the argument in one string, so the assertion is a
    /// single comparison against a single expectation rather than four
    /// that can each pass while the row is wrong.
    fn argument_of(row: &Caught) -> String {
        format!(
            "id={} src={} class={} signals={}",
            !row.id.is_empty(),
            row.src,
            !row.label.is_empty(),
            !row.signals.is_empty()
        )
    }

    /// Two turns saying the same thing are one candidate's worth of id, so
    /// re-reading a transcript does not grow the ledger. The ledger's
    /// `propose` refuses the duplicate; this proves the ids collide as it
    /// expects them to.
    #[test]
    fn the_same_correction_twice_yields_the_same_id() {
        let said = format!("{CORRECTION}\n{CORRECTION}");
        let out = caught(&said);
        let ids: Vec<&str> = out.caught.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(out.turns, 2);
        assert!(ids.windows(2).all(|w| w.first() == w.last()));
    }
    /// The CODEX shape, from R16 rather than from documentation: the turn
    /// nests under `payload`, the human is `user` there, and the words are
    /// in `payload.content[].text` as `input_text` blocks.
    const CODEX_USER: &str = concat!(
        r#"{"timestamp":"2026-09-05T10:11:24Z","type":"response_item","#,
        r#""payload":{"type":"message","role":"user","content":"#,
        r#"[{"type":"input_text","text":"Never commit a .env file."}]}}"#
    );

    #[test]
    fn a_codex_user_turn_is_read_through_payload() {
        let out = caught(CODEX_USER);
        assert_eq!(out.turns, 1, "payload.role must be reached");
        assert_eq!(out.caught.len(), 1);
    }

    /// V51, and the reason it is not merely tidiness. `developer` appears
    /// exactly once per Codex session and IS the injected `AGENTS.md`. The
    /// only public writeup of this format calls it the human; believing it
    /// would mine the agent's own rules file and hand them back as finds.
    #[test]
    fn a_codex_developer_turn_is_the_instructions_and_is_not_evidence() {
        let said =
            CODEX_USER.replace(r#""role":"user""#, r#""role":"developer""#);
        let out = caught(&said);
        assert_eq!(out.turns, 0, "the instruction preamble is not a turn");
        assert!(out.caught.is_empty());
    }

    #[test]
    fn a_codex_assistant_turn_is_not_evidence_either() {
        let said =
            CODEX_USER.replace(r#""role":"user""#, r#""role":"assistant""#);
        assert_eq!(caught(&said).turns, 0);
    }

    /// Both harnesses in one file: `catch` iterates roots, so a run may
    /// meet either shape and must not need telling which.
    #[test]
    fn claude_and_codex_shapes_read_side_by_side() {
        let claude = r#"{"role":"user","content":"Always run the gate before pushing."}"#;
        let out = caught(&format!("{CODEX_USER}\n{claude}"));
        assert_eq!(out.turns, 2, "one shape must not shadow the other");
        assert_eq!(out.skipped, 0);
    }

    /// A turn whose content is neither a string nor a block list carries
    /// no text this can read. Unknown shapes are IGNORED rather than
    /// refused (V46) -- a harness is free to add a field, and a reader
    /// that dies on one is a reader that stops working on an upgrade.
    #[test]
    fn a_content_shape_it_does_not_know_reads_as_empty() {
        let odd = "{\"type\":\"user\",\"message\":{\"content\":42}}\n";
        let out = caught(odd);
        assert!(out.caught.is_empty(), "{out:?}");
        assert_eq!(out.skipped, 0, "a shape it can parse is not unreadable");
    }
}
