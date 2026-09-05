//! What an extraction WRITES, and the marks a reader finds in it.
//!
//! Split out of `apply` when `check` was importing eight symbols from it:
//! `artifact_text`, `payload_of`, and the five heading and payload marks.
//! Those are not `apply`'s private business -- they are the artifact's
//! FORMAT, and it has four readers. `apply` writes one, `check` verifies
//! one, `issue` repairs one and `revert` removes one. A subject with four
//! readers living inside the module that happens to write it is a boundary
//! nobody drew on purpose.
//!
//! It is a FILE and not a module directory. Every `src/<module>/` here
//! carries a `SPEC.md` and is a federated node, and a 24th node was costed
//! at ~1,573 tokens of nav residue to relieve ~937 -- a 1.7x loss, and the
//! new node would have landed back at its own ceiling. `check` made the
//! same trade earlier and for the same reason (`head.rs`, `notes.rs`,
//! `delivery.rs`), so the rules for this file stay in `src/apply/SPEC.md`.

use crate::plan;

/// The artifact a step materializes.
///
/// Both forms arrive INERT and LOUD. A generated `M` script exits nonzero
/// saying it is unimplemented, so wiring it into the gate before writing
/// the check fails visibly rather than passing green; a generated `S` skill
/// carries a trigger heading and an explicit do-not-fire heading, because
/// V4 wants absence stated rather than inferred and a template that omits
/// it teaches the omission.
#[must_use]
pub fn text(step: &plan::Step) -> String {
    if step.label.starts_with('M') {
        return rule_script(step);
    }
    skill_file(step)
}

fn rule_script(step: &plan::Step) -> String {
    fill(RULE_TEMPLATE, step)
}

fn skill_file(step: &plan::Step) -> String {
    fill(SKILL_TEMPLATE, step)
}

/// What a generated runner says when the gate ALREADY enforces the rule.
///
/// The script still arrives inert: naming where the check comes from is
/// not the same as having moved it, and a runner that passed because a
/// comment described one would gate nothing (V2, V22).
fn runner_note(step: &plan::Step) -> String {
    if step.runner.is_empty() {
        return String::new();
    }
    let name = &step.runner;
    let artifact = &step.artifact;
    format!(
        "# MOVE THE CHECK HERE. Gate step `{name}` already enforces this\n\
         # rule. Move its body into this script and leave that step calling\n\
         # `sh {artifact}` -- one definition, many callers (V41, V23).\n#\n"
    )
}

/// The statement, with EVERY line commented (V42).
///
/// A wrapped bullet is the normal case in real prose, and its second line
/// lands inside a shell script. Unprefixed, that line is a COMMAND: the
/// first extraction this crate ever made of its own corpus produced
/// `line 6: here.: command not found` (B6). The corpus is input, and the
/// one place this crate quotes it is the one place that has to quote it.
#[must_use]
pub fn commented(text: &str) -> String {
    text.trim()
        .lines()
        .map(|line| format!("# {}", line.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The artifact's own name, taken from the directory it lands in.
///
/// The HOST indexes a skill by its folder, so the frontmatter name and the
/// path have to agree or the same skill answers to two names.
fn slug_of(artifact: &str) -> String {
    skill_slug(artifact).unwrap_or_else(|| "rekall-skill".to_string())
}

/// `.rekall/skills/<slug>/SKILL.md` -> `<slug>`, or nothing.
///
/// ONE definition, three callers (V41): the frontmatter name above, the
/// host link `apply` publishes, and the link `revert` and `issue` remove.
/// Those were three copies of four lines, which is the shape that lets one
/// of them drift while the other two keep working.
#[must_use]
pub fn skill_slug(artifact: &str) -> Option<String> {
    std::path::Path::new(artifact)
        .parent()
        .and_then(std::path::Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
}

/// The one line the HOST decides by (V43).
///
/// The statement itself, flattened and quoted. It is not a summary -- this
/// crate has no model and will not write one (V5) -- and the rule read as
/// prose is a better description than anything mechanical would invent.
///
/// DOUBLE-QUOTED because a rule says things like "Commit straight to
/// `main`. No feature branches": a colon followed by a space ends a plain
/// YAML scalar, and the host would read a truncated description or fail to
/// parse the file it was supposed to index.
fn summary_of(text: &str) -> String {
    let flat = crate::statement::normalize(text);
    let capped: String = flat.chars().take(SUMMARY_LEN).collect();
    let escaped = capped.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Long enough for a rule, short enough for a listing.
const SUMMARY_LEN: usize = 160;

/// Templates are CONSTS, not inline format walls. The generated artifact is
/// the thing a human edits next, so its text should be readable and
/// editable here rather than reassembled from fragments.
fn fill(template: &str, step: &plan::Step) -> String {
    template
        .replace("{RUNNER_NOTE}\n", &runner_note(step))
        .replace("{SLUG}", &slug_of(&step.artifact))
        .replace("{SUMMARY}", &summary_of(&step.text))
        .replace("{TEXT_SH}", &commented(&step.text))
        .replace("{ID}", &step.id)
        .replace("{SRC}", &step.src)
        .replace("{START}", &step.line_start.to_string())
        .replace("{END}", &step.line_end.to_string())
        .replace("{ARTIFACT}", &step.artifact)
        .replace("{TEXT}", step.text.trim())
}

/// What a generated runner says about itself until someone writes the
/// check, and the two headings a generated skill must end up carrying.
///
/// CONSTS because `check` reads them back out of the artifact (V2, V3,
/// V4). A template and the gate that judges it are two halves of one rule,
/// and two string literals that must match is the invisible kind of
/// divergence: each looks right alone, and the gate silently stops
/// noticing the placeholder it was written to catch. The tests below pin
/// each const to the template that carries it.
pub const UNIMPLEMENTED: &str = "is not implemented yet";

/// The PAYLOAD markers (V43), spelled for each file the templates write.
///
/// An artifact has three readers and they want different things: `hook`
/// injects the payload, `check` reads the scaffold around it, the host
/// indexes the head. Without a mark, the only thing a reader can take is
/// the whole file -- MEASURED, 318 tokens delivered to say 36.
///
/// Comment syntax in both cases, so the mark is invisible to the reader
/// the file is FOR: a shell script ignores a `#` line, and markdown does
/// not render an HTML comment.
pub const PAYLOAD_OPEN: [&str; 2] =
    ["# rekall:payload", "<!-- rekall:payload -->"];
pub const PAYLOAD_CLOSE: [&str; 2] =
    ["# rekall:/payload", "<!-- rekall:/payload -->"];

/// The rule an artifact carries, without the scaffold around it (V43).
///
/// `None` when the artifact is unmarked, which is a GATE finding rather
/// than something to paper over here: `hook` still has a rule to deliver,
/// and withholding it in the request path would turn a reporting problem
/// into a missing rule at the moment it mattered.
#[must_use]
pub fn payload_of(text: &str) -> Option<String> {
    let open = line_at(text, &PAYLOAD_OPEN)?;
    let close = line_at(text, &PAYLOAD_CLOSE)?;
    if close <= open {
        return None;
    }
    let inner: Vec<String> = text
        .lines()
        .skip(open.saturating_add(1))
        .take(close.saturating_sub(open).saturating_sub(1))
        .map(uncomment)
        .collect();
    Some(inner.join("\n").trim().to_string())
}

/// The payload of a SHELL artifact is commented (V42), so the marks come
/// off with it. A markdown payload has no prefix and is left alone.
fn uncomment(line: &str) -> String {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix("# ")
        .or_else(|| trimmed.strip_prefix('#'))
        .unwrap_or(line)
        .to_string()
}

fn line_at(text: &str, marks: &[&str]) -> Option<usize> {
    text.lines()
        .position(|line| marks.iter().any(|mark| line.trim() == *mark))
}
pub const FIRES: &str = "## Fires when";
pub const NOT_FIRES: &str = "## Does NOT fire when";

/// Arrives INERT and LOUD: exits nonzero until the check is written, so
/// wiring it into the gate before implementing it fails visibly rather
/// than passing green.
const RULE_TEMPLATE: &str = "\
#!/bin/sh
# Extracted by rekall from {SRC}:{START}-{END} (id {ID}).
#
# THE RULE, verbatim:
# rekall:payload
{TEXT_SH}
# rekall:/payload
#
{RUNNER_NOTE}
# Exits NONZERO until the check is written. A runner that passes without
# testing anything gates nothing, and is worse than no runner (V2, V22).
#
# ## Fires when
#
# This rule already gates at COMMIT -- that is what the wiring line in
# `rekall plan` asks you to do. The block below is different: it makes the
# rule arrive at a TOOL CALL too, before the mistake instead of after
# (V37).
#
# It arrives EMPTY, which means GATE-ONLY: nothing fires it early until
# you say when. Keys are `tool` (exact names), `path` (globs) and `word`
# (literals tested against the situation text). Within a key ANY value
# matches; across keys ALL present keys must match.
#
# ```rekall
# tool = []
# path = []
# word = []
# ```
#
# ## Does NOT fire when
#
# A match here refuses the load even when the block above matched. State
# the absence rather than leaving it inferred (V4).
#
# ```rekall
# tool = []
# path = []
# word = []
# ```
echo 'rekall: {ARTIFACT} is not implemented yet' >&2
exit 1
";

/// Carries BOTH headings. V4 wants absence stated rather than inferred, so
/// a template that omits the do-not-fire section teaches the omission.
const SKILL_TEMPLATE: &str = "\
---
name: {SLUG}
description: {SUMMARY}
disable-model-invocation: true
---

Extracted by rekall from {SRC}:{START}-{END} (id {ID}).

<!-- rekall:payload -->
{TEXT}
<!-- rekall:/payload -->

## Fires when

The block below IS the trigger. Prose here is for you and is never read
(V29). Keys: `tool` (exact names), `path` (globs), `word` (literals tested
against the situation text). Within a key ANY value matches; across keys
ALL present keys must match.

It arrives EMPTY, which matches nothing -- so this skill does not load
until you say when. A trigger nobody filled in should fire never, not
always: always-on prose is what this was extracted FROM (V3).

```rekall
tool = []
path = []
word = []
```

## Does NOT fire when

State the absence rather than leaving it inferred (V4). A list of what
fires says nothing about what does not, and a matcher has to decide both.
This block WINS: a match here refuses the load even when the block above
matched.

```rekall
tool = []
path = []
word = []
```
";

#[cfg(test)]
mod tests {
    use super::super::pointer_of;
    use super::super::testing::step;
    use super::*;

    /// V2: a runner that passes without testing anything gates nothing.
    /// The generated script fails until someone writes the check.
    #[test]
    fn a_generated_rule_script_exits_nonzero() {
        let text = text(&step("abc1234", 1, 1, "M1"));
        assert!(text.starts_with("#!/bin/sh"), "{text}");
        assert!(text.contains("exit 1"), "{text}");
        assert!(
            text.contains("- never commit to `main`"),
            "the rule is missing: {text}"
        );
    }
    /// V37, DISCOVERABLE. T39 shipped `M`-rule firing and the generated
    /// artifact said nothing about it, so the feature could only be found
    /// by reading the spec. Both blocks are now emitted, EMPTY -- which is
    /// gate-only, so behaviour is unchanged until someone fills one in.
    ///
    /// Commented, because this artifact is a SCRIPT: uncommented text
    /// would be executed. V29's block, in a file that runs.
    #[test]
    fn the_generated_runner_carries_two_parsable_empty_blocks() {
        let script = text(&step("abc1234", 1, 1, "M1"));
        for heading in [FIRES, NOT_FIRES] {
            let held = crate::trigger::parse_block(&script, heading);
            assert_eq!(
                held.as_ref().map(crate::trigger::Trigger::is_empty),
                Ok(true),
                "{heading} did not parse to an empty block: {held:?}"
            );
        }
    }
    /// The blocks are COMMENTED, or the shell would try to run them. A
    /// runner that fails on its own trigger block is worse than one with
    /// no block at all.
    #[test]
    fn the_generated_runner_keeps_its_blocks_commented() {
        let script = text(&step("abc1234", 1, 1, "M2"));
        for line in script.lines() {
            let held = line.trim();
            assert!(
                !held.starts_with("```") && !held.starts_with("## "),
                "an uncommented block line would execute: {held}"
            );
        }
    }
    /// V39: this costs NO window. The artifact is never always-on -- only
    /// the pointer is -- so the explanation is free where a longer pointer
    /// would not have been.
    #[test]
    fn the_runner_is_bigger_than_the_pointer_and_that_is_fine() {
        let script = text(&step("abc1234", 1, 1, "M1"));
        assert!(script.len() > pointer_of("abc1234").len() * 10);
    }
    /// V3 and V4: a trigger AND an explicit do-not-fire clause. A template
    /// that omitted the second would teach the omission.
    #[test]
    fn a_generated_skill_carries_both_headings() {
        let text = text(&step("abc1234", 1, 1, "S2"));
        assert!(text.contains(FIRES), "{text}");
        assert!(text.contains(NOT_FIRES), "{text}");
    }
    /// PINS the template to the const `check` reads back. If the wording
    /// moved on one side only, the gate would stop noticing the very
    /// placeholder it exists to catch -- and would report green.
    #[test]
    fn the_markers_check_reads_are_the_ones_the_templates_write() {
        assert!(
            text(&step("a", 1, 1, "M1")).contains(UNIMPLEMENTED),
            "the runner template no longer announces itself unimplemented"
        );
        let skill = text(&step("a", 1, 1, "S1"));
        assert!(skill.contains(FIRES) && skill.contains(NOT_FIRES));
    }
    /// V29: what `apply` writes must PARSE, and must arrive EMPTY. A
    /// template whose block did not parse would make every fresh
    /// extraction unreadable rather than merely unfinished, and one that
    /// arrived non-empty would load the skill somewhere nobody chose.
    #[test]
    fn the_generated_skill_carries_two_parsable_empty_blocks() {
        let skill = text(&step("abc1234", 1, 1, "S2"));
        for heading in [FIRES, NOT_FIRES] {
            let held = crate::trigger::parse_block(&skill, heading);
            assert_eq!(
                held.as_ref().map(crate::trigger::Trigger::is_empty),
                Ok(true),
                "{heading} did not parse to an empty block: {held:?}"
            );
        }
    }
    #[test]
    fn every_sharpness_of_m_generates_a_script() {
        for label in ["M1", "M2", "M3"] {
            assert!(
                text(&step("a", 1, 1, label)).starts_with("#!/bin/sh"),
                "{label}"
            );
        }
    }
    /// V41. The generated script says WHERE the check comes from -- and
    /// still exits nonzero, because naming a body is not moving one.
    #[test]
    fn a_named_runner_puts_the_move_in_the_script() {
        let mut with = step("a", 1, 1, "M1");
        with.runner = "ascii".to_string();
        let text = text(&with);
        assert!(text.contains("MOVE THE CHECK HERE"), "{text}");
        assert!(text.contains("`ascii`"), "{text}");
        assert!(text.contains(UNIMPLEMENTED), "{text}");
        assert!(text.contains("exit 1"), "{text}");
    }
    /// The default script must not mention a move it is not making, and
    /// must not leave the placeholder showing.
    #[test]
    fn an_empty_runner_leaves_no_note_and_no_placeholder() {
        let text = text(&step("a", 1, 1, "M1"));
        assert!(!text.contains("MOVE THE CHECK HERE"), "{text}");
        assert!(!text.contains("RUNNER_NOTE"), "{text}");
    }
    /// V42, B6. A wrapped statement is the normal case in real prose, and
    /// its second line lands inside a shell script -- where, unprefixed,
    /// it is a command. Every line carries the comment marker.
    #[test]
    fn every_line_of_a_wrapped_statement_is_commented() {
        let mut wrapped = step("a", 1, 2, "M1");
        wrapped.text =
            "- Rust source is ASCII only. `SPEC.md` symbols are FORMAT\n  and do not apply here."
                .to_string();
        let text = text(&wrapped);
        assert!(text.contains("# and do not apply here."), "{text}");
        assert!(!text.contains("\nand do not apply here."), "{text}");
    }
    /// The helper alone, so the rule is legible without reading a template
    /// around it: blank lines and indentation do not survive as commands.
    #[test]
    fn commenting_covers_every_line_it_is_given() {
        let out = commented("first\n  second\n\tthird");
        for line in out.lines() {
            assert!(line.starts_with("# "), "{out}");
        }
    }
    /// V43. The payload comes out WITHOUT the scaffold around it, and a
    /// shell payload loses the comment marks V42 put on every line.
    #[test]
    fn the_payload_comes_out_without_its_scaffold() {
        let script = text(&step("a", 1, 1, "M1"));
        let inner = payload_of(&script).unwrap_or_default();
        assert_eq!(inner, "- never commit to `main`", "{script}");
        let skill = text(&step("a", 1, 1, "S1"));
        let inner = payload_of(&skill).unwrap_or_default();
        assert_eq!(inner, "- never commit to `main`", "{skill}");
    }
    /// An artifact with no marks yields NOTHING rather than a guess. The
    /// gate reports that (V43); guessing where a rule ends would put the
    /// scaffold back in by another route.
    #[test]
    fn an_unmarked_artifact_has_no_payload() {
        assert_eq!(payload_of("#!/bin/sh\nexit 0\n"), None);
        assert_eq!(payload_of(""), None);
    }
    /// Marks in the wrong ORDER are not a payload either.
    #[test]
    fn a_closing_mark_before_the_opening_one_is_not_a_payload() {
        let upside_down =
            "<!-- rekall:/payload -->\nx\n<!-- rekall:payload -->\n";
        assert_eq!(payload_of(upside_down), None);
    }
    /// V43. The HEAD is what the host indexes, so a generated skill
    /// arrives with a name matching its folder and a description that is
    /// the rule itself. Before this, the first extraction listed under its
    /// hash.
    #[test]
    fn a_generated_skill_carries_a_head_the_host_can_index() {
        let mut skill = step("a", 1, 1, "S1");
        skill.artifact =
            ".claude/skills/never-commit-to-main/SKILL.md".to_string();
        let text = text(&skill);
        assert!(text.starts_with("---\n"), "{text}");
        assert!(text.contains("name: never-commit-to-main"), "{text}");
        assert!(text.contains("description: \"never commit"), "{text}");
        assert!(
            !text.contains("# a\n"),
            "the id is no longer the title: {text}"
        );
    }
    /// A rule says things like "Commit straight to `main`. No feature
    /// branches" -- a colon and a space would end a plain YAML scalar, so
    /// the description is quoted and its own quotes are escaped. A host
    /// that cannot parse the head cannot index the skill.
    #[test]
    fn a_description_survives_the_punctuation_a_rule_contains() {
        let mut skill = step("a", 1, 1, "S1");
        skill.artifact = ".claude/skills/x/SKILL.md".to_string();
        skill.text = "- Say \"no\": a rule with quotes: and colons".to_string();
        let text = text(&skill);
        assert!(
            text.contains("description: \"Say \\\"no\\\": a rule"),
            "{text}"
        );
    }
    /// A shell runner has no head. Nothing indexes `.rekall/rules`, and a
    /// frontmatter block there is scaffold with no reader.
    #[test]
    fn a_generated_runner_has_no_head() {
        let text = text(&step("a", 1, 1, "M1"));
        assert!(text.starts_with("#!/bin/sh"), "{text}");
    }
}
