//! The CLI contract, tested from OUTSIDE the crate.
//!
//! Every other test in this repository calls a Rust function. These call
//! the binary, because the things a consumer actually depends on -- the
//! exit codes, the JSON anatomy, whether a report-only verb writes -- are
//! properties of the PROGRAM. A function returning the right value while
//! `main` maps it to the wrong exit code passes every unit test in the
//! crate and still breaks every script that calls it.
//!
//! NO `unwrap`, `expect`, `panic!` or `assert!(false)`, under the same
//! lints as the rest of the crate. Helpers return a sentinel on failure --
//! an exit code of `-1`, an empty string -- and every assertion is written
//! so the sentinel fails it. A broken fixture fails its own test on the
//! next line instead of aborting the run somewhere else.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The binary cargo just built for this test target. Not a PATH lookup:
/// that would silently test whatever `rekall` is installed on the machine,
/// which on a developer's box is the stale copy these tests exist to catch.
const BIN: &str = env!("CARGO_BIN_EXE_rekall");

/// Four statements spanning both classes, so a change that collapses the
/// ladder shows up here and not only in the unit tests.
const CORPUS: &str = "\
# Working on acme

## Conventions

- Never commit a `.env` file.
- When a migration touches a table with more than a million rows, take the
  backup first and say in the PR how long the restore took.
";

/// Exit code, stdout, stderr. `-1` means the process never ran.
///
/// HOME is redirected into the fixture so `init` cannot discover the
/// developer's real `~/.claude` and report roots that differ per machine.
/// A test whose result depends on who runs it is not a test.
fn cmd(dir: &Path, args: &[&str]) -> Command {
    let mut built = Command::new(BIN);
    built
        .args(args)
        .current_dir(dir)
        .env("HOME", dir)
        .env("XDG_CONFIG_HOME", dir.join("config"));
    built
}

fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let Ok(out) = cmd(dir, args).output() else {
        return (-1, String::new(), String::new());
    };
    let code = out.status.code().unwrap_or(-1);
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    let cried = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, said, cried)
}

/// A fixture directory holding the corpus above.
///
/// Named by process id and a tag so no two tests share a directory.
/// Removed first, in case an earlier run died before cleanup: reusing a
/// stale directory would test whatever it left behind.
fn fixture(tag: &str) -> PathBuf {
    let at = std::env::temp_dir()
        .join(format!("rekall-it-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let _ = std::fs::write(at.join("CLAUDE.md"), CORPUS);
    at
}

fn cleanup(at: &Path) {
    let _ = std::fs::remove_dir_all(at);
}

/// The corpus as it currently stands, or an empty string.
fn corpus(at: &Path) -> String {
    std::fs::read_to_string(at.join("CLAUDE.md")).unwrap_or_default()
}

/// The first id `scan` reports, or `""`. Callers assert it is non-empty,
/// so an empty fixture fails on the assertion rather than here.
fn first_id(rows: &str) -> String {
    rows.split_whitespace().next().unwrap_or("").to_owned()
}

/// A fixture with `init` already run.
fn ready(tag: &str) -> PathBuf {
    let at = fixture(tag);
    let _ = run(&at, &["init"]);
    at
}

/// Feed `hook` a payload on stdin and read its decision back.
fn hook(dir: &Path, payload: &[u8]) -> (i32, String) {
    let Ok(mut child) = cmd(dir, &["hook"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
    else {
        return (-1, String::new());
    };
    write_stdin(&mut child, payload);
    let out = child.wait_with_output().ok();
    let code = out.as_ref().and_then(|o| o.status.code()).unwrap_or(-1);
    let said = out.map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    (code, said.unwrap_or_default())
}

fn write_stdin(child: &mut std::process::Child, payload: &[u8]) {
    if let Some(mut sink) = child.stdin.take() {
        use std::io::Write as _;
        let _ = sink.write_all(payload);
    }
}

// ------------------------------------------------------------------ usage

/// A BARE INVOCATION prints the verb list. What it EXITS with is
/// deliberately not asserted.
///
/// `SPEC.md` §I says `0 ok · 1 drift/violation · 2 usage` and does not say
/// which of those a missing verb is. Today it prints help and exits 0,
/// which is defensible -- `cargo` does the same -- and so is exiting 2 on
/// the grounds that no work was requested. Neither reading is written down.
///
/// So this asserts only what the spec actually promises: a user who types
/// the bare name learns what the verbs are. Asserting an exit code would
/// be inventing law in a test file, which is the inverse of how this
/// repository decides things. The gap belongs in `SPEC.md` through
/// `/spec`, and this test tightens once it is settled.
#[test]
fn a_bare_invocation_names_the_verbs() {
    let at = fixture("noargs");
    let (_, out, err) = run(&at, &[]);
    let said = format!("{out}{err}");
    assert!(said.contains("usage"), "a bare invocation must show usage");
    assert!(said.contains("scan"), "usage must list the verbs");
    cleanup(&at);
}

#[test]
fn an_unknown_verb_is_a_usage_error() {
    let at = fixture("unknownverb");
    let (code, _, _) = run(&at, &["teleport"]);
    assert_eq!(code, 2, "an unknown verb must exit 2");
    cleanup(&at);
}

#[test]
fn an_unknown_format_is_refused_not_defaulted() {
    let at = fixture("badformat");
    let (code, _, _) = run(&at, &["scan", "--format", "yaml"]);
    assert_eq!(code, 2, "an unknown --format must be refused");
    cleanup(&at);
}

#[test]
fn help_exits_zero_and_names_the_tool() {
    let at = fixture("help");
    let (code, out, _) = run(&at, &["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("rekall"), "help must name the tool");
    cleanup(&at);
}

// ----------------------------------------------------------- JSON anatomy

/// README.md promises an agent never has to string-surgery `M2` back into
/// its parts. These fields are that promise, and losing one is a silent
/// break for every consumer parsing the output rather than reading it.
#[test]
fn scan_json_separates_class_from_sharpness() {
    let at = ready("jsonanatomy");
    let (code, out, _) = run(&at, &["scan", "--format", "json"]);
    assert_eq!(code, 0, "scan is report-only and must exit 0");
    for field in [
        "\"id\"",
        "\"src\"",
        "\"class\"",
        "\"sharpness\"",
        "\"label\"",
    ] {
        assert!(out.contains(field), "scan json must carry {field}");
    }
    cleanup(&at);
}

#[test]
fn scan_finds_both_classes_in_the_fixture() {
    let at = ready("bothclasses");
    let (code, out, _) = run(&at, &["scan"]);
    assert_eq!(code, 0);
    assert!(out.contains(" M"), "the .env rule must classify mechanical");
    assert!(
        out.contains(" S"),
        "the migration rule must classify situational"
    );
    cleanup(&at);
}

// ------------------------------------------------------------ report-only

#[test]
fn report_only_verbs_do_not_touch_the_corpus() {
    let at = ready("reportonly");
    let before = corpus(&at);
    assert!(!before.is_empty(), "the fixture corpus must be readable");
    for verb in [&["scan"][..], &["check"][..], &["log"][..]] {
        let _ = run(&at, verb);
    }
    assert_eq!(before, corpus(&at), "a report-only verb rewrote the corpus");
    cleanup(&at);
}

// ------------------------------------------------------------------- loop

/// THE POINT OF THE TOOL, end to end. After `apply` the statement has left
/// the corpus and its artifact is a placeholder, so the rule is enforced
/// by nothing -- strictly worse than never extracting it. That state must
/// be loud, and it must be reversible.
#[test]
fn the_gate_refuses_an_unfinished_extraction() {
    let at = ready("loop");
    let (clean, _, _) = run(&at, &["check"]);
    assert_eq!(clean, 0, "nothing extracted is not drift");
    let (_, rows, _) = run(&at, &["scan"]);
    let id = first_id(&rows);
    assert!(!id.is_empty(), "scan produced no rows to extract");
    let (applied, _, _) = run(&at, &["apply", &id, "--auto-approve"]);
    assert_eq!(applied, 0, "apply must succeed");
    let (drift, out, _) = run(&at, &["check"]);
    assert_eq!(drift, 1, "an unfinished extraction must exit 1");
    assert!(
        !out.is_empty(),
        "drift must be described, not only signalled"
    );
    cleanup(&at);
}

#[test]
fn revert_restores_the_corpus_byte_for_byte() {
    let at = ready("revert");
    let (_, rows, _) = run(&at, &["scan"]);
    let id = first_id(&rows);
    assert!(!id.is_empty(), "scan produced no rows to extract");
    let _ = run(&at, &["apply", &id, "--auto-approve"]);
    let (code, _, _) = run(&at, &["revert", &id, "--auto-approve"]);
    assert_eq!(code, 0, "revert must succeed");
    assert_eq!(corpus(&at), CORPUS, "revert must restore verbatim");
    let (after, _, _) = run(&at, &["check"]);
    assert_eq!(after, 0, "a reverted extraction leaves no drift");
    cleanup(&at);
}

/// There is no tty in a test, so the prompt cannot be answered. Refusing is
/// the only safe reading: a script run without a terminal must not be able
/// to rewrite somebody's memory files by default.
#[test]
fn apply_off_a_tty_demands_auto_approve() {
    let at = ready("consent");
    let (_, rows, _) = run(&at, &["scan"]);
    let id = first_id(&rows);
    assert!(!id.is_empty(), "scan produced no rows");
    let (code, _, _) = run(&at, &["apply", &id]);
    assert_ne!(
        code, 0,
        "apply off a tty without --auto-approve must refuse"
    );
    assert_eq!(corpus(&at), CORPUS, "a refused apply must not have written");
    cleanup(&at);
}

// ------------------------------------------------------------------- hook

/// `hook` sits in the request path of every tool call. A harness that
/// changed its payload shape must get "nothing loads", never a crash --
/// the blast radius of panicking here is every tool use, not one report.
#[test]
fn hook_answers_an_unreadable_payload_with_an_empty_decision() {
    let at = ready("hookjunk");
    let (code, said) = hook(&at, b"this is not json at all");
    assert_eq!(code, 0, "an unreadable payload is not a failure");
    assert!(
        said.contains('{'),
        "hook must still answer with a JSON object"
    );
    cleanup(&at);
}

#[test]
fn hook_injects_nothing_when_nothing_was_extracted() {
    let at = ready("hookempty");
    let payload = br#"{"hook_event_name":"PreToolUse","tool_name":"Read"}"#;
    let (code, said) = hook(&at, payload);
    assert_eq!(code, 0, "hook must never fail a tool call");
    assert!(!said.contains("additionalContext"), "nothing to inject");
    cleanup(&at);
}

/// The README says, in those words, that every console block in it is real
/// output. That is a CLAIM about this binary, and until now nothing read it.
/// Two of those blocks had drifted -- an `init` example printing a paragraph
/// that only appears when a user root is FOUND while showing none, and a
/// `check` example carrying a message the code stopped producing when V56
/// learned to name its own blind spot.
///
/// The README's own corpus, run through the README's own commands, and the
/// README must CONTAIN what comes back. `contains` rather than equality
/// because the block sits inside prose and a fence; what matters is that the
/// bytes a reader will copy are bytes this binary produced.
#[test]
fn the_readme_scan_block_is_real_output() {
    let at = fixture("readme");
    let _ = std::fs::write(at.join("CLAUDE.md"), README_CORPUS);
    let _ = run(&at, &["init"]);
    let (code, out, _) = run(&at, &["scan"]);
    cleanup(&at);
    assert_eq!(code, 0, "scan failed on the README's own corpus");
    assert!(!out.is_empty(), "scan produced nothing");
    let readme = std::fs::read_to_string("README.md").unwrap_or_default();
    for line in out.lines() {
        assert!(
            readme.contains(line.trim_end()),
            "the README's scan block has drifted; this line is not in it:\n  {line}"
        );
    }
}

/// The corpus the README prints above its scan block, byte for byte. If
/// these disagree the ids change and the test above says so.
const README_CORPUS: &str = "\
# Working on acme

## Conventions

- Rust source is ASCII only. Unicode belongs in test fixtures, not in
  identifiers.
- Never commit a `.env` file.
- When a migration touches a table with more than a million rows, take the
  backup first and say in the PR how long the restore took.
- Prefer `anyhow` at the binary edge and `thiserror` in libraries.
";
