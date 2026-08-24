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
) -> Result<serde_json::Value, String> {
    let payload = hook::parse(stdin);
    // The runner's bound comes from config (B5). Read here at the edge, so
    // everything below stays a function of its arguments.
    let limit = runner::limit_from(
        resolve(base).ok().and_then(|held| held.runner_timeout_ms),
    );
    let path = ledger::path_in(base);
    let held = ledger::load(&path).map_err(|e| e.to_string())?;
    let texts = read_artifacts(base, &held);
    let candidates = zip_candidates(&held, &texts);
    let report = recall::decide(&candidates, &hook::situation(&payload));
    let loading = hook::loading(&report);
    count_fires(&path, &loading);
    let said = advice(&loading, &held, &texts, &At { base, limit });
    Ok(hook::decision(payload.hook_event_name.as_deref(), &said))
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
        return text.map(ToString::to_string);
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
    if !flags.is_empty() {
        eprintln!(
            "rekall: `hook` takes no flags -- it reads one payload on stdin"
        );
        return USAGE_EXIT;
    }
    let mut stdin = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut stdin);
    let decision = hook_command(&stdin, &env.cwd)
        .unwrap_or_else(|_| serde_json::json!({}));
    println!("{decision}");
    0
}
