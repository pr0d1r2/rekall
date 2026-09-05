//! Test helpers shared by every verb's own test module.
//!
//! Split out of `cli.rs` with the tests themselves (T41). These two are
//! the only helpers that were genuinely universal -- `args` had 113 call
//! sites and `env` 62, spanning every verb -- so they live in one place
//! rather than being copied into eleven modules, which is the duplication
//! V1 exists to remove.
//!
//! Everything else that looked shared was not: `project`, `run_in`,
//! `text_of` and `json_rows` are used only by `scan`'s tests, and
//! `first_scan_id` only by `plan`'s. Those travelled with their section.

#![allow(dead_code)]

use super::apply::apply_command;
use super::check::Checked;
use super::check::check_command;
use super::log::log_command;
use super::recall::recall_command;
use super::revert::revert_command;
use super::scan::scan_command;
use super::{Env, Output};
use crate::{apply, ledger};
use std::path::{Path, PathBuf};

pub(super) fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

/// A deterministic environment. Tests never read the real one: the point
/// of `Env` is that process globals arrive as arguments.
pub(super) fn env() -> Env {
    Env {
        cwd: PathBuf::from("."),
        home: None,
    }
}

pub(super) fn plan_project(name: &str) -> PathBuf {
    let dir = PathBuf::from("target").join("cli-plan").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ =
        std::fs::write(dir.join("rekall.toml"), "[sources]\nroots = [\".\"]\n");
    let _ = std::fs::write(
        dir.join("CLAUDE.md"),
        "- never commit to `main`\n\n- always prefer the simpler option\n",
    );
    dir
}

pub(super) fn apply_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
    let mut flags = args(&["-C", &dir.to_string_lossy()]);
    flags.extend(args(extra));
    apply_command(&flags, &env())
}

pub(super) fn rule_id(dir: &Path) -> String {
    scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
        .map(|out| out.text)
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}

pub(super) fn revert_project(name: &str) -> PathBuf {
    let dir = PathBuf::from("target").join("cli-revert").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ =
        std::fs::write(dir.join("rekall.toml"), "[sources]\nroots = [\".\"]\n");
    let _ = std::fs::write(
        dir.join("CLAUDE.md"),
        "# Rules\n\n- never commit to `main`\n\n- background prose\n",
    );
    dir
}

pub(super) fn revert_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
    let mut flags = args(&["-C", &dir.to_string_lossy()]);
    flags.extend(args(extra));
    revert_command(&flags, &env())
}

/// Extract one statement and hand back its id and the corpus as it
/// stood BEFORE the extraction -- which is the thing a revert has to
/// reproduce.
pub(super) fn extracted(dir: &Path) -> (String, String) {
    extracted_from(dir, &dir.join("CLAUDE.md"))
}

/// The artifact an extracted id landed, read back from the ledger --
/// the tests never hard-code the path, because the slug is derived
/// from the statement's own words.
pub(super) fn artifact_of(dir: &Path, id: &str) -> String {
    ledger::load(&ledger::path_in(dir))
        .unwrap_or_default()
        .find(id)
        .map(|row| row.artifact.clone())
        .unwrap_or_default()
}

/// A corpus whose only source file is one directory down.
pub(super) fn nested_project(name: &str) -> PathBuf {
    let dir = revert_project(name);
    let _ = std::fs::remove_file(dir.join("CLAUDE.md"));
    let _ = std::fs::create_dir_all(dir.join("docs"));
    let _ = std::fs::write(
        dir.join("docs").join("AGENTS.md"),
        "# Rules\n\n- never commit to `main`\n\n- background prose\n",
    );
    dir
}

/// `extracted`, for a corpus file that is not `CLAUDE.md`.
pub(super) fn extracted_from(dir: &Path, src: &Path) -> (String, String) {
    let before = std::fs::read_to_string(src).unwrap_or_default();
    let id = rule_id(dir);
    let mut flags = args(&["-C", &dir.to_string_lossy()]);
    flags.push(id.clone());
    flags.push("--auto-approve".to_string());
    let _ = apply_command(&flags, &env());
    (id, before)
}

/// It RAISES `runner_timeout_ms` far above the default. Tests built on
/// this fixture assert what a rule SAID, and the bound is wall-clock
/// (B5, V38): a loaded box turns a millisecond rule into a killed one,
/// and the suite then reports a failure that never happened. MEASURED
/// twice today, under a concurrent clippy run. That the bound WORKS is
/// tested where it belongs, by a rule that really does hang.
pub(super) fn check_project(name: &str) -> PathBuf {
    let dir = PathBuf::from("target").join("cli-check").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join("rekall.toml"),
        "[sources]\nroots = [\".\"]\n\n[triggers]\nrunner_timeout_ms = 60000\n",
    );
    let _ = std::fs::write(
        dir.join("CLAUDE.md"),
        "# Rules\n\n- never commit to `main`\n\n- background prose\n",
    );
    dir
}

pub(super) fn check_in(dir: &Path, extra: &[&str]) -> Result<Checked, String> {
    let mut flags = args(&["-C", &dir.to_string_lossy()]);
    flags.extend(args(extra));
    check_command(&flags, &env())
}

pub(super) fn dash_c(dir: &Path) -> Vec<String> {
    args(&["-C", &dir.to_string_lossy()])
}

pub(super) fn dead_text(dir: &Path) -> String {
    log_in(dir, &["--dead"]).map(|o| o.text).unwrap_or_default()
}

/// A project with one extracted `S` skill whose blocks are filled.
pub(super) fn recall_project(name: &str, fire: &str, refuse: &str) -> PathBuf {
    let dir = check_project(name);
    let _ = std::fs::write(
        dir.join("CLAUDE.md"),
        "# Rules\n\n- when editing Rust files, run clippy first\n",
    );
    let (id, _) = extracted(&dir);
    let path = dir.join(artifact_of(&dir, &id));
    let _ = std::fs::write(&path, skill_with(fire, refuse));
    dir
}

pub(super) fn recall_in(dir: &Path, extra: &[&str]) -> String {
    let mut flags = args(&["-C", &dir.to_string_lossy()]);
    flags.extend(args(extra));
    recall_command(&flags, &env())
        .map(|out| out.text)
        .unwrap_or_default()
}
pub(super) fn log_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
    let mut flags = args(&["-C", &dir.to_string_lossy()]);
    flags.extend(args(extra));
    log_command(&flags, &env())
}

/// A whole project on disk: a config plus a corpus file. Under
/// `target/` so `cargo clean` removes it, named after its test so two
/// never collide.
pub(super) fn project(name: &str) -> PathBuf {
    let dir = PathBuf::from("target").join("test-project").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let _ =
        std::fs::write(dir.join("rekall.toml"), "[sources]\nroots = [\".\"]\n");
    let _ = std::fs::write(
        dir.join("CLAUDE.md"),
        "- never commit to `main`\n\n- when writing tests, prefer tables\n",
    );
    dir
}

pub(super) fn skill_with(fire: &str, refuse: &str) -> String {
    format!(
        "---\nname: s\ndescription: \"- never commit to `main`\"\n---\n\n<!-- rekall:payload -->\n- never commit to `main`\n\
         <!-- rekall:/payload -->\n\n{}\n\n```rekall\n{fire}\n```\n\n{}\n\n```rekall\n{refuse}\n```\n",
        apply::FIRES,
        apply::NOT_FIRES
    )
}

/// A project with one extracted `M` rule, left exactly as `apply`
/// wrote it -- no trigger block, which is the state B4 misread.
pub(super) fn one_extracted_rule(name: &str) -> PathBuf {
    let dir = check_project(name);
    let _ = std::fs::write(
        dir.join("CLAUDE.md"),
        "# Rules\n\n- never commit to `main`\n",
    );
    let _ = extracted(&dir);
    dir
}
