use super::recall::{read_artifacts, zip_candidates};
use super::{Env, USAGE_EXIT, resolve};
use crate::{hook, ledger, recall, runner};
use std::path::Path;
/// Run `hook`: payload in, decision out.
///
/// Takes the payload as a STRING rather than reading stdin itself, so
/// everything with a decision in it is testable without a pipe. The two
/// lines that touch stdin and stdout are in `run_hook`.
///
/// V18 made literal: this calls `recall::decide`, the same function the
/// `recall` verb prints. There is no second matcher to disagree with.
pub fn hook_command(
    stdin: &str,
    base: &Path,
    agent: hook::Agent,
) -> Result<hook::Reply, String> {
    let payload = hook::parse(stdin);
    let said = advice_for(&payload, base)?;
    Ok(hook::decision(
        agent,
        payload.hook_event_name.as_deref(),
        &said,
    ))
}

/// What this situation has to say, in the order V18 requires: ONE matcher,
/// the same one `recall` prints from.
fn advice_for(
    payload: &hook::Payload,
    base: &Path,
) -> Result<Vec<String>, String> {
    let path = ledger::path_in(base);
    let held = ledger::load(&path).map_err(|e| e.to_string())?;
    let texts = read_artifacts(base, &held);
    let candidates = zip_candidates(&held, &texts);
    let report = recall::decide(&candidates, &hook::situation(payload));
    let loading = hook::loading(&report);
    count_fires(&path, &loading);
    let at = At {
        base,
        limit: bound(base),
    };
    Ok(advice(&loading, &held, &texts, &at))
}

/// The runner's CPU bound, from config (B5).
///
/// Read at the edge, so everything below stays a function of its
/// arguments.
fn bound(base: &Path) -> std::time::Duration {
    runner::limit_from(
        resolve(base).ok().and_then(|held| held.runner_timeout_ms),
    )
}

/// What the harness is told, per firing row.
///
/// An `S` contributes its TEXT -- the skill is the advice. An `M`
/// contributes what its RUNNER said, and only when the runner objected: a
/// rule that looked and found nothing has nothing to add, and injecting
/// "passed" on every tool call is the always-on cost this crate removes.
fn advice(
    loading: &[String],
    held: &ledger::Ledger,
    texts: &[Option<String>],
    at: &At<'_>,
) -> Vec<String> {
    held.extracted
        .iter()
        .zip(texts)
        .filter(|(row, _)| loading.contains(&row.id))
        .filter_map(|(row, text)| said_by(row, text.as_deref(), at))
        .collect()
}

fn said_by(
    row: &ledger::Extracted,
    text: Option<&str>,
    at: &At<'_>,
) -> Option<String> {
    if !row.label.starts_with('M') {
        // V43: the PAYLOAD, not the file. The scaffold beside it is for
        // whoever maintains the trigger, and it is 8x the rule -- paid at
        // the fire point, which is the one place this crate exists to
        // keep cheap. An UNMARKED artifact still delivers its rule: that
        // is the gate's finding to report, not the hook's to enforce by
        // withholding a rule someone is about to need.
        return text.map(|whole| {
            crate::apply::artifact::payload_of(whole)
                .unwrap_or_else(|| whole.to_string())
        });
    }
    runner::run(&at.base.join(&row.artifact), at.limit).advice()
}

/// Where the corpus is and how long a rule gets. Bundled because they
/// travel together into every runner call and mean nothing apart.
struct At<'a> {
    base: &'a Path,
    limit: std::time::Duration,
}

/// V34: the counter, and the ONLY thing `hook` writes.
///
/// A failure to record is SWALLOWED. This runs in the request path of
/// every tool call, and a read-only checkout or a full disk must not turn
/// "your skill loaded" into "your tool broke" -- a lost count is a smaller
/// harm than a wedged harness, and `--dead` degrades toward reporting
/// MORE artifacts dead, which is the safe direction.
fn count_fires(path: &Path, loading: &[String]) {
    for id in loading {
        let _ = ledger::record_fire(&ledger::fires_beside(path), id);
    }
}

/// Stdin to stdout, and NEVER a signal in the exit code (section I).
///
/// Exit 0 even when the ledger cannot be read: a harness reads the
/// document, and a nonzero exit here would read as "the tool broke" on
/// every single tool call.
pub(super) fn run_hook(flags: &[String], env: &Env) -> u8 {
    let agent = match agent_from(flags) {
        Ok(agent) => agent,
        Err(why) => {
            eprintln!("rekall: {why}");
            return USAGE_EXIT;
        }
    };
    answer(agent, env)
}

/// Read the payload, write the decision, say what could not be delivered.
///
/// Exit 0 either way (V58): this adapter is in the request path of every
/// tool call, so a nonzero exit is not one bad report, it is an error on
/// every action a person takes -- and the first fix anyone reaches for is
/// deleting the hook line.
fn answer(agent: hook::Agent, env: &Env) -> u8 {
    let mut stdin = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut stdin);
    let reply = hook_command(&stdin, &env.cwd, agent)
        .unwrap_or_else(|_| hook::decision(agent, None, &[]));
    if let Some(said) = reply.refused.as_deref() {
        eprintln!("{said}");
    }
    println!("{}", reply.decision);
    0
}

/// `hook` takes ONE flag, and it is the only one it will ever take.
///
/// `--format` is refused on purpose: this verb speaks the harness's JSON
/// on both ends, so there is no second format to choose (V17's exception).
/// An unknown flag is a usage error rather than something ignored -- a
/// flag silently dropped reads, to whoever typed it, exactly like a flag
/// that was honoured.
fn agent_from(flags: &[String]) -> Result<hook::Agent, String> {
    match flags {
        [] => Ok(hook::Agent::default()),
        [flag, name] if flag == "--agent" => hook::agent_named(name),
        [flag] if flag == "--agent" => {
            Err("`--agent` needs a value".to_string())
        }
        _ => Err(
            "`hook` takes one payload on stdin and at most `--agent <name>`"
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::consent::read_answer;

    use crate::cli::reported;
    use crate::cli::scan::scan_command;
    use crate::cli::testing::*;
    use crate::cli::{Action, USAGE_EXIT, decide, perform};
    use crate::{apply, check, ledger};
    use std::path::{Path, PathBuf};

    fn payload(dir: &Path, path: &str) -> String {
        format!(
            "{{\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"Edit\",\
             \"tool_input\":{{\"file_path\":\"{path}\"}},\"cwd\":\"{}\"}}",
            dir.to_string_lossy()
        )
    }

    fn hook_in(dir: &Path, path: &str) -> serde_json::Value {
        decided(&payload(dir, path), dir, hook::Agent::Claude)
    }

    /// The decision alone, which is what most of these tests assert on.
    fn decided(
        stdin: &str,
        dir: &Path,
        agent: hook::Agent,
    ) -> serde_json::Value {
        hook_command(stdin, dir, agent)
            .map(|reply| reply.decision)
            .unwrap_or_default()
    }

    /// `--agent` reaches the verb as a flag, so a Codex user can name
    /// their harness on the line they paste into their config.
    #[test]
    fn the_agent_flag_is_dispatched() {
        assert_eq!(
            decide(&args(&["hook", "--agent", "codex"])),
            Action::Hook(args(&["--agent", "codex"]))
        );
    }

    #[test]
    fn no_flag_means_claude() {
        assert_eq!(agent_from(&[]).ok(), Some(hook::Agent::Claude));
    }

    #[test]
    fn the_agent_flag_names_the_dialect() {
        let flags = args(&["--agent", "codex"]);
        assert_eq!(agent_from(&flags).ok(), Some(hook::Agent::Codex));
    }

    /// `src:V47`: an unknown name is exit 2, never a fall back. And a
    /// flag this verb does not have is a usage error rather than
    /// something ignored -- a flag silently dropped reads, to whoever
    /// typed it, exactly like one that was honoured.
    #[test]
    fn a_bad_agent_or_a_stray_flag_exits_two() {
        assert_eq!(
            perform(Action::Hook(args(&["--agent", "codx"])), &env()),
            USAGE_EXIT
        );
        assert!(agent_from(&args(&["--agent"])).is_err());
        assert!(agent_from(&args(&["--format", "json"])).is_err());
    }

    /// The whole point of T61, end to end: the SAME payload and the same
    /// project, rendered two ways because the two harnesses read two
    /// different keys. Before this, Codex got the Claude envelope and
    /// loaded nothing, silently.
    #[test]
    fn one_situation_renders_differently_per_agent() {
        let dir = recall_project("dialects", "path = [\"**/*.rs\"]", "");
        let said = payload(&dir, "src/main.rs");
        let claude = decided(&said, &dir, hook::Agent::Claude);
        let codex = decided(&said, &dir, hook::Agent::Codex);
        assert!(claude.get("hookSpecificOutput").is_some(), "{claude}");
        assert!(codex.get("systemMessage").is_some(), "{codex}");
        assert_eq!(
            claude
                .pointer("/hookSpecificOutput/additionalContext")
                .and_then(serde_json::Value::as_str),
            codex
                .get("systemMessage")
                .and_then(serde_json::Value::as_str),
            "same advice, two envelopes"
        );
    }

    #[test]
    fn hook_is_dispatched() {
        assert_eq!(decide(&args(&["hook"])), Action::Hook(Vec::new()));
    }

    /// V18, made literal. What `recall` PRINTS is what `hook` DECIDES,
    /// asserted by driving both from the same project and comparing --
    /// two matchers is the invisible defect, where each looks right alone
    /// and the divergence only shows in production.
    #[test]
    fn hook_decides_what_recall_prints() {
        let dir = recall_project("agree", "path = [\"**/*.rs\"]", "");
        let printed = recall_in(&dir, &["--tool", "Edit", "--path", "a.rs"]);
        let decided = hook_in(&dir, "a.rs");
        assert!(printed.starts_with("load"), "{printed}");
        assert!(decided.get("hookSpecificOutput").is_some(), "{decided}");

        let printed = recall_in(&dir, &["--tool", "Edit", "--path", "a.md"]);
        let decided = hook_in(&dir, "a.md");
        assert!(printed.starts_with("skip"), "{printed}");
        assert_eq!(decided, serde_json::json!({}), "{decided}");
    }

    /// The skill's RULE is what reaches the model. An id would tell it a
    /// rule exists without saying what the rule is; the whole file would
    /// tell it how to maintain a trigger it will never edit (V43).
    #[test]
    fn the_skill_text_is_what_gets_injected() {
        let dir = recall_project("inject", "path = [\"**/*.rs\"]", "");
        let out = hook_in(&dir, "a.rs");
        let context = out
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(context.contains("never commit"), "{context}");
    }

    /// V34: the counter, and V11 finally has an author. A skill that
    /// LOADS is a skill that fired.
    #[test]
    fn a_loaded_skill_is_counted_as_fired() {
        let dir = recall_project("counted", "path = [\"**/*.rs\"]", "");
        assert_eq!(fires_of(&dir), 0);
        let _ = hook_in(&dir, "a.rs");
        assert_eq!(fires_of(&dir), 1);
        let _ = hook_in(&dir, "a.rs");
        assert_eq!(fires_of(&dir), 2, "the second fire was lost");
    }

    /// A skill that did NOT load must not count. Otherwise `--dead`
    /// measures how often the hook ran, not how often the rule mattered.
    #[test]
    fn a_skill_that_did_not_load_is_not_counted() {
        let dir = recall_project("uncounted", "path = [\"**/*.rs\"]", "");
        let _ = hook_in(&dir, "notes.md");
        assert_eq!(fires_of(&dir), 0);
    }

    /// V34's hazard, exercised the way it would actually bite: one hook
    /// per tool call means concurrent writers. A read-modify-write of the
    /// ledger would lose counts here and only here.
    #[test]
    fn concurrent_hooks_do_not_lose_counts() {
        let dir = recall_project("concurrent", "path = [\"**/*.rs\"]", "");
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let at = dir.clone();
                scope.spawn(move || hook_in(&at, "a.rs"));
            }
        });
        assert_eq!(fires_of(&dir), 8, "a concurrent fire was lost");
    }

    /// V11 reads the folded number: `--dead` must stop calling an
    /// artifact dead once a hook has fired it.
    #[test]
    fn a_fired_skill_leaves_the_dead_list() {
        let dir = recall_project("dead", "path = [\"**/*.rs\"]", "");
        assert!(!dead_text(&dir).is_empty(), "should start dead");
        let _ = hook_in(&dir, "a.rs");
        assert_eq!(dead_text(&dir), "", "still dead after firing");
    }

    fn fires_of(dir: &Path) -> u64 {
        ledger::load(&ledger::path_in(dir))
            .unwrap_or_default()
            .extracted
            .first()
            .map_or(0, |row| row.fires)
    }

    /// Section I: the signal is in the JSON, NEVER the exit code. A
    /// harness reading a nonzero exit would call the tool broken on every
    /// tool use.
    #[test]
    fn a_broken_payload_still_exits_zero_with_valid_json() {
        let dir = check_project("hook-garbage");
        let out = decided("not json", &dir, hook::Agent::Claude);
        assert_eq!(out, serde_json::json!({}));
        assert_eq!(perform(Action::Hook(Vec::new()), &env()), 0);
    }

    #[test]
    fn hook_takes_no_flags() {
        assert_eq!(
            perform(Action::Hook(args(&["--format", "json"])), &env()),
            USAGE_EXIT
        );
    }

    /// A project with one extracted `M` rule whose runner is real and
    /// whose trigger fires on `*.rs`.
    ///
    fn rule_project(name: &str, body: &str) -> PathBuf {
        let dir = check_project(name);
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n",
        );
        let (id, _) = extracted(&dir);
        let path = dir.join(artifact_of(&dir, &id));
        let _ = std::fs::write(&path, body);
        let _ = crate::cli::publish::make_runnable(&path);
        write_trigger(&dir, &id, "path = [\"**/*.rs\"]");
        dir
    }

    /// An `M` artifact carries its runner AND its trigger block: the
    /// script is the rule, the block is when it arrives early (V37).
    fn write_trigger(dir: &Path, id: &str, fire: &str) {
        let path = dir.join(artifact_of(dir, id));
        let held = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::write(
            &path,
            format!(
                "{held}\n# {}\n#\n# ```rekall\n# {fire}\n# ```\n#\n# {}\n#\n# ```rekall\n# ```\n",
                apply::artifact::FIRES,
                apply::artifact::NOT_FIRES
            ),
        );
    }

    /// V37 end to end: a rule with a trigger FIRES from the hook, and its
    /// own words reach the model.
    #[test]
    fn a_mechanical_rule_with_a_trigger_advises() {
        let dir = rule_project(
            "m-fires",
            "#!/bin/sh\necho 'do not commit to main' >&2\nexit 1\n",
        );
        let out = hook_in(&dir, "a.rs");
        assert_eq!(
            out.pointer("/hookSpecificOutput/additionalContext")
                .and_then(serde_json::Value::as_str),
            Some("do not commit to main"),
            "{out}"
        );
    }

    /// V38, THE ONE THAT MATTERS. A failing rule ADVISES; it must never
    /// deny the tool call. A wrong rule that blocks costs the user their
    /// work, and unwedging it means editing the corpus mid-task.
    #[test]
    fn a_failing_rule_never_denies_the_tool_call() {
        let dir = rule_project("m-advises", "#!/bin/sh\necho no >&2\nexit 1\n");
        let out = hook_in(&dir, "a.rs");
        assert!(
            out.pointer("/hookSpecificOutput/permissionDecision")
                .is_none(),
            "the hook tried to block a tool call: {out}"
        );
        assert_eq!(perform(Action::Hook(Vec::new()), &env()), 0);
    }

    /// A rule that looked and found nothing says NOTHING. Injecting
    /// "passed" on every tool call is the always-on cost this crate
    /// exists to remove.
    #[test]
    fn a_clean_rule_adds_no_context() {
        let dir = rule_project("m-clean", "#!/bin/sh\nexit 0\n");
        assert_eq!(hook_in(&dir, "a.rs"), serde_json::json!({}));
    }

    /// V38: bounded. A hung rule is killed and reported rather than
    /// stalling the harness on every call.
    #[test]
    fn a_hanging_rule_does_not_stall_the_hook() {
        // This one wants the bound to BITE, so it sets a SHORT one --
        // through the same config key, which is the point.
        let dir = rule_project("m-hang", "#!/bin/sh\nsleep 30\n");
        set_runner_timeout(&dir, 150);
        let started = std::time::Instant::now();
        let out = hook_in(&dir, "a.rs");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "the hook was not bounded"
        );
        let said = out
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(said.contains("did not finish"), "{out}");
    }

    /// V2: a generated runner arrives EXECUTABLE. Otherwise the failure
    /// surfaces as "permission denied" at a tool call rather than as
    /// something `check` could have told you.
    #[test]
    fn a_generated_runner_is_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = check_project("m-mode");
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n",
        );
        let (id, _) = extracted(&dir);
        let mode = std::fs::metadata(dir.join(artifact_of(&dir, &id)))
            .map(|meta| meta.permissions().mode() & 0o111);
        assert_eq!(mode.ok(), Some(0o111), "the runner is not executable");
    }

    /// THE PAYOFF of T45. The generated artifact now TELLS you the block
    /// exists, so filling it in is a local edit rather than a spec read.
    /// This walks that path: take the emitted block, put a real trigger in
    /// it, and the rule arrives at the tool call.
    #[test]
    fn filling_the_emitted_block_makes_the_rule_fire() {
        let dir = one_extracted_rule("emitted-block");
        assert_eq!(hook_in(&dir, "a.rs"), serde_json::json!({}));
        write_a_real_rule(&dir);
        let out = hook_in(&dir, "a.rs");
        assert_eq!(
            out.pointer("/hookSpecificOutput/additionalContext")
                .and_then(serde_json::Value::as_str),
            Some("main is protected"),
            "{out}"
        );
    }

    /// What a user actually does once the artifact tells them the block is
    /// there: name the trigger, and REPLACE the placeholder echo with a
    /// real check rather than adding alongside it.
    fn write_a_real_rule(dir: &Path) {
        let path = dir.join(artifact_of(dir, &extracted_id(dir)));
        let held = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::write(&path, real_rule(&held));
    }

    fn extracted_id(dir: &Path) -> String {
        ledger::load(&ledger::path_in(dir))
            .unwrap_or_default()
            .extracted
            .first()
            .map(|row| row.id.clone())
            .unwrap_or_default()
    }

    fn real_rule(held: &str) -> String {
        held.replacen("# tool = []", "# tool = [\"Edit\"]", 1)
            .lines()
            .map(|line| {
                if line.contains(apply::artifact::UNIMPLEMENTED) {
                    "echo 'main is protected' >&2"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// `check` says NOTHING about an `M`s triggers, before or after the
    /// blocks are emitted. It wants the runner (V2) and nothing else.
    #[test]
    fn emitting_the_blocks_adds_no_drift() {
        let dir = one_extracted_rule("no-new-drift");
        let text = check_in(&dir, &[])
            .map(|c| c.output.text)
            .unwrap_or_default();
        assert!(text.contains(check::NO_RUNNER), "{text}");
        assert!(!text.contains(check::BAD_TRIGGER_BLOCK), "{text}");
        assert!(!text.contains(check::NO_REFUSAL_CLAUSE), "{text}");
    }

    fn set_runner_timeout(dir: &Path, ms: u64) {
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            format!(
                "[sources]\nroots = [\".\"]\n\n[triggers]\nrunner_timeout_ms = {ms}\n"
            ),
        );
    }

    /// And a GATE-ONLY rule must not start firing as a side effect of
    /// being called legal. `loads` stays false, so the hook says nothing.
    #[test]
    fn a_gate_only_rule_still_does_not_fire() {
        let dir = one_extracted_rule("gate-only-quiet");
        assert_eq!(hook_in(&dir, "a.rs"), serde_json::json!({}));
    }

    #[test]
    fn recall_is_dispatched() {
        assert_eq!(
            decide(&args(&["recall", "editing"])),
            Action::Recall(args(&["editing"]))
        );
    }

    /// `reported` routes only the verbs that answer with an `Output`.
    /// The catch-all is unreachable through `perform`, which is why it is
    /// asserted HERE: an arm nothing can reach and nothing tests is an
    /// arm that silently rots into the wrong behaviour.
    #[test]
    fn a_verb_that_does_not_report_an_output_says_so() {
        let said = reported(Action::PrintVersion, &env()).err();
        assert!(
            said.is_some_and(|s| s.contains("does not report an Output")),
            "the catch-all changed shape"
        );
    }

    /// A reader that fails mid-line is an ERROR, not a silent yes.
    #[test]
    fn a_failing_reader_is_not_consent() {
        struct Broken;
        impl std::io::Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("no"))
            }
        }
        let mut input = std::io::BufReader::new(Broken);
        assert!(read_answer(&mut input).is_err());
    }

    /// V41. A `runner` names a GATE STEP, and only an `M` rule has one.
    /// Ignoring it would read, to whoever filled it in, exactly like
    /// honouring it -- so the plan is refused and the message says which
    /// field to clear.
    #[test]
    fn a_runner_on_a_skill_step_is_refused() {
        let dir = plan_project("runner-on-skill");
        let plan_path = dir.join("x.plan");
        let _ = std::fs::write(
            &plan_path,
            "format = 1\nfingerprint = []\n\n[[steps]]\nid = \"abc1234\"\nsrc = \"CLAUDE.md\"\nline_start = 1\nline_end = 1\ntext = \"- when editing `.rs`, prefer modules\"\nlabel = \"S1\"\nartifact = \".claude/skills/x/SKILL.md\"\nrunner = \"ascii\"\nwiring = \"\"\n",
        );
        let result =
            apply_in(&dir, &["--auto-approve", &plan_path.to_string_lossy()]);
        let message = result.err().unwrap_or_default();
        assert!(message.contains("runner"), "{message}");
        assert!(message.contains("GATE STEP"), "{message}");
    }

    /// V28: a config with no roots is a SETUP problem, and the message
    /// names the command that fixes it rather than reporting an empty
    /// corpus as if that were a normal answer.
    #[test]
    fn a_config_with_no_roots_names_the_command_that_fixes_it() {
        let dir = PathBuf::from("target").join("cli-check").join("no-roots");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ =
            std::fs::write(dir.join("rekall.toml"), "[sources]\nroots = []\n");
        let failed =
            scan_command(&args(&["-C", &dir.to_string_lossy()]), &env());
        let said = failed.err().unwrap_or_default();
        assert!(said.contains("rekall init"), "{said}");
    }

    /// A Bash payload, whose text lives in the COMMAND rather than in a
    /// prompt. This is the shape B7 was invisible in.
    fn command_payload(dir: &Path, command: &str) -> String {
        format!(
            "{{\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"Bash\",\
             \"tool_input\":{{\"command\":\"{command}\"}},\"cwd\":\"{}\"}}",
            dir.to_string_lossy()
        )
    }

    /// B7, and V18 where it actually broke. A `word` trigger is tested
    /// against the situation TEXT, and a tool call carries no prompt -- so
    /// reading `prompt` alone made every `word` trigger dead on the one
    /// event `hook` runs on, while `recall`, which takes the situation as
    /// an argument, said `load` for the same skill.
    ///
    /// ONE payload drives BOTH verbs here. Two tests that each build their
    /// own input is how the two paths drifted while both looked right.
    #[test]
    fn a_word_trigger_fires_on_what_the_tool_was_asked_to_run() {
        let dir = recall_project("word-command", "word = [\"cargo test\"]", "");
        let printed = recall_in(&dir, &["--tool", "Bash", "cargo test"]);
        let decided = decided(
            &command_payload(&dir, "cargo test"),
            &dir,
            hook::Agent::Claude,
        );
        assert!(printed.starts_with("load"), "{printed}");
        assert!(
            decided.get("hookSpecificOutput").is_some(),
            "recall said load and hook said nothing: {decided}"
        );
    }

    /// And the do-not-fire clause still wins over the command text.
    #[test]
    fn a_refusal_clause_wins_over_the_command_text() {
        let dir = recall_project(
            "word-refused",
            "word = [\"cargo test\"]",
            "word = [\"--list\"]",
        );
        let decided = decided(
            &command_payload(&dir, "cargo test --list"),
            &dir,
            hook::Agent::Claude,
        );
        assert_eq!(decided, serde_json::json!({}), "{decided}");
    }

    /// V43, at the place it costs. What reaches the model is the RULE, not
    /// the file that carries it -- MEASURED on this crate's own skill, 330
    /// tokens of file to say 46, paid at the fire point on every match.
    #[test]
    fn the_hook_injects_the_payload_and_not_the_scaffold() {
        let dir = recall_project("payload-only", "path = [\"**/*.rs\"]", "");
        let context = hook_in(&dir, "a.rs")
            .pointer("/hookSpecificOutput/additionalContext")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(context.contains("never commit"), "{context}");
        assert!(!context.contains("Fires when"), "{context}");
        assert!(!context.contains("rekall:payload"), "{context}");
    }
}
