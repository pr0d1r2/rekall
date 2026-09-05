//! Running an extracted `M` rule.
//!
//! V38 fixes the two things that matter here. A fired rule ADVISES rather
//! than blocking: a wrong rule that blocks costs the user their work, and
//! unwedging it means editing the corpus mid-task. And a runner that does
//! not finish in BOUNDED time did not fire -- this executes in the
//! tool-call path, so a hung script would stall the harness rather than
//! fail it.
//!
//! Nothing is lost by advising. The same runner still GATES at commit
//! (V2's wiring), so the hook is the early word and the gate is the last.

use std::path::Path;
use std::time::{Duration, Instant};

/// How long a rule gets. Generous for a lint, short enough that a wedged
/// script is a blip rather than a hang -- and the wait is polled, so a
/// fast rule costs its own runtime and nothing more.
pub const LIMIT: Duration = Duration::from_millis(2000);

/// The limit from config, or the shipped default.
///
/// The bound is WALL-CLOCK, so it is not really a statement about the
/// rule: it is a statement about the rule AND the machine. On a loaded box
/// a script whose whole body is `echo` can exceed two seconds, be killed,
/// and report a timeout that never happened (B5). Raising it is a
/// legitimate answer for a slow box, which is why it is configurable.
#[must_use]
pub fn limit_from(configured: Option<u64>) -> Duration {
    configured.map_or(LIMIT, Duration::from_millis)
}

/// How often the wait wakes to check. Small enough that a millisecond
/// rule is not rounded up to the poll interval.
const POLL: Duration = Duration::from_millis(5);

/// What running a rule produced.
#[derive(Debug, PartialEq, Eq)]
pub enum Fired {
    /// Exit 0. The rule looked and found nothing.
    Clean,
    /// Nonzero: the rule has something to say. Carries what it SAID, not
    /// its exit code -- a number tells the model nothing.
    Violated(String),
    /// Past the limit, and killed. Reported rather than swallowed: a rule
    /// that always times out is as dead as one that never fires, and V11
    /// only measures the second.
    ///
    /// Carries the limit it BLEW, because the bound is wall-clock and the
    /// honest reading is "this did not finish in time HERE" -- which a
    /// reader can act on by raising `[triggers].runner_timeout_ms`.
    TimedOut(u128),
    /// The artifact could not be executed at all.
    Unrunnable(String),
}

impl Fired {
    /// Whether this has anything to tell the model.
    #[must_use]
    pub fn advice(&self) -> Option<String> {
        match self {
            Self::Clean => None,
            Self::Violated(said) => Some(said.clone()),
            Self::TimedOut(ms) => Some(format!(
                "rekall: the rule did not finish within {ms}ms and was \
                 killed. That bound is WALL-CLOCK, so a busy machine can \
                 trip it on a fast rule -- raise \
                 `[triggers].runner_timeout_ms` if this box is loaded, or \
                 make the rule answer sooner"
            )),
            Self::Unrunnable(said) => {
                Some(format!("rekall: the rule could not run: {said}"))
            }
        }
    }
}

/// Run one rule, bounded.
///
/// Executed DIRECTLY rather than through `sh`, so the artifact's own
/// shebang decides its interpreter. Forcing `sh` would silently break the
/// first rule somebody writes in python, and the generated template says
/// `#!/bin/sh` for itself rather than on everyone's behalf.
pub fn run(path: &Path, limit: Duration) -> Fired {
    let child = std::process::Command::new(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();
    match child {
        Ok(child) => wait(child, limit),
        Err(error) => Fired::Unrunnable(error.to_string()),
    }
}

/// POLLED rather than blocking, because the standard library has no
/// wait-with-timeout and a thread per rule would cost more than the rule.
fn wait(mut child: std::process::Child, limit: Duration) -> Fired {
    let deadline = Instant::now().checked_add(limit);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return finish(child),
            Err(error) => return Fired::Unrunnable(error.to_string()),
            Ok(None) => {}
        }
        if deadline.is_some_and(|end| Instant::now() >= end) {
            let _ = child.kill();
            let _ = child.wait();
            return Fired::TimedOut(limit.as_millis());
        }
        std::thread::sleep(POLL);
    }
}

fn finish(child: std::process::Child) -> Fired {
    let Ok(done) = child.wait_with_output() else {
        return Fired::Unrunnable("output could not be read".to_string());
    };
    if done.status.success() {
        return Fired::Clean;
    }
    Fired::Violated(said(&done))
}

/// What the rule said, stderr FIRST.
///
/// A shell rule writes its complaint to stderr and its incidental noise
/// to stdout, so preferring stderr puts the sentence a human wrote ahead
/// of whatever the script happened to echo.
fn said(done: &std::process::Output) -> String {
    let err = String::from_utf8_lossy(&done.stderr).trim().to_string();
    if !err.is_empty() {
        return err;
    }
    let out = String::from_utf8_lossy(&done.stdout).trim().to_string();
    if out.is_empty() {
        return "the rule reported a violation without saying what".to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The bound these tests run under, and it is DELIBERATELY generous.
    ///
    /// The bound is WALL-CLOCK (`B5`, `B16`), so a loaded box kills a rule that
    /// cost milliseconds. Every test below asserts what a rule SAID, and under
    /// `cargo test`'s own parallelism -- or a nix sandbox, where this was
    /// MEASURED failing six ways -- the default 2000 turns those into timeouts
    /// that never happened. `cli::testing::check_project` raises it for the same
    /// reason and cites the same bug.
    ///
    /// The one test that asserts the bound WORKS sets its own tight limit, and
    /// has to: that one is about the clock rather than about the rule.
    const PATIENT: Duration = Duration::from_secs(60);

    fn script(name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let dir = PathBuf::from("target").join("runner");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = std::fs::write(&path, body);
        let _ = std::fs::set_permissions(
            &path,
            std::fs::Permissions::from_mode(0o700),
        );
        path
    }

    #[test]
    fn a_rule_that_passes_says_nothing() {
        let path = script("clean.sh", "#!/bin/sh\nexit 0\n");
        let out = run(&path, PATIENT);
        assert_eq!(out, Fired::Clean);
        assert_eq!(out.advice(), None);
    }

    /// The rule's OWN words reach the model, not its exit code -- a number
    /// tells it nothing about what to do differently.
    #[test]
    fn a_violation_carries_what_the_rule_said() {
        let path = script(
            "violate.sh",
            "#!/bin/sh\necho 'do not commit to main' >&2\nexit 1\n",
        );
        assert_eq!(
            run(&path, PATIENT),
            Fired::Violated("do not commit to main".to_string())
        );
    }

    /// stderr first: a shell rule complains there and echoes incidentals
    /// to stdout.
    #[test]
    fn stderr_is_preferred_over_stdout() {
        let path = script(
            "both.sh",
            "#!/bin/sh\necho noise\necho 'the real complaint' >&2\nexit 1\n",
        );
        assert_eq!(
            run(&path, PATIENT),
            Fired::Violated("the real complaint".to_string())
        );
    }

    /// stdout is the FALLBACK, not ignored. A rule that only echoes still
    /// gets its words to the model.
    #[test]
    fn stdout_is_used_when_stderr_is_silent() {
        let path =
            script("out.sh", "#!/bin/sh\necho 'said on stdout'\nexit 1\n");
        assert_eq!(
            run(&path, PATIENT),
            Fired::Violated("said on stdout".to_string())
        );
    }

    #[test]
    fn a_silent_violation_still_says_something() {
        let path = script("mute.sh", "#!/bin/sh\nexit 3\n");
        let out = run(&path, PATIENT);
        assert!(
            out.advice()
                .is_some_and(|said| said.contains("without saying")),
            "{out:?}"
        );
    }

    /// V38: bounded. A rule that hangs is KILLED and reported, because
    /// this runs in the tool-call path and a stall there is worse than a
    /// failure.
    #[test]
    fn a_hanging_rule_is_killed_and_reported() {
        let path = script("hang.sh", "#!/bin/sh\nsleep 30\n");
        let started = Instant::now();
        let out = run(&path, Duration::from_millis(80));
        assert!(matches!(out, Fired::TimedOut(_)), "{out:?}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the wait was not bounded: {:?}",
            started.elapsed()
        );
        assert!(
            out.advice()
                .is_some_and(|said| said.contains("did not finish")),
            "{out:?}"
        );
    }

    /// An artifact that cannot be executed is reported, not silently
    /// treated as passing -- V26's shape, inside one rule.
    /// B5: the bound is CONFIGURED, and the shipped default is what a
    /// config without the key gets.
    #[test]
    fn the_limit_comes_from_config_or_falls_back() {
        assert_eq!(limit_from(None), LIMIT);
        assert_eq!(limit_from(Some(50)), Duration::from_millis(50));
    }

    /// The timeout REPORTS the bound it blew, and names the knob. The old
    /// message quoted a constant, which on a loaded box was advice about
    /// the wrong thing entirely.
    #[test]
    fn a_timeout_names_its_own_limit_and_the_knob() {
        let said = Fired::TimedOut(50).advice().unwrap_or_default();
        assert!(said.contains("50ms"), "{said}");
        assert!(said.contains("runner_timeout_ms"), "{said}");
        assert!(said.contains("WALL-CLOCK"), "{said}");
    }

    #[test]
    fn an_unrunnable_artifact_is_reported() {
        let out = run(Path::new("/definitely/not/here"), PATIENT);
        assert!(matches!(out, Fired::Unrunnable(_)), "{out:?}");
        assert!(
            out.advice()
                .is_some_and(|said| said.contains("could not run"))
        );
    }

    /// The shebang decides the interpreter. Forcing `sh` would break the
    /// first rule somebody writes in another language.
    #[test]
    fn the_shebang_chooses_the_interpreter() {
        let path = script(
            "shebang.sh",
            "#!/bin/sh\ntest \"$0\" != '' && echo ok >&2\nexit 1\n",
        );
        assert_eq!(run(&path, PATIENT), Fired::Violated("ok".to_string()));
    }
}
