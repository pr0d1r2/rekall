//! Whether a mutating run may proceed.
//!
//! Split out of `apply` when that module crossed the file cap, along the
//! seam the code already named: V20 is ONE rule and not one per verb.
//! `apply`, `revert` and `issue` all delete from somebody's private
//! memory, and a second confirm gate written for the second verb is a
//! second place for the rule to be slightly wrong.

/// How consent for a mutating run was obtained.
///
/// An enum rather than two booleans, because `approved(true, false)` at a
/// call site says nothing about which is which -- the two-bool limit in
/// clippy.toml exists for exactly this, and the three states here are a
/// KIND rather than a pair of flags.
#[derive(Debug, PartialEq, Eq)]
pub enum Consent {
    /// `--auto-approve` was passed.
    Flag,
    /// A terminal was present and the user answered.
    Answered(String),
    /// No terminal and no flag. Silence is NOT consent: prompting into a
    /// pipe hangs a CI job until someone kills it, and proceeding without
    /// asking makes the DESTRUCTIVE path the quiet one (V20).
    Unattended,
}

/// Whether this run may mutate.
pub fn approved(consent: &Consent) -> Result<(), String> {
    match consent {
        Consent::Flag => Ok(()),
        Consent::Unattended => Err(NEEDS_APPROVAL.to_string()),
        Consent::Answered(answer) => match answer.trim() {
            "y" | "Y" | "yes" => Ok(()),
            _ => Err("cancelled".to_string()),
        },
    }
}

pub const NEEDS_APPROVAL: &str = "this would edit your corpus and stdin is not a terminal. \
Re-run with --auto-approve if that is what you want";

/// Takes the rendered names rather than an outcome, because V20 is ONE
/// rule and not one per verb: the three mutating verbs all delete from the
/// user's private memory, and each would otherwise carry its own gate.
pub(super) fn consent_for(
    auto_approve: bool,
    named: &str,
) -> Result<Consent, String> {
    if auto_approve {
        return Ok(Consent::Flag);
    }
    eprint!("{named}");
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return Ok(Consent::Unattended);
    }
    ask_at_terminal()
}

/// Read an answer from anywhere.
///
/// Takes the reader rather than reaching for stdin, so the parsing of a
/// consent answer -- the part with a decision in it -- is testable without
/// a terminal. What is left needing one is the two lines below.
pub fn read_answer(
    input: &mut impl std::io::BufRead,
) -> Result<Consent, String> {
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .map_err(|error| error.to_string())?;
    Ok(Consent::Answered(answer))
}

fn ask_at_terminal() -> Result<Consent, String> {
    eprint!("apply these changes? [y/N] ");
    read_answer(&mut std::io::stdin().lock())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V20: off a tty, silence is not consent.
    #[test]
    fn without_a_terminal_and_without_the_flag_a_verb_refuses() {
        assert_eq!(
            approved(&Consent::Unattended),
            Err(NEEDS_APPROVAL.to_string())
        );
        assert!(NEEDS_APPROVAL.contains("--auto-approve"));
    }

    #[test]
    fn the_flag_is_consent_and_a_no_is_not() {
        assert!(approved(&Consent::Flag).is_ok());
        assert!(approved(&Consent::Answered("y\n".to_string())).is_ok());
        assert!(approved(&Consent::Answered("yes".to_string())).is_ok());
        assert!(approved(&Consent::Answered("n".to_string())).is_err());
        assert!(approved(&Consent::Answered(String::new())).is_err());
    }

    /// Bare Enter means NO. The prompt reads `[y/N]`, and a destructive
    /// default that triggers on a stray keypress is not a confirmation.
    #[test]
    fn an_empty_answer_cancels() {
        assert_eq!(
            approved(&Consent::Answered("\n".to_string())),
            Err("cancelled".to_string())
        );
    }

    #[test]
    fn an_answer_is_read_from_any_reader() {
        let mut input = std::io::Cursor::new(b"y\n".to_vec());
        assert_eq!(
            read_answer(&mut input).ok(),
            Some(Consent::Answered("y\n".to_string()))
        );
    }

    #[test]
    fn an_answer_that_is_read_is_then_judged() {
        let mut yes = std::io::Cursor::new(b"yes\n".to_vec());
        let mut no = std::io::Cursor::new(b"\n".to_vec());
        assert!(read_answer(&mut yes).and_then(|c| approved(&c)).is_ok());
        assert!(read_answer(&mut no).and_then(|c| approved(&c)).is_err());
    }
}
