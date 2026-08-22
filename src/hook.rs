//! `rekall hook` -- the harness adapter.
//!
//! ADAPTER SHAPE, copied from `itok guard` because it is proven (R9): one
//! process per call, hook JSON on stdin, decision JSON on stdout, no
//! daemon and nothing in the request path. The signal is in the JSON and
//! never in the exit code -- a harness reads the document, and a nonzero
//! exit there means "the tool broke", not "the answer is no".
//!
//! V17's ONE exception lives here: `hook` speaks the harness's JSON on
//! both ends rather than offering `--format human|json`, because a harness
//! is its only caller. `recall` is the human view of the same decision,
//! and V18 makes that literal -- this module computes NOTHING about
//! matching. It maps a payload onto a `Situation`, hands it to the same
//! `recall::decide`, and maps the answer back.

use crate::{recall, trigger};

/// The fields this crate reads from a harness payload.
///
/// Everything else the harness sends is IGNORED rather than rejected: a
/// hook payload is someone else's document with its own release cadence,
/// and refusing an unfamiliar field would break this adapter every time
/// the harness grew one.
#[derive(Debug, Default, serde::Deserialize)]
pub struct Payload {
    #[serde(default)]
    pub hook_event_name: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Option<ToolInput>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// What the user typed, on the events that carry it.
    #[serde(default)]
    pub prompt: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct ToolInput {
    #[serde(default)]
    pub file_path: Option<String>,
    /// Notebooks name their path differently. Read both rather than
    /// letting one tool's spelling silently match nothing.
    #[serde(default)]
    pub notebook_path: Option<String>,
}

/// Turn a payload into the situation the matcher understands.
#[must_use]
pub fn situation(payload: &Payload) -> trigger::Situation {
    trigger::Situation {
        tool: payload.tool_name.clone(),
        path: payload.tool_input.as_ref().and_then(path_of),
        cwd: payload.cwd.clone(),
        text: payload.prompt.clone().unwrap_or_default(),
    }
}

fn path_of(input: &ToolInput) -> Option<String> {
    input
        .file_path
        .clone()
        .or_else(|| input.notebook_path.clone())
}

/// Read a payload. An unreadable one is an EMPTY situation, not a crash.
///
/// A harness that changed its shape should get "nothing loads" rather than
/// a broken tool in its request path -- this runs on every call, and the
/// blast radius of panicking here is every tool use, not one report.
#[must_use]
pub fn parse(stdin: &str) -> Payload {
    serde_json::from_str(stdin).unwrap_or_default()
}

/// The ids whose skills load, in report order.
#[must_use]
pub fn loading(report: &recall::Report) -> Vec<String> {
    report
        .rows
        .iter()
        .filter(|row| row.loads)
        .map(|row| row.id.clone())
        .collect()
}

/// What the harness is told.
///
/// SILENT when nothing loads: an empty object is a valid decision that
/// says "no opinion", and injecting an empty context block on every tool
/// call would be the always-on cost this whole crate exists to remove.
#[must_use]
pub fn decision(event: Option<&str>, skills: &[String]) -> serde_json::Value {
    if skills.is_empty() {
        return serde_json::json!({});
    }
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": event.unwrap_or("PreToolUse"),
            "additionalContext": skills.join("\n\n"),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_of(out: &serde_json::Value) -> Option<String> {
        out.pointer("/hookSpecificOutput/additionalContext")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string)
    }

    const EDIT: &str = r#"{"hook_event_name":"PreToolUse","tool_name":"Edit",
        "tool_input":{"file_path":"src/main.rs"},"cwd":"/repo"}"#;

    #[test]
    fn a_payload_becomes_a_situation() {
        let at = situation(&parse(EDIT));
        assert_eq!(at.tool.as_deref(), Some("Edit"));
        assert_eq!(at.path.as_deref(), Some("src/main.rs"));
        assert_eq!(at.cwd.as_deref(), Some("/repo"));
    }

    /// A notebook names its path differently. Reading only `file_path`
    /// would make every notebook edit match nothing, silently.
    #[test]
    fn a_notebook_path_is_read_too() {
        let raw = r#"{"tool_name":"NotebookEdit",
            "tool_input":{"notebook_path":"a.ipynb"}}"#;
        assert_eq!(situation(&parse(raw)).path.as_deref(), Some("a.ipynb"));
    }

    #[test]
    fn the_prompt_becomes_the_situation_text() {
        let raw =
            r#"{"hook_event_name":"UserPromptSubmit","prompt":"run clippy"}"#;
        assert_eq!(situation(&parse(raw)).text, "run clippy");
    }

    /// Fields this crate does not know are IGNORED. A harness grows them,
    /// and rejecting one would break the adapter on somebody else's
    /// release schedule.
    #[test]
    fn unknown_fields_do_not_break_the_adapter() {
        let raw = r#"{"tool_name":"Edit","session_id":"x","a_field":{"b":1}}"#;
        assert_eq!(situation(&parse(raw)).tool.as_deref(), Some("Edit"));
    }

    /// Malformed input yields an EMPTY situation, not a panic. This runs
    /// in the request path of every tool call.
    #[test]
    fn unreadable_input_yields_an_empty_situation() {
        let at = situation(&parse("not json at all"));
        assert!(at.tool.is_none() && at.path.is_none() && at.text.is_empty());
    }

    #[test]
    fn nothing_loading_is_an_empty_decision() {
        assert_eq!(decision(Some("PreToolUse"), &[]), serde_json::json!({}));
    }

    #[test]
    fn a_loading_skill_is_injected_as_context() {
        let out = decision(Some("PreToolUse"), &["be careful".to_string()]);
        assert_eq!(context_of(&out), Some("be careful".to_string()));
        assert_eq!(
            out.pointer("/hookSpecificOutput/hookEventName")
                .and_then(serde_json::Value::as_str),
            Some("PreToolUse")
        );
    }

    #[test]
    fn several_skills_are_joined() {
        let out = decision(None, &["one".to_string(), "two".to_string()]);
        assert_eq!(context_of(&out), Some("one\n\ntwo".to_string()));
    }
}
