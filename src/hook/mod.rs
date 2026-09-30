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
use std::path::Path;

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
    /// What a shell tool was asked to RUN.
    ///
    /// The situation text a `word` trigger is tested against (B7). A tool
    /// call carries no prompt, so reading only `prompt` left every `word`
    /// trigger matching nothing on the one event `hook` runs on.
    #[serde(default)]
    pub command: Option<String>,
}

/// Turn a payload into the situation the matcher understands.
#[must_use]
pub fn situation(payload: &Payload) -> trigger::Situation {
    trigger::Situation {
        tool: payload.tool_name.clone(),
        path: payload.tool_input.as_ref().and_then(path_of),
        cwd: payload.cwd.clone(),
        text: text_of(payload),
    }
}

/// What a `word` trigger is tested against.
///
/// BOTH halves, joined: what the user typed on the events that carry it,
/// and what the tool was asked to do on the events that carry that. A tool
/// call has no prompt, so reading `prompt` alone made every `word` trigger
/// dead on the only event `hook` runs on -- and `recall`, which takes the
/// situation as an argument, said the opposite (B7, V18).
fn text_of(payload: &Payload) -> String {
    let typed = payload.prompt.clone().unwrap_or_default();
    let asked = payload
        .tool_input
        .as_ref()
        .and_then(|input| input.command.clone())
        .unwrap_or_default();
    [typed, asked]
        .iter()
        .filter(|part| !part.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
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

/// Which harness is on the other end.
///
/// NAMED, never sniffed (`src:V47`). Codex sends `turn_id` and
/// `permission_mode` where Claude Code sends neither, so a discriminator
/// would rest on ABSENCE -- and absence is what the next release changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Agent {
    /// The DEFAULT, admitted as one: it keeps every hook line already
    /// pasted into a `settings.json` working, and a Codex user passes the
    /// flag or gets V49's refusal instead of a silent no-op.
    #[default]
    Claude,
    Codex,
}

/// The names this build renders for.
pub const AGENTS: [&str; 2] = ["claude", "codex"];

/// An UNKNOWN name is a usage error, never a fall back to the default
/// (`src:V47`). Someone who typed `--agent codx` meant something by it,
/// and quietly answering as Claude Code would deliver the wrong dialect
/// to a harness that then loads nothing and says nothing.
pub fn agent_named(raw: &str) -> Result<Agent, String> {
    match raw {
        "claude" => Ok(Agent::Claude),
        "codex" => Ok(Agent::Codex),
        other => Err(format!(
            "unknown --agent `{other}` -- expected one of {}",
            AGENTS.join(", ")
        )),
    }
}

/// What the harness is told, and what the human is told when it cannot be.
///
/// Two channels because V58 splits them: `decision` goes to stdout for the
/// harness to parse, and `refused` goes to stderr for the person who wired
/// this. A refusal still yields a VALID EMPTY DECISION, so the harness
/// never meets a parse error on top of a skip.
#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    pub decision: serde_json::Value,
    pub refused: Option<String>,
}

/// What the harness is told.
///
/// SILENT when nothing loads: an empty object is a valid decision that
/// says "no opinion", and injecting an empty context block on every tool
/// call would be the always-on cost this whole crate exists to remove.
///
/// The RENDERING differs per agent and the difference is not cosmetic
/// (V49). Emitting `additionalContext` at a Codex `PreToolUse` loads
/// nothing and says nothing -- exit 0, a skip wearing a successful exit,
/// which is the one failure mode this crate refuses.
#[must_use]
pub fn decision(agent: Agent, event: Option<&str>, skills: &[String]) -> Reply {
    if skills.is_empty() {
        return quiet();
    }
    rendered(agent, event.unwrap_or("PreToolUse"), skills)
}

/// ONE (agent, event) pair, rendered or refused.
fn rendered(agent: Agent, at: &str, skills: &[String]) -> Reply {
    let joined = skills.join("\n\n");
    match agent {
        Agent::Claude => say(serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": at,
                "additionalContext": joined,
            }
        })),
        Agent::Codex if at == "PreToolUse" => {
            say(serde_json::json!({ "systemMessage": joined }))
        }
        Agent::Codex => refuse(at, skills.len()),
    }
}

fn quiet() -> Reply {
    Reply {
        decision: serde_json::json!({}),
        refused: None,
    }
}

fn say(decision: serde_json::Value) -> Reply {
    Reply {
        decision,
        refused: None,
    }
}

/// A pair this crate has not MEASURED, named rather than guessed at.
///
/// It says what was dropped and where the gap is, because the alternative
/// is an envelope nobody here has run -- and a shape guessed right is
/// indistinguishable from one guessed wrong until it silently drops a
/// skill (V58).
fn refuse(event: &str, dropped: usize) -> Reply {
    Reply {
        decision: serde_json::json!({}),
        refused: Some(format!(
            "rekall: {dropped} skill(s) NOT delivered -- no measured Codex \
             rendering for `{event}`, and this build will not guess at one. \
             Codex `PreToolUse` is the pair that works (V49, V58)"
        )),
    }
}

/// Whether this project WIRES this adapter.
///
/// Here rather than in `check`, which asks the question, or `plan`, which
/// now asks it too: this module is the one that knows a harness exists at
/// all (the federation edge table), and two callers reading a settings
/// file two different ways is how they end up disagreeing about it.
///
/// The project settings files `docs/INTEGRATION.md` names.
///
/// PROJECT scope only. A home path is not this crate's to read -- `check`
/// reads the ledger and the files the ledger names -- so a user-level
/// wiring is invisible here, and the finding SAYS that rather than
/// asserting there is none.
const SETTINGS: [&str; 2] = ["settings.json", "settings.local.json"];

/// Does anything in this project wire `rekall hook`?
///
/// Matched as TEXT, not by parsing the hook schema. The schema is the
/// harness's and it changes on their release cadence, so a parser here
/// would answer "not wired" the day they nest the key one level deeper --
/// reporting a fault that is really a version skew. The literal
/// `rekall hook` in a settings file means one thing, and JSON has no
/// comments to hide it in.
#[must_use]
pub fn wired(base: &Path) -> bool {
    SETTINGS.iter().any(|name| {
        std::fs::read_to_string(base.join(".claude").join(name))
            .is_ok_and(|text| text.contains("rekall hook"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wiring is read as TEXT from the files `INTEGRATION.md` names,
    /// and `settings.local.json` counts -- it is where a person wiring
    /// this for themselves would put it.
    #[test]
    fn wiring_is_found_in_either_settings_file() {
        let dir = std::path::PathBuf::from("target").join("hook-wired");
        let claude = dir.join(".claude");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&claude);
        assert!(!wired(&dir), "nothing written yet");
        let _ = std::fs::write(
            claude.join("settings.local.json"),
            "{\"hooks\":{\"PreToolUse\":[{\"hooks\":[{\"command\":\"rekall hook\"}]}]}}",
        );
        assert!(wired(&dir));
    }

    /// A settings file that mentions the crate but not the VERB is not
    /// wiring. `rekall check` in a lint hook delivers no skill.
    #[test]
    fn another_rekall_verb_is_not_a_hook() {
        let dir = std::path::PathBuf::from("target").join("hook-other-verb");
        let claude = dir.join(".claude");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&claude);
        let _ = std::fs::write(
            claude.join("settings.json"),
            "{\"hooks\":{\"PreToolUse\":[{\"hooks\":[{\"command\":\"rekall check\"}]}]}}",
        );
        assert!(!wired(&dir));
    }
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
        let out = decision(Agent::Claude, Some("PreToolUse"), &[]);
        assert_eq!(out.decision, serde_json::json!({}));
        assert!(out.refused.is_none());
    }

    #[test]
    fn a_loading_skill_is_injected_as_context() {
        let said = ["be careful".to_string()];
        let out = decision(Agent::Claude, Some("PreToolUse"), &said).decision;
        assert_eq!(context_of(&out), Some("be careful".to_string()));
        assert_eq!(
            out.pointer("/hookSpecificOutput/hookEventName")
                .and_then(serde_json::Value::as_str),
            Some("PreToolUse")
        );
    }

    #[test]
    fn several_skills_are_joined() {
        let said = ["one".to_string(), "two".to_string()];
        let out = decision(Agent::Claude, None, &said).decision;
        assert_eq!(context_of(&out), Some("one\n\ntwo".to_string()));
    }

    /// V49, and the whole reason this dialect exists. MEASURED: Codex
    /// `PreToolUse` accepts `systemMessage` and ignores
    /// `additionalContext`, so the Claude rendering there is a silent
    /// no-op -- exit 0, nothing loaded, nothing said.
    #[test]
    fn codex_is_told_at_a_pretooluse_through_system_message() {
        let said = ["be careful".to_string()];
        let out = decision(Agent::Codex, Some("PreToolUse"), &said).decision;
        assert_eq!(
            out.get("systemMessage").and_then(serde_json::Value::as_str),
            Some("be careful")
        );
        assert!(out.get("hookSpecificOutput").is_none(), "{out}");
    }

    /// V58: a pair nobody here has RUN is refused and named, never
    /// guessed at. A shape guessed right is indistinguishable from one
    /// guessed wrong until it silently drops a skill.
    #[test]
    fn an_unmeasured_codex_event_refuses_and_says_what_was_dropped() {
        let said = ["be careful".to_string()];
        let out = decision(Agent::Codex, Some("SessionStart"), &said);
        assert_eq!(out.decision, serde_json::json!({}), "still valid JSON");
        let why = out.refused.unwrap_or_default();
        assert!(why.contains("1 skill(s) NOT delivered"), "{why}");
        assert!(why.contains("SessionStart"), "{why}");
        assert!(why.contains("PreToolUse"), "names what works: {why}");
    }

    /// Nothing to deliver is not a refusal, whatever the event. There is
    /// no skip to report when there was nothing to skip.
    #[test]
    fn an_unmeasured_pair_with_nothing_loading_stays_quiet() {
        let out = decision(Agent::Codex, Some("SessionStart"), &[]);
        assert_eq!(out.decision, serde_json::json!({}));
        assert!(out.refused.is_none());
    }

    /// `src:V47`: the name is NAMED, and an unknown one is a usage error
    /// rather than a quiet fall back to the default -- which would hand
    /// the wrong dialect to a harness that then loads nothing.
    #[test]
    fn an_unknown_agent_is_refused_and_lists_the_known_ones() {
        assert_eq!(agent_named("claude").ok(), Some(Agent::Claude));
        assert_eq!(agent_named("codex").ok(), Some(Agent::Codex));
        let why = agent_named("codx").err().unwrap_or_default();
        assert!(why.contains("codx"), "{why}");
        assert!(why.contains("claude, codex"), "{why}");
    }

    /// The default is `claude`, admitted as one: it keeps every hook line
    /// already pasted into a `settings.json` working.
    #[test]
    fn the_default_agent_is_claude() {
        assert_eq!(Agent::default(), Agent::Claude);
    }
}
