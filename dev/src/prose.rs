//! The README's and docs' PROSE numbers, held to the same `Facts` the badges
//! render from, and the `shipped-links` check. Both are pure functions over
//! text, so neither needs a repository to be tested.

use crate::{External, Facts};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Byte span and value of the first number in `text` whose text before it
/// satisfies `before` and whose text after it starts with something `after`
/// accepts.
fn number_at(
    text: &str,
    before: &dyn Fn(&str) -> bool,
    after: &dyn Fn(&str) -> bool,
) -> Option<(usize, usize, usize)> {
    let mut start = 0;
    while start < text.len() {
        let rest = text.get(start..)?;
        let offset = rest.find(|c: char| c.is_ascii_digit())?;
        let from = start + offset;
        let tail = text.get(from..)?;
        let len = tail
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(tail.len());
        let end = from + len;
        if before(text.get(..from)?)
            && after(text.get(end..)?)
            && let Ok(n) = tail.get(..len)?.parse()
        {
            return Some((from, end, n));
        }
        start = end;
    }
    None
}

fn number(
    text: &str,
    before: &dyn Fn(&str) -> bool,
    after: &dyn Fn(&str) -> bool,
) -> Option<usize> {
    number_at(text, before, after).map(|(_, _, n)| n)
}

/// `[0-9]+ .{0,4}V. invariants`
fn invariants_after(rest: &str) -> bool {
    let Some(rest) = rest.strip_prefix(' ') else {
        return false;
    };
    rest.char_indices().take(5).any(|(at, c)| {
        c == 'V'
            && rest.get(at + 1..).is_some_and(|tail| {
                tail.chars().next().is_some_and(|second| {
                    tail.get(second.len_utf8()..)
                        .is_some_and(|after| after.starts_with(" invariants"))
                })
            })
    })
}

/// `truth . [0-9]+ invariants`: "truth", one character, a space.
fn truth_before(head: &str) -> bool {
    head.strip_suffix(' ').is_some_and(|rest| {
        rest.char_indices().next_back().is_some_and(|(at, _)| {
            rest.get(..at).is_some_and(|h| h.ends_with("truth "))
        })
    })
}

type Prefix = &'static dyn Fn(&str) -> bool;

fn any(_: &str) -> bool {
    true
}

/// One prose pattern: what it counts, how to find the number, which fact.
struct Pattern {
    what: &'static str,
    before: Prefix,
    after: Prefix,
    fact: fn(&Facts) -> usize,
}

/// NO TASK SPLIT. The README used to say "at N landed and M remaining", and
/// that is the one figure EVERY task moves: a trip that lands one `§T` row
/// changes it, so every task trip had to edit README prose it never touched.
/// `apply--T50` was refused on 38/6 against 39/5 twice (B29), in a sandbox
/// where neither `hk` nor the `--fix` below could run, and the loop braked
/// 12 hours each time. The sentence no longer states the split, so nothing
/// here reads it: a figure that is not declared needs no runner (V22).
fn readme_patterns() -> Vec<Pattern> {
    vec![
        Pattern {
            what: "invariants (prose)",
            before: &truth_before,
            after: &|r| r.starts_with(" invariants"),
            fact: |f| f.invariants,
        },
        Pattern {
            what: "bug rows (prose)",
            before: &|h| h.ends_with("; "),
            after: &|r| r.starts_with(" recorded"),
            fact: |f| f.bugs,
        },
        Pattern {
            what: "gate steps on commit",
            before: &any,
            after: &|r| r.starts_with(" steps on commit"),
            fact: |f| f.commit_steps,
        },
        Pattern {
            what: "gate steps on push",
            before: &|h| h.ends_with("commit, "),
            after: &|r| r.starts_with(" on push"),
            fact: |f| f.push_steps,
        },
    ]
}

fn doc_patterns() -> Vec<Pattern> {
    vec![
        Pattern {
            what: "gate steps on commit",
            before: &any,
            after: &|r| r.starts_with(" steps on"),
            fact: |f| f.commit_steps,
        },
        Pattern {
            what: "gate steps on push",
            before: &any,
            after: &|r| r.starts_with(" on push"),
            fact: |f| f.push_steps,
        },
        Pattern {
            what: "invariants",
            before: &any,
            after: &invariants_after,
            fact: |f| f.invariants,
        },
    ]
}

/// Findings for the README (every pattern must be present and right) and for
/// each other doc (a pattern absent from a doc is not an error).
pub fn check_prose(
    facts: &Facts,
    readme: &str,
    docs: &[(String, String)],
) -> Vec<String> {
    let mut out = Vec::new();
    for p in readme_patterns() {
        let want = (p.fact)(facts);
        match number(readme, p.before, p.after) {
            Some(said) if said == want => {}
            said => out.push(format!(
                "hk: README says {} is {}; the sources hold {want}. Fix the SENTENCE in README.md. The badge carrying this number is GENERATED by the badges step; the prose is not, which is why it is still checked here (V22).",
                p.what,
                said.map_or_else(|| "absent".to_owned(), |n| n.to_string()),
            )),
        }
    }
    for (name, text) in docs {
        for p in doc_patterns() {
            let want = (p.fact)(facts);
            if let Some(said) = number(text, p.before, p.after)
                && said != want
            {
                out.push(format!(
                    "hk: {name} says {} is {said}; the sources hold {want}. Fix the SENTENCE there.",
                    p.what
                ));
            }
        }
    }
    out
}

/// `text` with each pattern's stated number replaced by the fact it counts.
///
/// ONLY THE DIGITS MOVE. The sentence a pattern finds stays the human's, word
/// for word, and a pattern with no sentence in `text` writes nothing:
/// inventing the sentence would be authoring prose. The rule the check exists
/// for (V22, a declared figure is read) is untouched; what goes is the chore
/// that turned it into a refusal. A gate that names the right number but
/// will not write it made one stale digit end a trip as "needs a human".
fn fix_numbers(facts: &Facts, text: &str, patterns: &[Pattern]) -> String {
    let mut out = text.to_owned();
    for p in patterns {
        let want = (p.fact)(facts);
        if let Some((from, end, said)) = number_at(&out, p.before, p.after)
            && said != want
        {
            out.replace_range(from..end, &want.to_string());
        }
    }
    out
}

/// The README and each doc, with every number `check_prose` would flag
/// rewritten to the fact. A README sentence that is ABSENT stays a finding.
pub fn fix_prose(
    facts: &Facts,
    readme: &str,
    docs: &[(String, String)],
) -> (String, Vec<(String, String)>) {
    let readme = fix_numbers(facts, readme, &readme_patterns());
    let docs = docs
        .iter()
        .map(|(name, text)| {
            (name.clone(), fix_numbers(facts, text, &doc_patterns()))
        })
        .collect();
    (readme, docs)
}

/// Write `new` to `root/name` only when it differs from `old`, so a fix with
/// nothing to do touches no file.
fn write_changed(
    root: &Path,
    name: &str,
    old: &str,
    new: &str,
) -> std::io::Result<()> {
    if old == new {
        return Ok(());
    }
    fs::write(root.join(name), new)
        .map_err(|e| std::io::Error::new(e.kind(), format!("{name}: {e}")))
}

/// File parts of `](path#fragment)` links, `path` non-empty and restricted
/// to `[A-Za-z0-9_./-]`.
fn link_paths(text: &str) -> Vec<&str> {
    text.split("](")
        .skip(1)
        .filter_map(|rest| {
            let end = rest.find(')')?;
            let path = rest.get(..end)?.split('#').next().unwrap_or_default();
            (!path.is_empty()
                && path
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_./-".contains(c)))
            .then_some(path)
        })
        .collect()
}

/// `  <doc> -> <target>` for every link in `text` (at repo path `doc`) that
/// does not resolve to a line of `crate_files`.
pub fn shipped_links(doc: &str, text: &str, crate_files: &str) -> Vec<String> {
    link_paths(text)
        .into_iter()
        .filter_map(|path| {
            let resolved = if doc.starts_with("docs/") {
                let mut dir: Vec<&str> = doc.split('/').collect();
                dir.pop();
                let mut rest = path;
                let mut escaped = false;
                while let Some(up) = rest.strip_prefix("../") {
                    escaped |= dir.pop().is_none();
                    rest = up;
                }
                dir.push(rest);
                (!escaped).then(|| dir.join("/"))
            } else {
                Some(path.to_owned())
            };
            (!resolved.is_some_and(|r| crate_files.lines().any(|l| l == r)))
                .then(|| format!("  {doc} -> {path}"))
        })
        .collect()
}

/// Every `.md` under `root/dir`, as (repository-relative path, text). A
/// directory or file that cannot be read is an error, never a silent skip.
fn collect_docs(
    root: &Path,
    dir: &str,
    out: &mut Vec<(String, String)>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(root.join(dir))? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let rel = format!("{dir}/{name}");
        let path = entry.path();
        if path.is_dir() {
            collect_docs(root, &rel, out)?;
        } else if path.extension().is_some_and(|x| x == "md") {
            let text = fs::read_to_string(&path).map_err(|e| {
                std::io::Error::new(e.kind(), format!("{rel}: {e}"))
            })?;
            out.push((rel, text));
        }
    }
    Ok(())
}

pub(crate) fn run(
    root: &Path,
    args: &[String],
    external: &mut External<'_>,
    err: &mut dyn Write,
) -> u8 {
    let read = |name: &str| fs::read_to_string(root.join(name));
    let mut readme = match read("README.md") {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    let mut docs = Vec::new();
    if let Err(e) = collect_docs(root, "docs", &mut docs)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        let _ = writeln!(err, "hk: cannot read docs: {e}");
        return 1;
    }
    docs.sort();
    let selected = args.iter().find_map(|a| a.strip_prefix("--only="));
    let fix = args.iter().any(|a| a == "--fix");
    if selected.is_some_and(|s| s != "counts" && s != "links")
        || (fix && selected != Some("counts"))
    {
        let _ = writeln!(
            err,
            "usage: rekall-dev prose [--only=counts|links] [--only=counts --fix]"
        );
        return 2;
    }
    let only = |part: &str| selected.is_none_or(|s| s == part);
    let mut failed = false;
    if only("counts") {
        let facts = match Facts::gather(root, external) {
            Ok(f) => f,
            Err(e) => {
                let _ = writeln!(err, "{e}");
                return 1;
            }
        };
        if fix {
            let (fixed, fixed_docs) = fix_prose(&facts, &readme, &docs);
            let mut written = write_changed(root, "README.md", &readme, &fixed);
            for ((name, old), (_, new)) in docs.iter().zip(&fixed_docs) {
                written =
                    written.and_then(|()| write_changed(root, name, old, new));
            }
            if let Err(e) = written {
                let _ = writeln!(err, "{e}");
                return 1;
            }
            readme = fixed;
            docs = fixed_docs;
        }
        for finding in check_prose(&facts, &readme, &docs) {
            failed = true;
            let _ = writeln!(err, "{finding}");
        }
    }
    if !only("links") {
        return u8::from(failed);
    }
    let crate_files = match read(".crate-files") {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    let mut dead = shipped_links("README.md", &readme, &crate_files);
    for (name, text) in &docs {
        dead.extend(shipped_links(name, text, &crate_files));
    }
    if !dead.is_empty() {
        failed = true;
        let _ = writeln!(
            err,
            "hk: a SHIPPED doc links to a file the .crate EXCLUDES, so the link is dead for every reader on crates.io and docs.rs:\n{}\nMake it an absolute repository url, or stop excluding the file.",
            dead.join("\n")
        );
    }
    u8::from(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            invariants: 67,
            bugs: 28,
            commit_steps: 25,
            push_steps: 31,
        }
    }

    fn says(readme: &str) -> Vec<String> {
        check_prose(&facts(), readme, &[])
    }

    const GOOD: &str = "truth \u{2014} 67 invariants; 28 recorded. 25 steps on commit, 31 on push";

    #[test]
    fn matching_prose_passes() {
        let all = says(GOOD);
        assert!(all.is_empty(), "{all:?}");
    }

    #[test]
    fn a_landed_task_never_refuses_the_readme() {
        // B29: `apply--T50` landed one task and the README's 38/6 split was
        // refused against 39/5. No task count is read, so none can go stale.
        let readme = format!("{GOOD}, at 38 landed and 6 remaining.");
        let all = says(&readme);
        assert!(all.is_empty(), "{all:?}");
        let (fixed, _) = fix_prose(&facts(), &readme, &[]);
        assert_eq!(fixed, readme);
    }

    #[test]
    fn each_readme_pattern_is_checked_on_its_own() {
        for (old, new, what) in [
            (
                "truth \u{2014} 67",
                "truth \u{2014} 66",
                "invariants (prose)",
            ),
            ("; 28 recorded", "; 27 recorded", "bug rows (prose)"),
            ("25 steps on commit", "24 steps on commit", "on commit"),
            ("commit, 31 on push", "commit, 30 on push", "on push"),
        ] {
            let found = says(&GOOD.replace(old, new));
            assert_eq!(found.len(), 1, "{what}: {found:?}");
            assert!(found[0].contains(what), "{found:?}");
        }
    }

    #[test]
    fn a_missing_readme_sentence_is_a_finding() {
        let found = says("nothing here");
        assert_eq!(found.len(), 4);
        assert!(found[0].contains("is absent"));
    }

    #[test]
    fn docs_are_checked_when_they_state_a_number() {
        let doc = |t: &str| vec![("docs/A.md".to_owned(), t.to_owned())];
        let run = |t: &str| check_prose(&facts(), GOOD, &doc(t));
        assert!(run("nothing numeric").is_empty());
        assert!(
            run("25 steps on commit, 31 on push, 67 `V` invariants").is_empty()
        );
        assert!(run("24 steps on commit")[0].contains("docs/A.md"));
        assert!(run("32 on push")[0].contains("push"));
        assert!(run("51 V* invariants")[0].contains("invariants"));
    }

    #[test]
    fn fix_rewrites_only_the_stale_digits() {
        // One step was added, so both plan sizes moved by one.
        let stale =
            GOOD.replace("25 steps on commit, 31", "24 steps on commit, 30");
        let doc = vec![(
            "docs/A.md".to_owned(),
            "24 steps on commit, then 7 more words".to_owned(),
        )];
        let (readme, docs) = fix_prose(&facts(), &stale, &doc);
        assert_eq!(readme, GOOD);
        assert_eq!(docs[0].1, "25 steps on commit, then 7 more words");
        assert!(check_prose(&facts(), &readme, &docs).is_empty());
    }

    #[test]
    fn fix_writes_no_sentence_that_is_absent() {
        let (readme, docs) = fix_prose(&facts(), "nothing here", &[]);
        assert_eq!(readme, "nothing here");
        assert!(docs.is_empty());
        assert_eq!(says(&readme).len(), 4);
    }

    #[test]
    fn facts_count_rows_not_distinct_ids() {
        let spec = "V1 a\nV1 again\nV2 b\nB1|x\nB1|y\nB2|z\nVx not a row\nT1|x|a\nT2|x|b\nT3|.|c\nTx|x|d\n";
        let f =
            Facts::from_specs(&[spec.to_owned(), "V3 c\n".to_owned()], 2, 3);
        assert_eq!(
            f,
            Facts {
                invariants: 4,
                bugs: 3,
                commit_steps: 2,
                push_steps: 3
            }
        );
    }

    const CRATE: &str = "README.md\nSPEC.md\ndocs/A.md\ndocs/B.md\n";

    #[test]
    fn shipped_links_flags_dead_and_unshipped_targets() {
        let readme = "[ok](SPEC.md) [a](docs/A.md#x) [gate](hk.pkl) [gone](nope.md) [web](https://x.y/z)";
        assert_eq!(
            shipped_links("README.md", readme, CRATE),
            ["  README.md -> hk.pkl", "  README.md -> nope.md"]
        );
    }

    #[test]
    fn shipped_links_checks_the_file_part_of_fragment_links() {
        let readme = "[m](missing.md#section) [ok](SPEC.md#top) [f](#only)";
        assert_eq!(
            shipped_links("README.md", readme, CRATE),
            ["  README.md -> missing.md"]
        );
    }

    #[test]
    fn shipped_links_resolves_docs_relative_to_docs() {
        let doc = "[s](../SPEC.md) [b](B.md) [flake](../flake.nix) [dead](C.md) [top](#frag)";
        assert_eq!(
            shipped_links("docs/A.md", doc, CRATE),
            ["  docs/A.md -> ../flake.nix", "  docs/A.md -> C.md"]
        );
    }

    #[test]
    fn shipped_links_rejects_paths_escaping_the_crate_root() {
        let doc = "[x](../../SPEC.md)";
        assert_eq!(
            shipped_links("docs/A.md", doc, CRATE),
            ["  docs/A.md -> ../../SPEC.md"]
        );
    }
}

#[cfg(test)]
mod run_tests {
    use super::*;

    fn repo(readme: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rekall-dev-prose-{}-{}",
            std::process::id(),
            readme.len()
        ));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::create_dir_all(root.join("docs"));
        let _ = fs::write(root.join("README.md"), readme);
        let _ = fs::write(root.join(".crate-files"), "README.md\n");
        let _ = fs::write(root.join("SPEC.md"), "V1 a\nB1|x\nT1|x|a\nT2|.|b\n");
        root
    }

    fn verb(root: &Path, only: &str) -> (u8, String) {
        let (code, text, _) = counted(root, only);
        (code, text)
    }

    /// Like `verb`, also returning how often the external hook was invoked.
    fn counted(root: &Path, only: &str) -> (u8, String, usize) {
        let args = vec!["prose".to_owned(), only.to_owned()];
        let calls = std::cell::Cell::new(0);
        let mut plan = |_: &str| {
            calls.set(calls.get() + 1);
            Ok("\"orderIndex\"\n".to_owned())
        };
        let mut err = Vec::new();
        let code = run(root, &args, &mut plan, &mut err);
        (
            code,
            String::from_utf8_lossy(&err).into_owned(),
            calls.get(),
        )
    }

    #[test]
    fn only_selects_which_half_runs() {
        let readme = "9 steps on commit [dead](gone.md)";
        let root = repo(readme);
        let (both, text) = verb(&root, "--check");
        assert_eq!(both, 1);
        assert!(text.contains("steps on commit") && text.contains("gone.md"));
        let (links, text, calls) = counted(&root, "--only=links");
        assert!(links == 1 && text.contains("gone.md"));
        assert!(!text.contains("steps on commit"));
        assert_eq!(calls, 0, "--only=links must not call the external hook");
        let (counts, text) = verb(&root, "--only=counts");
        assert!(counts == 1 && !text.contains("gone.md"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn only_counts_does_not_read_the_crate_file_list() {
        let root = repo("9 steps on commit [dead](gone.md) only counts");
        let _ = fs::remove_file(root.join(".crate-files"));
        let (counts, text) = verb(&root, "--only=counts");
        assert_eq!(counts, 1);
        assert!(text.contains("steps on commit") && !text.contains("os error"));
        let (links, text) = verb(&root, "--only=links");
        assert!(links == 1 && text.contains("os error"), "{text}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fix_rewrites_the_readme_and_docs_on_disk() {
        let root = repo("9 steps on commit, 9 on push, fixed");
        let _ = fs::create_dir_all(root.join("docs/sub"));
        let _ = fs::write(root.join("docs/sub/A.md"), "7 steps on commit");
        let (code, _) = fix(&root);
        let readme =
            fs::read_to_string(root.join("README.md")).unwrap_or_default();
        assert_eq!(readme, "1 steps on commit, 1 on push, fixed");
        let doc =
            fs::read_to_string(root.join("docs/sub/A.md")).unwrap_or_default();
        assert_eq!(doc, "1 steps on commit");
        // Only the step counts are in this README, so the other two stay
        // findings: a fix never invents the sentence it would correct.
        assert_eq!(code, 1);
        let (_, text) = verb(&root, "--only=counts");
        assert!(!text.contains("steps on commit") && !text.contains("on push"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fix_is_only_for_counts() {
        let root = repo("9 steps on commit; fix needs counts too");
        let args = ["prose", "--fix"].map(str::to_owned);
        let mut plan = |_: &str| Ok(String::new());
        let mut err = Vec::new();
        assert_eq!(run(&root, &args, &mut plan, &mut err), 2);
        let readme =
            fs::read_to_string(root.join("README.md")).unwrap_or_default();
        assert_eq!(readme, "9 steps on commit; fix needs counts too");
        let _ = fs::remove_dir_all(root);
    }

    fn fix(root: &Path) -> (u8, String) {
        let args = ["prose", "--only=counts", "--fix"].map(str::to_owned);
        let mut plan = |_: &str| Ok("\"orderIndex\"\n".to_owned());
        let mut err = Vec::new();
        let code = run(root, &args, &mut plan, &mut err);
        (code, String::from_utf8_lossy(&err).into_owned())
    }

    #[test]
    fn write_changed_touches_nothing_unchanged_and_names_a_failed_file() {
        let nowhere = std::env::temp_dir()
            .join(format!("rekall-dev-prose-{}-absent", std::process::id()))
            .join("no-such-dir");
        assert!(write_changed(&nowhere, "README.md", "same", "same").is_ok());
        let e = write_changed(&nowhere, "README.md", "old", "new")
            .map_or_else(|e| e.to_string(), |()| String::new());
        assert!(e.starts_with("README.md: "), "{e}");
    }

    #[test]
    fn unknown_only_value_is_a_usage_error() {
        let root = repo("9 steps on commit");
        let (code, text) = verb(&root, "--only=bogus");
        assert_eq!(code, 2);
        assert!(text.contains("usage: rekall-dev prose"));
        let _ = fs::remove_dir_all(root);
    }
}
