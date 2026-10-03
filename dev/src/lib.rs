use std::fmt::Write as _;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub type External<'a> = dyn FnMut(&str) -> Result<String, String> + 'a;

pub fn dependency_count(cargo: &str) -> usize {
    let mut in_deps = false;
    let mut nested = 0usize;
    cargo
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_deps = trimmed == "[dependencies]";
                return false;
            }
            let top_level = nested == 0;
            nested += trimmed.chars().filter(|c| *c == '{').count();
            nested = nested
                .saturating_sub(trimmed.chars().filter(|c| *c == '}').count());
            in_deps
                && top_level
                && !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && trimmed.split_once('=').is_some_and(|(key, _)| {
                    key.trim()
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
                })
        })
        .count()
}

pub fn lock_node<'a>(lock: &'a str, name: &str) -> Option<&'a str> {
    let header = format!("    \"{name}\": {{");
    let start = lock.lines().position(|line| line == header)?;
    let mut depth = 0usize;
    let mut end = start;
    for (index, line) in lock.lines().enumerate().skip(start) {
        depth += line.chars().filter(|c| *c == '{').count();
        depth =
            depth.saturating_sub(line.chars().filter(|c| *c == '}').count());
        end = index;
        if index > start && depth == 0 {
            break;
        }
    }
    Some(
        lock.lines()
            .skip(start)
            .take(end - start + 1)
            .collect::<Vec<_>>()
            .join("\n")
            .leak(),
    )
}

pub fn marker_block(text: &str, name: &str) -> Result<(usize, usize), String> {
    let begin = format!("<!-- BEGIN {name} -->");
    let end = format!("<!-- END {name} -->");
    let starts: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| *l == begin)
        .map(|(n, _)| n)
        .collect();
    let ends: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| *l == end)
        .map(|(n, _)| n)
        .collect();
    if starts.len() != 1 || ends.len() != 1 || starts[0] >= ends[0] {
        return Err(format!("missing, duplicate or misordered {name} markers"));
    }
    Ok((starts[0], ends[0]))
}

pub fn splice(
    text: &str,
    name: &str,
    replacement: &str,
) -> Result<String, String> {
    let (begin, end) = marker_block(text, name)?;
    let lines: Vec<_> = text.lines().collect();
    let mut out = lines[..begin].join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(replacement.trim_end());
    out.push('\n');
    out.push_str(&lines[end + 1..].join("\n"));
    if text.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.trim().strip_prefix(key).map(|v| {
            v.trim().trim_end_matches(',').trim_matches('"').to_string()
        })
    })
}

fn unix_date(seconds: &str) -> String {
    let mut days = seconds.parse::<u64>().unwrap_or_default() / 86_400;
    let mut year = 1970i64;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let length = if leap { 366 } else { 365 };
        if days < length {
            break;
        }
        days -= length;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let lengths = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let month = lengths
        .iter()
        .position(|length| {
            if days < *length as u64 {
                true
            } else {
                days -= *length as u64;
                false
            }
        })
        .map_or(12, |index| index + 1);
    format!("{year:04}-{month:02}-{:02}", days + 1)
}
fn rows(root: &Path, prefix: &str) -> usize {
    walk_specs(root)
        .iter()
        .flat_map(|p| fs::read_to_string(p).ok())
        .flat_map(|s| s.lines().map(str::to_owned).collect::<Vec<_>>())
        .filter(|l| l.starts_with(prefix))
        .count()
}
#[allow(clippy::excessive_nesting)]
fn walk_specs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn go(p: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                let q = e.path();
                if q.file_name().is_some_and(|n| n == "target" || n == ".git") {
                    continue;
                }
                if q.is_dir() {
                    go(&q, out)
                } else if q.file_name().is_some_and(|n| n == "SPEC.md") {
                    out.push(q)
                }
            }
        }
    }
    go(root, &mut out);
    out
}

fn render(root: &Path, external: &mut External<'_>) -> Result<String, String> {
    let cargo = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|e| e.to_string())?;
    let lock = fs::read_to_string(root.join("flake.lock"))
        .map_err(|e| e.to_string())?;
    let lic =
        value(&cargo, "license = ").ok_or("Cargo.toml license is empty")?;
    let ed =
        value(&cargo, "edition = ").ok_or("Cargo.toml edition is empty")?;
    let msrv = value(&cargo, "rust-version = ")
        .ok_or("Cargo.toml rust-version is empty")?;
    let slug = value(&cargo, "repository = ")
        .ok_or("Cargo.toml repository is empty")?
        .trim_start_matches("https://github.com/")
        .to_string();
    let node = lock_node(&lock, "nixpkgs")
        .ok_or("flake.lock nixpkgs node is missing")?;
    let nixref = value(node, "\"ref\":")
        .unwrap_or_default()
        .trim_start_matches("nixos-")
        .to_string();
    let rev = value(node, "\"rev\":").unwrap_or_default();
    let nixrev = rev.chars().take(7).collect::<String>();
    let when = unix_date(&value(node, "\"lastModified\":").unwrap_or_default());
    let when_url = when.replace('-', "--");
    let com = external("hk run pre-commit --plan --json -a < /dev/null")
        .map_err(|e| format!("missing hk: {e}"))?
        .lines()
        .filter(|l| l.contains("orderIndex"))
        .count();
    let pus = external("hk run pre-push --plan --json -a < /dev/null")
        .map_err(|e| format!("missing hk: {e}"))?
        .lines()
        .filter(|l| l.contains("orderIndex"))
        .count();
    if dependency_count(&cargo) == 0
        || com == 0
        || pus == 0
        || rows(root, "V") == 0
    {
        return Err("a badge source is empty or zero".into());
    }
    let inv = walk_specs(root)
        .iter()
        .flat_map(|p| fs::read_to_string(p).ok())
        .flat_map(|s| s.lines().map(str::to_owned).collect::<Vec<_>>())
        .filter_map(|l| {
            l.strip_prefix('V')
                .and_then(|x| x.split_whitespace().next())
                .map(str::to_owned)
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let bug = walk_specs(root)
        .iter()
        .flat_map(|p| fs::read_to_string(p).ok())
        .flat_map(|s| s.lines().map(str::to_owned).collect::<Vec<_>>())
        .filter_map(|line| {
            line.strip_prefix('B')
                .and_then(|rest| rest.split('|').next())
                .map(str::to_owned)
        })
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let nod = walk_specs(root).len();
    let cov = value(
        &fs::read_to_string(root.join(".coverage"))
            .map_err(|e| e.to_string())?,
        "lines ",
    )
    .ok_or("coverage is empty")?;
    let mut out = String::new();
    writeln!(
        out,
        "<!-- BEGIN badges -->\n[![CI](https://github.com/{slug}/actions/workflows/ci.yml/badge.svg)](https://github.com/{slug}/actions/workflows/ci.yml)\n[![License: {lic}](https://img.shields.io/badge/license-{lic}-blue.svg)](LICENSE)\n[![edition {ed}](https://img.shields.io/badge/edition-{ed}-000000?logo=rust&logoColor=white)](Cargo.toml)\n[![MSRV {msrv}](https://img.shields.io/badge/MSRV-{msrv}-000000?logo=rust&logoColor=white)](Cargo.toml)\n[![direct dependencies {deps}](https://img.shields.io/badge/direct_dependencies-{deps}-brightgreen)](docs/THIRD-PARTY-NOTICES.md)\n[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-brightgreen)](Cargo.toml)\n[![network none](https://img.shields.io/badge/network-none-brightgreen)](docs/SECURITY.md)\n\n[![gate hk](https://img.shields.io/badge/gate-hk-6E4AFF)][hk]\n[![gate steps {com} commit / {pus} push](https://img.shields.io/badge/gate_steps-{com}_commit_%2F_{pus}_push-6E4AFF)][hk]\n[![coverage floor {cov}%](https://img.shields.io/badge/coverage_floor-%E2%89%A5{cov}%25-brightgreen)](.coverage)\n[![invariants {inv}](https://img.shields.io/badge/invariants-{inv}-6E4AFF)](SPEC.md)\n[![bugs logged {bug}](https://img.shields.io/badge/bugs_logged-{bug}-6E4AFF)](SPEC.md)\n[![federated nodes {nod}](https://img.shields.io/badge/federated_nodes-{nod}-6E4AFF)](docs/FEDERATION.md)\n\n[![nix flake](https://img.shields.io/badge/nix-flake-5277C3?logo=nixos&logoColor=white)][flake]\n[![nixpkgs {nixref} ({when} - {nixrev})](https://img.shields.io/badge/nixpkgs-{nixref}_({when_url}_--_{nixrev})-5277C3?logo=nixos&logoColor=white)](https://github.com/{slug}/blob/main/flake.lock)\n[![intel linux](https://img.shields.io/badge/linux-5277C3?logo=intel&logoColor=white)][flake]\n[![amd linux](https://img.shields.io/badge/linux-5277C3?logo=amd&logoColor=white)][flake]\n[![arm linux](https://img.shields.io/badge/linux-5277C3?logo=arm&logoColor=white)][flake]\n[![arm macos](https://img.shields.io/badge/macos-5277C3?logo=arm&logoColor=white)][flake]\n\n[![built with Claude Code](https://img.shields.io/badge/built_with-Claude_Code-D97757)](https://claude.com/claude-code)\n[![built with Opus 5](https://img.shields.io/badge/built_with-Opus_5-D97757)](https://www.anthropic.com/claude)\n[![built with SDD](https://img.shields.io/badge/built_with-spec--driven_development-D97757)](SPEC.md)\n<!-- END badges -->",
        deps = dependency_count(&cargo),
    )
    .map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_count_ignores_wrapped_features() {
        assert_eq!(
            dependency_count(
                "[dependencies]\nalpha = \"1\"\nbeta = { version = \"1\",\n features = [\"x\"] }\n"
            ),
            2
        );
    }

    #[test]
    fn lock_node_uses_exact_header_and_decoy_first() {
        let lock = "    \"nix-hk\": {\n      \"rev\": \"bad\"\n    },\n    \"nixpkgs\": {\n      \"rev\": \"good\"\n    }";
        assert!(lock_node(lock, "nixpkgs").is_some_and(|n| n.contains("good")));
    }

    #[test]
    fn markers_are_whole_lines_and_splice_is_idempotent() {
        let text = "x\n<!-- BEGIN badges -->\nold\n<!-- END badges -->\ny\n";
        let once = splice(
            text,
            "badges",
            "<!-- BEGIN badges -->\nnew\n<!-- END badges -->",
        )
        .unwrap_or_default();
        assert_eq!(
            splice(
                &once,
                "badges",
                "<!-- BEGIN badges -->\nnew\n<!-- END badges -->"
            )
            .unwrap_or_default(),
            once
        );
        assert!(
            marker_block(
                "<!-- BEGIN badges-extra -->\n<!-- END badges -->",
                "badges"
            )
            .is_err()
        );
        assert!(marker_block("<!-- BEGIN badges -->\n<!-- BEGIN badges -->\n<!-- END badges -->", "badges").is_err());
    }
}

pub fn run(
    args: &[String],
    root: &Path,
    external: &mut External<'_>,
    err: &mut dyn Write,
) -> u8 {
    if args.first().map(String::as_str) != Some("readme") {
        let _ = writeln!(err, "usage: rekall-dev readme [--check] [<path>...]");
        return 2;
    }
    let check = args.iter().any(|a| a == "--check");
    let path = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with('-'))
        .map_or_else(|| root.join("README.md"), |p| root.join(p));
    let old = match fs::read_to_string(&path) {
        Ok(v) => v,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    let rendered = match render(root, external) {
        Ok(v) => v,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    let new = match splice(&old, "badges", &rendered) {
        Ok(v) => v,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return 1;
        }
    };
    if old == new {
        return 0;
    }
    if check {
        let _ = writeln!(
            err,
            "want: README badge block matches sources\nhave: README badge block is stale"
        );
        return 1;
    }
    match fs::write(path, new) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            1
        }
    }
}
