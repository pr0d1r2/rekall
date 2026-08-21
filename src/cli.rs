//! Argument parsing and dispatch.
//!
//! Hand-rolled rather than derived. The verb set is small and fixed by
//! section I, and the exit codes are part of the published contract -- 0 ok,
//! 1 drift, 2 usage -- so the mapping from argument to exit stays visible
//! here instead of inside a macro.

use crate::{apply, config, init, ledger, plan, revert, scan, show, statement};
use std::path::{Path, PathBuf};

pub const USAGE_EXIT: u8 = 2;

#[derive(Debug, PartialEq, Eq)]
pub enum Format {
    Human,
    Json,
}

/// An unknown `--format` is a USAGE error, never a silent fall back to
/// prose (V17). An agent that asked for json and received a table would
/// parse garbage rather than fail.
pub fn parse_format(raw: &str) -> Result<Format, String> {
    match raw {
        "human" => Ok(Format::Human),
        "json" => Ok(Format::Json),
        other => Err(format!(
            "unknown --format `{other}` -- expected `human` or `json`"
        )),
    }
}

#[derive(Debug, Default)]
pub struct ScanArgs {
    pub filter: scan::Filter,
    pub sources: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

/// Parse `scan`'s flags.
///
/// A flag that takes a value and is given none is an error rather than a
/// default: `--top` with nothing after it is a truncated command line, and
/// guessing what was meant is how a scan silently reports the wrong slice.
pub fn parse_scan(args: &[String]) -> Result<ScanArgs, String> {
    let mut out = ScanArgs::default();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        apply_scan_flag(&mut out, flag, &mut rest)?;
    }
    Ok(out)
}

fn apply_scan_flag<'a>(
    out: &mut ScanArgs,
    flag: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match flag {
        "--sources" => out.sources = true,
        "--format" => {
            out.json = parse_format(&need(flag, rest)?)? == Format::Json
        }
        "--class" => out.filter.class = Some(need(flag, rest)?),
        "--sharpness" => {
            out.filter.sharpness = Some(parse_level(&need(flag, rest)?)?)
        }
        "--top" => out.filter.top = Some(parse_count(&need(flag, rest)?)?),
        "-C" => out.cwd = Some(PathBuf::from(need(flag, rest)?)),
        other => return Err(format!("unknown flag `{other}`")),
    }
    Ok(())
}

fn need<'a>(
    flag: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<String, String> {
    rest.next()
        .cloned()
        .ok_or_else(|| format!("`{flag}` needs a value"))
}

fn parse_level(raw: &str) -> Result<u8, String> {
    match raw {
        "1" => Ok(1),
        "2" => Ok(2),
        "3" => Ok(3),
        other => Err(format!("--sharpness must be 1, 2 or 3, got `{other}`")),
    }
}

fn parse_count(raw: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|_| format!("--top needs a whole number, got `{raw}`"))
}

/// Load both config scopes and report every root with the scope that named
/// it. The union lives in `config::merge`; this keeps the attribution so
/// `--sources` can show where each root came from.
pub fn resolve(cwd: &Path) -> Result<Resolved, String> {
    let user = load_optional(config::user_path().as_deref())?;
    let project = load_optional(config::find_project(cwd).as_deref())?;
    let merged = config::merge(user.clone(), project.clone());
    Ok(Resolved {
        roots: config::attribute(&user, &project),
        globs: merged.sources.globs.unwrap_or_default(),
    })
}

pub struct Resolved {
    pub roots: Vec<(String, config::Scope)>,
    pub globs: Vec<String>,
}

/// A config file that is absent is fine; one that is present and broken is
/// not. Treating a parse error as "no config" would run the scan against a
/// corpus the user never described.
fn load_optional(path: Option<&Path>) -> Result<config::Config, String> {
    let Some(path) = path else {
        return Ok(config::Config::default());
    };
    if !path.is_file() {
        return Ok(config::Config::default());
    }
    config::load_file(path).map_err(|error| error.to_string())
}

/// Every verb SPEC.md section I defines, in the order it lists them.
pub const VERBS: [&str; 11] = [
    "init", "scan", "show", "plan", "apply", "check", "recall", "hook", "log",
    "catch", "revert",
];

pub const USAGE: &str = "\
rekall -- extract always-on agent prose into rules and skills

usage: rekall <verb> [args]

verbs:
  init    detect corpus roots and write rekall.toml
  scan    inventory the corpus, one row per statement
  show    one statement or artifact in full
  plan    diff an extraction, optionally into a plan file
  apply   execute an extraction; confirms before it mutates
  check   the gate -- exit 1 on drift
  recall  which situational skills load here
  hook    harness hook JSON on stdin, decision JSON on stdout
  log     read the ledger
  catch   mine a transcript for candidate statements
  revert  reverse one extraction, verbatim; confirms before it mutates

exit: 0 ok, 1 drift or violation, 2 usage
";

/// What the binary should do, decided without performing any of it.
///
/// Separating the DECISION from the printing is what makes the dispatch
/// testable: a test asserts on this value, while `dispatch` does the
/// unavoidable side effects in three lines nothing can cover.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    PrintUsage {
        code: u8,
    },
    PrintVersion,
    Scan(Vec<String>),
    Init(Vec<String>),
    Show(Vec<String>),
    Plan(Vec<String>),
    Apply(Vec<String>),
    Revert(Vec<String>),
    /// A verb section I defines that this build cannot perform.
    Unimplemented(String),
    /// A verb the binary has never heard of -- a typo, not a backlog row.
    Unknown(String),
}

/// Decide what a command line asks for.
///
/// An unimplemented verb and an unknown one carry DIFFERENT messages and
/// the SAME exit code. Section I fixes the exit set at 0 ok, 1 drift, 2
/// usage, and neither case is drift -- inventing a fourth code to make a
/// scaffold more expressive would change a published contract for a state
/// that stops existing once the verbs land.
fn rest(args: &[String]) -> Vec<String> {
    args.get(1..).unwrap_or_default().to_vec()
}

#[must_use]
pub fn decide(args: &[String]) -> Action {
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help") => Action::PrintUsage { code: 0 },
        Some("-V" | "--version") => Action::PrintVersion,
        Some("scan") => Action::Scan(rest(args)),
        Some("init") => Action::Init(rest(args)),
        Some("show") => Action::Show(rest(args)),
        Some("plan") => Action::Plan(rest(args)),
        Some("apply") => Action::Apply(rest(args)),
        Some("revert") => Action::Revert(rest(args)),
        Some(verb) if VERBS.contains(&verb) => {
            Action::Unimplemented(verb.to_string())
        }
        Some(other) => Action::Unknown(other.to_string()),
    }
}

/// Perform the decided action and return the EXIT CODE as a value.
///
/// A value rather than an `ExitCode` so tests can assert on it: `ExitCode`
/// is deliberately opaque and implements no comparison, which would leave
/// every arm here exercised but unasserted. `dispatch` converts once.
/// The process facts the library needs but must not READ for itself.
///
/// `current_dir` and `HOME` are process globals. A library that reaches for
/// them is a library whose behaviour depends on state no caller passed and
/// no test can set -- `std::env::set_var` is unsafe in edition 2024, and
/// mutating it would race other tests anyway. The binary reads them once,
/// at the edge, and hands them in.
#[derive(Debug, Clone)]
pub struct Env {
    pub cwd: PathBuf,
    pub home: Option<String>,
}

#[must_use]
pub fn perform(action: Action, env: &Env) -> u8 {
    match action {
        Action::PrintUsage { code } => show_usage(code),
        Action::PrintVersion => show_version(),
        Action::Scan(flags) => run_scan(&flags, env),
        Action::Init(flags) => run_init(&flags, env),
        Action::Show(flags) => run_show(&flags, env),
        Action::Plan(flags) => run_plan(&flags, env),
        Action::Apply(flags) => run_apply(&flags, env),
        Action::Revert(flags) => run_revert(&flags, env),
        Action::Unimplemented(verb) => say_unimplemented(&verb),
        Action::Unknown(other) => say_unknown(&other),
    }
}

fn show_usage(code: u8) -> u8 {
    print!("{USAGE}");
    code
}

fn show_version() -> u8 {
    println!("rekall {}", env!("CARGO_PKG_VERSION"));
    0
}

fn say_unimplemented(verb: &str) -> u8 {
    eprintln!(
        "rekall: `{verb}` is specified but not implemented in this build"
    );
    USAGE_EXIT
}

fn say_unknown(other: &str) -> u8 {
    eprintln!("rekall: unknown verb `{other}`\n");
    show_usage(USAGE_EXIT)
}

/// Print an outcome and return its exit code.
///
/// EVERY verb ends this way, so it is written once. Six copies of the same
/// four lines is six chances for one verb to print its warnings to stdout,
/// or to exit 0 on an error, and each copy looks correct on its own -- the
/// divergence only shows to whoever piped that one verb into something.
fn emit(result: Result<Output, String>) -> u8 {
    match result {
        Ok(output) => {
            for warning in &output.warnings {
                eprintln!("rekall: {warning}");
            }
            print!("{}", output.text);
            0
        }
        Err(message) => {
            eprintln!("rekall: {message}");
            USAGE_EXIT
        }
    }
}

/// `init` flags. `--force` is separate from `--auto-approve` on purpose:
/// this writes its OWN config, never the corpus, so it is not the
/// destructive path V20 guards.
#[derive(Debug, Default)]
pub struct InitArgs {
    pub force: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_init(args: &[String]) -> Result<InitArgs, String> {
    let mut out = InitArgs::default();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        apply_init_flag(&mut out, flag, &mut rest)?;
    }
    Ok(out)
}

fn apply_init_flag<'a>(
    out: &mut InitArgs,
    flag: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match flag {
        "--force" => out.force = true,
        "--format" => {
            out.json = parse_format(&need(flag, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(flag, rest)?)),
        other => return Err(format!("unknown flag `{other}`")),
    }
    Ok(())
}

/// Run `init` end to end.
pub fn init_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_init(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let report = init::run(&base, env.home.as_deref(), args.force)
        .map_err(|error| error.to_string())?;
    Ok(Output {
        text: render_init(&report, args.json)?,
        warnings: Vec::new(),
    })
}

fn render_init(report: &init::Report, json: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string_pretty(report)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(init::render_human(report))
}

fn run_init(flags: &[String], env: &Env) -> u8 {
    emit(init_command(flags, env))
}

/// `show` takes ONE id (or an unambiguous prefix of one) plus the usual
/// format and directory flags.
#[derive(Debug, Default)]
pub struct ShowArgs {
    pub id: Option<String>,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_show(args: &[String]) -> Result<ShowArgs, String> {
    let mut out = ShowArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_show_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_show_arg<'a>(
    out: &mut ShowArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        id => return set_id(out, id),
    }
    Ok(())
}

/// A second positional is a MISTAKE, not a second lookup. `show a b` most
/// likely means the shell split something, and quietly showing only `a`
/// would answer a question nobody asked.
fn set_id(out: &mut ShowArgs, id: &str) -> Result<(), String> {
    if out.id.is_some() {
        return Err(format!("`show` takes one id, got a second: `{id}`"));
    }
    out.id = Some(id.to_string());
    Ok(())
}

/// Run `show` end to end.
pub fn show_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_show(flags)?;
    let id = args.id.clone().ok_or_else(|| NO_ID.to_string())?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let resolved = resolve(&base)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    render_lookup(&find(&resolved, &base, env, &id)?, &id, args.json)
}

pub const NO_ID: &str = "`show` needs an id -- copy one from `rekall scan`";

fn find(
    resolved: &Resolved,
    base: &Path,
    env: &Env,
    id: &str,
) -> Result<show::Lookup, String> {
    let at = scan::Corpus {
        roots: &resolved.roots,
        globs: &resolved.globs,
        home: env.home.as_deref(),
        base,
    };
    show::lookup(&at, id).map_err(|error| error.to_string())
}

fn render_lookup(
    found: &show::Lookup,
    id: &str,
    json: bool,
) -> Result<Output, String> {
    match found {
        show::Lookup::Unique(found) => Ok(Output {
            text: render_found(found, json)?,
            warnings: Vec::new(),
        }),
        show::Lookup::Ambiguous(ids) => Err(format!(
            "`{id}` matches {} statements: {}. Use more characters.",
            ids.len(),
            ids.join(", ")
        )),
        show::Lookup::Missing => Err(format!("no statement matches `{id}`")),
    }
}

fn render_found(found: &show::Found, json: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string_pretty(found)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(show::render_human(found))
}

fn run_show(flags: &[String], env: &Env) -> u8 {
    emit(show_command(flags, env))
}

/// `plan` takes one or more ids plus `--out` and the usual flags.
#[derive(Debug, Default)]
pub struct PlanArgs {
    pub ids: Vec<String>,
    pub out: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_plan(args: &[String]) -> Result<PlanArgs, String> {
    let mut out = PlanArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_plan_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_plan_arg<'a>(
    out: &mut PlanArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json
        }
        "--out" => out.out = Some(PathBuf::from(need(arg, rest)?)),
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        id => out.ids.push(id.to_string()),
    }
    Ok(())
}

/// Run `plan` end to end.
pub fn plan_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_plan(flags)?;
    if args.ids.is_empty() {
        return Err(NO_PLAN_IDS.to_string());
    }
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let loaded = load_corpus(&base, env)?;
    let chosen = choose(&loaded.statements, &args.ids)?;
    let built = plan::build(&chosen, &loaded.sources)
        .map_err(|error| error.to_string())?;
    emit_plan(&built, &args, &base)
}

pub const NO_PLAN_IDS: &str =
    "`plan` needs at least one id -- copy them from `rekall scan`";

fn load_corpus(base: &Path, env: &Env) -> Result<scan::Loaded, String> {
    let resolved = resolve(base)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    let at = scan::Corpus {
        roots: &resolved.roots,
        globs: &resolved.globs,
        home: env.home.as_deref(),
        base,
    };
    scan::load(&at).map_err(|error| error.to_string())
}

/// Resolve each id prefix to exactly one statement.
///
/// Every id is resolved BEFORE any step is built, so a typo in the third
/// id does not produce a partial plan for the first two.
fn choose(
    statements: &[statement::Statement],
    ids: &[String],
) -> Result<Vec<statement::Statement>, String> {
    let mut out = Vec::new();
    for id in ids {
        out.push(one(statements, id)?);
    }
    Ok(out)
}

fn one(
    statements: &[statement::Statement],
    id: &str,
) -> Result<statement::Statement, String> {
    let hits: Vec<&statement::Statement> =
        statements.iter().filter(|s| s.id.starts_with(id)).collect();
    match hits.as_slice() {
        [] => Err(plan::Error::Unknown(id.to_string()).to_string()),
        [only] => Ok((*only).clone()),
        many => Err(plan::Error::Ambiguous(
            id.to_string(),
            many.iter().map(|s| s.id.clone()).collect(),
        )
        .to_string()),
    }
}

fn emit_plan(
    built: &plan::Plan,
    args: &PlanArgs,
    base: &Path,
) -> Result<Output, String> {
    let mut warnings = Vec::new();
    if let Some(path) = args.out.as_deref() {
        // ANCHORED to `-C`, like every other path this command touches. A
        // relative `--out` resolved against the PROCESS directory would
        // write the plan somewhere the project it describes cannot see --
        // the same split `-C` already had to fix for corpus roots.
        let path = base.join(path);
        write_plan(&path, built)?;
        warnings.push(format!("wrote plan to {}", path.display()));
    }
    Ok(Output {
        text: render_plan(built, args.json)?,
        warnings,
    })
}

/// The path is always base-joined by the caller, so it always has a
/// parent and the directory creation needs no "if there is one" branch.
fn write_plan(path: &Path, built: &plan::Plan) -> Result<(), String> {
    let text =
        toml::to_string_pretty(built).map_err(|error| error.to_string())?;
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(path, text).map_err(|error| error.to_string())
}

fn render_plan(built: &plan::Plan, json: bool) -> Result<String, String> {
    if json {
        return serde_json::to_string_pretty(built)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(plan::render_human(built))
}

fn run_plan(flags: &[String], env: &Env) -> u8 {
    emit(plan_command(flags, env))
}

/// `apply` takes ids OR a plan file, plus `--auto-approve`.
#[derive(Debug, Default)]
pub struct ApplyArgs {
    pub ids: Vec<String>,
    /// A positional that names an EXISTING FILE is read as a plan; anything
    /// else is an id. Section I writes the two as alternatives, and an id is
    /// seven hex characters, so the collision is a file literally named
    /// `95bae35` -- at which point reading it as a plan and failing to parse
    /// is a better outcome than silently treating a plan path as an id.
    pub plan_file: Option<PathBuf>,
    pub auto_approve: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_apply(args: &[String]) -> Result<ApplyArgs, String> {
    let mut out = ApplyArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_apply_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_apply_arg<'a>(
    out: &mut ApplyArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--auto-approve" => out.auto_approve = true,
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        positional if Path::new(positional).is_file() => {
            out.plan_file = Some(PathBuf::from(positional));
        }
        id => out.ids.push(id.to_string()),
    }
    Ok(())
}

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

/// Run `apply` end to end.
///
/// Order matters and is the point: resolve the plan, CHECK IT IS FRESH
/// (V19), name every file (V7), ask (V20), and only then write.
pub fn apply_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_apply(flags)?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let loaded = load_corpus(&base, env)?;
    let path = ledger::path_in(&base);
    let held = ledger::load(&path).map_err(|error| error.to_string())?;
    let requested = resolve_plan(&args, &loaded, &held)?;
    refuse_if_stale(&requested.plan, &loaded)?;
    let outcome = staged_outcome(&requested, &held);
    let staged = Staged {
        held,
        outcome,
        path,
    };
    commit(&args, &requested.plan, &base, staged)
}

fn staged_outcome(
    requested: &Requested,
    held: &ledger::Ledger,
) -> apply::Outcome {
    let mut outcome = apply::preview(&requested.plan.steps, held);
    outcome.skipped.extend(requested.already.clone());
    outcome
}

/// A plan, plus the requested ids that were ALREADY extracted.
///
/// Those come back together because an already-extracted id has no
/// statement left in the corpus to plan from -- `apply` replaced it with a
/// pointer -- so looking it up fails, and failing is exactly what V13 says
/// must not happen.
struct Requested {
    plan: plan::Plan,
    already: Vec<String>,
}

struct Staged {
    held: ledger::Ledger,
    outcome: apply::Outcome,
    path: PathBuf,
}

/// Either the plan file the user handed us, or one built from ids.
fn resolve_plan(
    args: &ApplyArgs,
    loaded: &scan::Loaded,
    held: &ledger::Ledger,
) -> Result<Requested, String> {
    if let Some(path) = args.plan_file.as_deref() {
        return from_file(path);
    }
    if args.ids.is_empty() {
        return Err(NO_APPLY_INPUT.to_string());
    }
    let split = split_requested(&args.ids, loaded, held)?;
    let plan = plan::build(&split.chosen, &loaded.sources)
        .map_err(|error| error.to_string())?;
    Ok(Requested {
        plan,
        already: split.already,
    })
}

fn from_file(path: &Path) -> Result<Requested, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let plan = toml::from_str(&text).map_err(|error| error.to_string())?;
    Ok(Requested {
        plan,
        already: Vec::new(),
    })
}

/// Sort requested ids into "still in the corpus" and "already extracted".
///
/// An id the corpus does not hold but the LEDGER does is not an error: it
/// is work already done. `apply` replaced that statement with a pointer,
/// so there is nothing left to look up -- and V13 makes re-applying a
/// no-op at exit 0, not an "unknown id" at exit 2.
fn split_requested(
    ids: &[String],
    loaded: &scan::Loaded,
    held: &ledger::Ledger,
) -> Result<Split, String> {
    let mut out = Split::default();
    for id in ids {
        match one(&loaded.statements, id) {
            Ok(found) => out.chosen.push(found),
            Err(message) => out.already.push(known_or_fail(id, held, message)?),
        }
    }
    Ok(out)
}

#[derive(Default)]
struct Split {
    chosen: Vec<statement::Statement>,
    already: Vec<String>,
}

fn known_or_fail(
    id: &str,
    held: &ledger::Ledger,
    message: String,
) -> Result<String, String> {
    match held.find(id) {
        Some(row) => Ok(row.id.clone()),
        None => Err(message),
    }
}

pub const NO_APPLY_INPUT: &str =
    "`apply` needs ids or a plan file -- run `rekall plan` first";

/// V19. A plan whose sources moved would delete the wrong lines, so it is
/// REFUSED rather than adjusted. Re-planning is cheap and report-only.
fn refuse_if_stale(
    built: &plan::Plan,
    loaded: &scan::Loaded,
) -> Result<(), String> {
    let current: Vec<(String, Option<String>)> = loaded
        .sources
        .iter()
        .map(|(src, text)| (src.clone(), Some(text.clone())))
        .collect();
    let stale = plan::staleness(built, &current);
    if stale.is_empty() {
        return Ok(());
    }
    let reasons: Vec<String> = stale.iter().map(ToString::to_string).collect();
    Err(format!("refusing a stale plan: {}", reasons.join("; ")))
}

fn commit(
    args: &ApplyArgs,
    built: &plan::Plan,
    base: &Path,
    mut staged: Staged,
) -> Result<Output, String> {
    let pending = apply::pending(&built.steps, &staged.held);
    if pending.is_empty() {
        return report(
            &staged.outcome,
            args.json,
            vec!["nothing to do".to_string()],
            render_apply_human,
        );
    }
    let named = render_apply_human(&staged.outcome, &[]);
    approved(&consent_for(args.auto_approve, &named)?)?;
    let done = perform_apply(&pending, base, &mut staged)?;
    report(&staged.outcome, args.json, done, render_apply_human)
}

fn perform_apply(
    pending: &[plan::Step],
    base: &Path,
    staged: &mut Staged,
) -> Result<Vec<String>, String> {
    let mut done = write_all(pending, base)?;
    for step in pending {
        staged.held.record(apply::row_for(step, now()));
    }
    ledger::save(&staged.path, &staged.held)
        .map_err(|error| error.to_string())?;
    done.push(format!("recorded {} extraction(s)", pending.len()));
    Ok(done)
}

/// Unix seconds. Zero if the clock is unreadable -- a missing timestamp is
/// a worse ledger row than an old one, and neither is worth failing over.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// V7: name every file BEFORE asking, so the question is informed.
///
/// Takes the rendered names rather than an outcome, because V20 is ONE
/// rule and not one per verb: `apply` and `revert` both delete from the
/// user's private memory, and a second confirm gate written for the second
/// verb is a second place for the rule to be slightly wrong.
fn consent_for(auto_approve: bool, named: &str) -> Result<Consent, String> {
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

fn write_all(
    pending: &[plan::Step],
    base: &Path,
) -> Result<Vec<String>, String> {
    let mut done = Vec::new();
    for step in pending {
        write_artifact(base, step)?;
        done.push(format!("wrote {}", step.artifact));
    }
    edit_sources(pending, base, &mut done)?;
    Ok(done)
}

fn write_artifact(base: &Path, step: &plan::Step) -> Result<(), String> {
    let path = base.join(&step.artifact);
    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(&path, apply::artifact_text(step))
        .map_err(|error| error.to_string())
}

/// One rewrite per source file, with every span for that file applied
/// together and bottom-up. Rewriting per STEP would shift the spans of the
/// steps that follow it.
fn edit_sources(
    pending: &[plan::Step],
    base: &Path,
    done: &mut Vec<String>,
) -> Result<(), String> {
    for src in sources_touched(pending) {
        let steps: Vec<plan::Step> =
            pending.iter().filter(|s| s.src == src).cloned().collect();
        let path = base.join(&src);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| error.to_string())?;
        let edited = apply::splice_all(&text, &steps).ok_or_else(|| {
            format!("{src}: a span no longer fits -- re-run `rekall plan`")
        })?;
        std::fs::write(&path, edited).map_err(|error| error.to_string())?;
        done.push(format!("edited {src}"));
    }
    Ok(())
}

fn sources_touched(pending: &[plan::Step]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for step in pending {
        if !out.contains(&step.src) {
            out.push(step.src.clone());
        }
    }
    out
}

/// Report what a MUTATING verb did, in either format.
///
/// `apply` and `revert` differ in what they RENDER, not in how they
/// report: the machine-readable outcome goes to stdout and the list of
/// what actually happened to stderr, so `--format json` stays parseable
/// while the receipt is still visible (V17). Writing that branch twice
/// would be two rule sets of the smallest and most forgettable kind.
fn report<T: serde::Serialize>(
    outcome: &T,
    json: bool,
    done: Vec<String>,
    human: impl Fn(&T, &[String]) -> String,
) -> Result<Output, String> {
    if json {
        let text = serde_json::to_string_pretty(outcome)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string())?;
        return Ok(Output {
            text,
            warnings: done,
        });
    }
    Ok(Output {
        text: human(outcome, &done),
        warnings: Vec::new(),
    })
}

#[must_use]
pub fn render_apply_human(outcome: &apply::Outcome, done: &[String]) -> String {
    let mut out = String::new();
    for path in &outcome.writes {
        out.push_str(&format!("write   {path}\n"));
    }
    for path in &outcome.edits {
        out.push_str(&format!("edit    {path}\n"));
    }
    for id in &outcome.skipped {
        out.push_str(&format!("skip    {id} (already extracted)\n"));
    }
    for line in done {
        out.push_str(&format!("done    {line}\n"));
    }
    out
}

fn run_apply(flags: &[String], env: &Env) -> u8 {
    emit(apply_command(flags, env))
}

/// `revert` takes ONE id plus `--auto-approve` and the usual flags.
///
/// One, because section I writes it `revert <id>` and because reverting is
/// the deliberate undoing of a single decision. A batch revert would need
/// its own answer to what happens when the third of five cannot be found,
/// and that answer is better as five commands.
#[derive(Debug, Default)]
pub struct RevertArgs {
    pub id: Option<String>,
    pub auto_approve: bool,
    pub cwd: Option<PathBuf>,
    pub json: bool,
}

pub fn parse_revert(args: &[String]) -> Result<RevertArgs, String> {
    let mut out = RevertArgs::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        apply_revert_arg(&mut out, arg, &mut rest)?;
    }
    Ok(out)
}

fn apply_revert_arg<'a>(
    out: &mut RevertArgs,
    arg: &str,
    rest: &mut impl Iterator<Item = &'a String>,
) -> Result<(), String> {
    match arg {
        "--auto-approve" => out.auto_approve = true,
        "--format" => {
            out.json = parse_format(&need(arg, rest)?)? == Format::Json
        }
        "-C" => out.cwd = Some(PathBuf::from(need(arg, rest)?)),
        other if other.starts_with('-') => {
            return Err(format!("unknown flag `{other}`"));
        }
        id => return set_revert_id(out, id),
    }
    Ok(())
}

/// A second positional is a MISTAKE, not a second revert -- the same
/// reasoning `show` uses, and it matters more here: quietly reverting only
/// the first of two named ids would leave the second extraction in place
/// while the command reported success.
fn set_revert_id(out: &mut RevertArgs, id: &str) -> Result<(), String> {
    if out.id.is_some() {
        return Err(format!("`revert` takes one id, got a second: `{id}`"));
    }
    out.id = Some(id.to_string());
    Ok(())
}

pub const NO_REVERT_INPUT: &str =
    "`revert` needs an id -- copy one from `rekall log`";

/// Run `revert` end to end.
///
/// Order is the point, as it is in `apply`: find the row, compute the
/// restored bytes and REFUSE if the pointer is gone, name both files
/// (V7), ask (V20), and only then write.
pub fn revert_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_revert(flags)?;
    let id = args.id.clone().ok_or_else(|| NO_REVERT_INPUT.to_string())?;
    let base = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let mut held = Held::open(&base)?;
    let Some(row) = chosen_row(&held.ledger, &id, &base, env)? else {
        return nothing_to_revert(&id, args.json);
    };
    undo(&row, &base, &args, &mut held)
}

/// V13: nothing left to undo is a no-op at exit 0, and says so.
fn nothing_to_revert(id: &str, json: bool) -> Result<Output, String> {
    report(
        &revert::nothing_to_do(id),
        json,
        vec!["nothing to do".to_string()],
        render_revert_human,
    )
}

/// Compute, name, ask, write -- in that order (V7, V20).
fn undo(
    row: &ledger::Extracted,
    base: &Path,
    args: &RevertArgs,
    held: &mut Held,
) -> Result<Output, String> {
    let restored = restored_text(row, base)?;
    let outcome = revert::preview(row);
    let named = render_revert_human(&outcome, &[]);
    approved(&consent_for(args.auto_approve, &named)?)?;
    let done = perform_revert(row, &restored, held)?;
    report(&outcome, args.json, done, render_revert_human)
}

/// The ledger and where it lives, carried together so writing it back does
/// not need the path threaded through every call beneath it.
struct Held {
    ledger: ledger::Ledger,
    path: PathBuf,
}

impl Held {
    fn open(base: &Path) -> Result<Self, String> {
        let path = ledger::path_in(base);
        let ledger = ledger::load(&path).map_err(|error| error.to_string())?;
        Ok(Self { ledger, path })
    }
}

/// Which ledger row to undo, or `None` when there is nothing left to undo.
///
/// A prefix the ledger does not hold is NOT automatically an error. If the
/// statement is back in the corpus, this id has already been reverted --
/// and its text rehashes to the same id, which is what makes the check
/// possible at all. V13 makes that a no-op at exit 0, the mirror of
/// re-applying an id the ledger already holds. An id in NEITHER place is
/// still an error.
fn chosen_row(
    held: &ledger::Ledger,
    id: &str,
    base: &Path,
    env: &Env,
) -> Result<Option<ledger::Extracted>, String> {
    match held.matching(id).as_slice() {
        [only] => Ok(Some((*only).clone())),
        [] => already_reverted(id, base, env),
        many => Err(plan::Error::Ambiguous(
            id.to_string(),
            many.iter().map(|row| row.id.clone()).collect(),
        )
        .to_string()),
    }
}

fn already_reverted(
    id: &str,
    base: &Path,
    env: &Env,
) -> Result<Option<ledger::Extracted>, String> {
    let loaded = load_corpus(base, env)?;
    one(&loaded.statements, id).map(|_| None)
}

/// The bytes to put back, computed BEFORE anything is asked or written.
///
/// Refusing before the prompt matters twice over: asking someone to
/// approve an edit that cannot happen wastes the one deliberate act V20
/// exists to require, and it fixes the ORDER -- the source is restored
/// before the artifact is removed, so a failure never leaves the artifact
/// deleted and the statement still absent, which would lose the rule
/// outright.
struct Restored {
    path: PathBuf,
    text: String,
}

fn restored_text(
    row: &ledger::Extracted,
    base: &Path,
) -> Result<Restored, String> {
    let path = base.join(&row.src);
    let text =
        std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let text =
        revert::unsplice(&text, row).map_err(|fault| fault.to_string())?;
    Ok(Restored { path, text })
}

fn perform_revert(
    row: &ledger::Extracted,
    restored: &Restored,
    held: &mut Held,
) -> Result<Vec<String>, String> {
    std::fs::write(&restored.path, &restored.text)
        .map_err(|error| error.to_string())?;
    let mut done = vec![format!("restored {}", row.src)];
    done.push(remove_artifact(row, &restored.path)?);
    held.ledger.take(&row.id);
    ledger::save(&held.path, &held.ledger)
        .map_err(|error| error.to_string())?;
    done.push(format!("dropped the ledger row for {}", row.id));
    Ok(done)
}

/// The other half of the move (V1).
///
/// An artifact that is ALREADY GONE is reported, not an error: the corpus
/// still ends in the state the revert promised. An artifact that will not
/// delete IS an error, and the ledger row is left in place on purpose --
/// the extraction is not fully undone, and a ledger that said it was would
/// hide the leftover from `check` as well as from the user.
fn remove_artifact(
    row: &ledger::Extracted,
    source: &Path,
) -> Result<String, String> {
    let path = base_of(source, &row.src).join(&row.artifact);
    if !path.exists() {
        return Ok(format!("{} was already gone", row.artifact));
    }
    std::fs::remove_file(&path)
        .map(|()| format!("removed {}", row.artifact))
        .map_err(|cause| {
            format!(
                "{} is back in {}, but {} could not be removed: {cause}. \
                 Delete it by hand -- the ledger row is kept so `rekall check` \
                 still reports the leftover",
                row.id, row.src, row.artifact
            )
        })
}

/// The project root, recovered from the source path that was just written.
///
/// Derived rather than passed, so the artifact and the source it pairs
/// with can never be resolved against two different roots.
fn base_of(source: &Path, src: &str) -> PathBuf {
    let mut base = source.to_path_buf();
    for _ in Path::new(src).components() {
        base.pop();
    }
    base
}

#[must_use]
pub fn render_revert_human(
    outcome: &revert::Outcome,
    done: &[String],
) -> String {
    let mut out = String::new();
    if outcome.already {
        out.push_str(&format!("skip    {} (nothing to revert)\n", outcome.id));
    }
    if !outcome.restores.is_empty() {
        out.push_str(&format!("restore {}\n", outcome.restores));
        out.push_str(&format!("remove  {}\n", outcome.removes));
    }
    for line in done {
        out.push_str(&format!("done    {line}\n"));
    }
    out
}

fn run_revert(flags: &[String], env: &Env) -> u8 {
    emit(revert_command(flags, env))
}

fn run_scan(flags: &[String], env: &Env) -> u8 {
    emit(scan_command(flags, env))
}

/// What a scan produced for a caller to print.
#[derive(Debug)]
pub struct Output {
    pub text: String,
    /// Files that could not be read. NAMED, never silently dropped:
    /// reporting a smaller corpus as if it were the whole one is the skip
    /// V26 forbids.
    pub warnings: Vec<String>,
}

/// Run `scan` end to end: parse flags, resolve config, inventory, render.
pub fn scan_command(flags: &[String], env: &Env) -> Result<Output, String> {
    let args = parse_scan(flags)?;
    let cwd = args.cwd.clone().unwrap_or_else(|| env.cwd.clone());
    let resolved = resolve(&cwd)?;
    if resolved.roots.is_empty() {
        return Err(NO_SOURCES.to_string());
    }
    let outcome = inventory(&resolved, &args, &cwd, env.home.as_deref())?;
    Ok(Output {
        text: render(&outcome, &args)?,
        warnings: warnings(&outcome),
    })
}

pub const NO_SOURCES: &str = "no corpus roots configured. \
Run `rekall init` to detect them, or add [sources].roots to rekall.toml";

fn inventory(
    resolved: &Resolved,
    args: &ScanArgs,
    base: &Path,
    home: Option<&str>,
) -> Result<scan::Outcome, String> {
    let corpus = scan::Corpus {
        roots: &resolved.roots,
        globs: &resolved.globs,
        home,
        base,
    };
    scan::run(&corpus, &args.filter).map_err(|error| error.to_string())
}

fn render(outcome: &scan::Outcome, args: &ScanArgs) -> Result<String, String> {
    if args.json {
        return serde_json::to_string_pretty(&outcome.report)
            .map(|text| format!("{text}\n"))
            .map_err(|error| error.to_string());
    }
    Ok(scan::render_human(&outcome.report, args.sources))
}

fn warnings(outcome: &scan::Outcome) -> Vec<String> {
    outcome
        .unreadable
        .iter()
        .map(|path| format!("could not read {} -- skipped", path.display()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    /// A deterministic environment. Tests never read the real one: the
    /// point of `Env` is that process globals arrive as arguments.
    fn env() -> Env {
        Env {
            cwd: PathBuf::from("."),
            home: None,
        }
    }

    #[test]
    fn format_accepts_the_two_documented_values() {
        assert_eq!(parse_format("human").ok(), Some(Format::Human));
        assert_eq!(parse_format("json").ok(), Some(Format::Json));
    }

    /// V17: an unknown format is a USAGE error, never a silent fall back to
    /// prose. An agent that asked for json and got a table parses garbage.
    #[test]
    fn an_unknown_format_is_an_error_not_a_fallback() {
        assert!(parse_format("yaml").is_err());
    }

    #[test]
    fn no_flags_is_a_valid_scan() {
        let parsed = parse_scan(&args(&[]));
        assert!(parsed.is_ok());
    }

    #[test]
    fn flags_populate_the_filter() {
        let parsed = parse_scan(&args(&[
            "--class",
            "M",
            "--sharpness",
            "2",
            "--top",
            "5",
        ]));
        let filter = parsed.map(|parsed| parsed.filter).ok();
        assert_eq!(
            filter.as_ref().and_then(|f| f.class.clone()),
            Some("M".to_string())
        );
        assert_eq!(filter.as_ref().and_then(|f| f.sharpness), Some(2));
        assert_eq!(filter.and_then(|f| f.top), Some(5));
    }

    #[test]
    fn sources_and_json_are_flags_without_values() {
        let parsed = parse_scan(&args(&["--sources", "--format", "json"]));
        assert_eq!(
            parsed.map(|parsed| (parsed.sources, parsed.json)).ok(),
            Some((true, true))
        );
    }

    #[test]
    fn dash_c_sets_the_working_directory() {
        let parsed = parse_scan(&args(&["-C", "/tmp/x"]));
        assert_eq!(
            parsed.map(|parsed| parsed.cwd).ok().flatten(),
            Some(PathBuf::from("/tmp/x"))
        );
    }

    /// A truncated command line is an error rather than a default. Guessing
    /// what `--top` meant is how a scan silently reports the wrong slice.
    #[test]
    fn a_flag_missing_its_value_is_an_error() {
        assert!(parse_scan(&args(&["--top"])).is_err());
        assert!(parse_scan(&args(&["--class"])).is_err());
        assert!(parse_scan(&args(&["-C"])).is_err());
    }

    #[test]
    fn an_unknown_flag_is_an_error() {
        assert!(parse_scan(&args(&["--nope"])).is_err());
    }

    #[test]
    fn sharpness_outside_the_ladder_is_rejected() {
        assert!(parse_scan(&args(&["--sharpness", "4"])).is_err());
        assert!(parse_scan(&args(&["--sharpness", "0"])).is_err());
    }

    #[test]
    fn top_must_be_a_whole_number() {
        assert!(parse_scan(&args(&["--top", "-1"])).is_err());
        assert!(parse_scan(&args(&["--top", "many"])).is_err());
    }

    /// An absent config is fine; a broken one is not. Treating a parse
    /// error as "no config" would scan a corpus the user never described.
    #[test]
    fn an_absent_config_is_not_an_error() {
        assert!(load_optional(None).is_ok());
        assert!(
            load_optional(Some(Path::new("definitely/not/here.toml"))).is_ok()
        );
    }

    #[test]
    fn a_broken_config_is_an_error() {
        assert!(load_optional(Some(Path::new("Cargo.lock"))).is_err());
    }
    #[test]
    fn a_scan_with_no_configured_roots_says_what_to_run() {
        let outcome = scan_command(&args(&["-C", "/tmp"]), &env());
        assert!(
            outcome.err().is_some_and(|m| m.contains("rekall init")),
            "an unconfigured project must say what to run"
        );
    }

    #[test]
    fn an_unknown_flag_reaches_the_caller_as_an_error() {
        assert!(scan_command(&args(&["--nope"]), &env()).is_err());
    }

    #[test]
    fn warnings_are_empty_when_everything_was_readable() {
        assert!(warnings(&empty_outcome()).is_empty());
    }

    #[test]
    fn an_unreadable_file_is_named_in_the_warnings() {
        let outcome = scan::Outcome {
            report: scan::Report {
                rows: Vec::new(),
                sources: Vec::new(),
            },
            unreadable: vec![PathBuf::from("/secret/notes.md")],
        };
        let found = warnings(&outcome);
        assert_eq!(found.len(), 1);
        assert!(
            found
                .first()
                .is_some_and(|w| w.contains("/secret/notes.md")),
            "warnings were {found:?}"
        );
    }

    fn empty_outcome() -> scan::Outcome {
        scan::Outcome {
            report: scan::Report {
                rows: Vec::new(),
                sources: Vec::new(),
            },
            unreadable: Vec::new(),
        }
    }

    #[test]
    fn json_output_is_parseable_and_carries_the_report_anatomy() {
        let json_args = ScanArgs {
            json: true,
            ..ScanArgs::default()
        };
        assert!(
            render(&empty_outcome(), &json_args)
                .is_ok_and(|text| text.contains("\"rows\"")
                    && text.contains("\"sources\"")),
        );
    }

    #[test]
    fn human_output_is_not_json() {
        assert!(
            render(&empty_outcome(), &ScanArgs::default())
                .is_ok_and(|text| !text.contains("\"rows\"")),
        );
    }

    #[test]
    fn no_arguments_prints_usage_and_succeeds() {
        assert_eq!(decide(&args(&[])), Action::PrintUsage { code: 0 });
    }

    #[test]
    fn help_flags_print_usage() {
        assert_eq!(decide(&args(&["-h"])), Action::PrintUsage { code: 0 });
        assert_eq!(decide(&args(&["--help"])), Action::PrintUsage { code: 0 });
    }

    #[test]
    fn version_flags_are_recognized() {
        assert_eq!(decide(&args(&["-V"])), Action::PrintVersion);
        assert_eq!(decide(&args(&["--version"])), Action::PrintVersion);
    }

    #[test]
    fn scan_carries_its_remaining_flags() {
        assert_eq!(
            decide(&args(&["scan", "--top", "3"])),
            Action::Scan(args(&["--top", "3"]))
        );
    }

    /// A verb the spec defines but this build cannot perform is a BACKLOG
    /// ROW, and says so. A verb nobody has heard of is a typo. Same exit
    /// code, different message -- the distinction is the message's job.
    /// The verbs that actually do something. Kept beside the loop below so
    /// implementing a verb without dispatching it fails here.
    const IMPLEMENTED: [&str; 6] =
        ["scan", "init", "show", "plan", "apply", "revert"];

    #[test]
    fn a_specified_verb_is_unimplemented_not_unknown() {
        for verb in VERBS {
            if IMPLEMENTED.contains(&verb) {
                continue;
            }
            assert_eq!(
                decide(&args(&[verb])),
                Action::Unimplemented(verb.to_string()),
                "{verb} should be recognized"
            );
        }
    }

    #[test]
    fn an_unrecognized_verb_is_unknown() {
        assert_eq!(
            decide(&args(&["scna"])),
            Action::Unknown("scna".to_string())
        );
    }

    /// The usage text and the verb list must not drift apart. They are the
    /// same eleven verbs section I defines, so the text is checked against
    /// the const rather than proof-read.
    #[test]
    fn usage_text_names_every_verb() {
        for verb in VERBS {
            assert!(USAGE.contains(verb), "usage text is missing {verb}");
        }
    }

    #[test]
    fn usage_text_states_the_exit_codes() {
        assert!(USAGE.contains("0 ok, 1 drift or violation, 2 usage"));
    }
    #[test]
    fn usage_exits_zero_when_asked_for() {
        assert_eq!(perform(Action::PrintUsage { code: 0 }, &env()), 0);
    }

    #[test]
    fn version_exits_zero() {
        assert_eq!(perform(Action::PrintVersion, &env()), 0);
    }

    /// Both are usage errors -- section I fixes the exit set, and neither
    /// an unknown verb nor an unimplemented one is drift.
    #[test]
    fn unknown_and_unimplemented_both_exit_two() {
        assert_eq!(
            perform(Action::Unimplemented("plan".to_string()), &env()),
            USAGE_EXIT
        );
        assert_eq!(
            perform(Action::Unknown("scna".to_string()), &env()),
            USAGE_EXIT
        );
        assert_eq!(USAGE_EXIT, 2);
    }

    #[test]
    fn usage_shown_after_an_unknown_verb_still_reports_failure() {
        assert_eq!(perform(Action::Unknown("nope".to_string()), &env()), 2);
    }

    #[test]
    fn a_scan_that_cannot_run_exits_two() {
        assert_eq!(
            perform(Action::Scan(args(&["--nope"])), &env()),
            USAGE_EXIT
        );
    }
    /// A whole project on disk: a config plus a corpus file. Under
    /// `target/` so `cargo clean` removes it, named after its test so two
    /// never collide.
    fn project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("test-project").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "- never commit to `main`\n\n- when writing tests, prefer tables\n",
        );
        dir
    }

    fn run_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags =
            vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        flags.extend(extra.iter().map(|item| (*item).to_string()));
        scan_command(&flags, &env())
    }

    #[test]
    fn a_configured_project_scans_end_to_end() {
        let dir = project("end-to-end");
        let output = run_in(&dir, &[]).ok();
        assert!(
            output.as_ref().is_some_and(|out| out.text.contains("M1")),
            "output was {output:?}"
        );
        assert_eq!(output.map(|out| out.warnings.len()), Some(0));
    }

    /// Paths in the report are relative to the project, not to wherever the
    /// process happened to start. This is what keeps ids findable later.
    #[test]
    fn output_paths_are_project_relative() {
        let dir = project("relative");
        let text = text_of(&dir, &[]);
        assert!(text.contains("CLAUDE.md:1-1"), "text was {text}");
        assert!(
            !text.contains("target/test-project"),
            "the absolute temp path leaked into the report"
        );
    }

    #[test]
    fn json_output_parses_and_matches_the_human_row_count() {
        let dir = project("json");
        let human_rows = text_of(&dir, &[]).lines().count();
        assert_eq!(json_rows(&dir), human_rows);
        assert!(human_rows > 0, "the fixture must produce rows");
    }

    /// The rendered text, or an empty string if the scan failed. A failed
    /// scan then shows up as a failed ASSERTION on content rather than as
    /// a panic in a helper -- which keeps every test path executable.
    fn text_of(dir: &Path, extra: &[&str]) -> String {
        run_in(dir, extra).map(|out| out.text).unwrap_or_default()
    }

    fn json_rows(dir: &Path) -> usize {
        let text = text_of(dir, &["--format", "json"]);
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .as_ref()
            .and_then(|parsed| parsed.get("rows"))
            .and_then(|rows| rows.as_array())
            .map_or(0, std::vec::Vec::len)
    }

    #[test]
    fn the_sources_flag_adds_a_source_line() {
        let dir = project("sources");
        let plain = text_of(&dir, &[]);
        let with_sources = text_of(&dir, &["--sources"]);
        assert!(with_sources.lines().count() > plain.lines().count());
        assert!(with_sources.contains("project"));
    }

    #[test]
    fn filters_reach_through_the_command() {
        let dir = project("filter");
        let text = text_of(&dir, &["--class", "S"]);
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("S2"), "text was {text}");
    }

    #[test]
    fn an_unreadable_corpus_file_becomes_a_named_warning() {
        let dir = project("warned");
        let _ = std::fs::write(dir.join("bad.md"), [0xff_u8, 0xfe]);
        let warnings = run_in(&dir, &[])
            .map(|out| out.warnings)
            .unwrap_or_default();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings.first().is_some_and(|w| w.contains("bad.md")),
            "{warnings:?}"
        );
    }

    /// A broken config is an error, not an empty scan. Treating it as "no
    /// config" would scan a corpus the user never described.
    #[test]
    fn a_broken_project_config_stops_the_scan() {
        let dir = project("broken");
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = \"not a list\"\n",
        );
        assert!(run_in(&dir, &[]).is_err());
    }
    /// The binary's SUCCESS path: a real project, printed, exit 0. This is
    /// the arm every ordinary invocation takes, and it was the last part
    /// of the command path never executed by the suite.
    #[test]
    fn a_successful_scan_exits_zero() {
        let dir = project("exit-zero");
        let flags = vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        assert_eq!(perform(Action::Scan(flags), &env()), 0);
    }

    /// A corpus with an unreadable file still SUCCEEDS -- the warning is
    /// printed, the readable statements are reported, and the exit code
    /// stays 0. A partial corpus is not a failed run, but it must not be
    /// a silent one either.
    #[test]
    fn a_scan_with_warnings_still_exits_zero() {
        let dir = project("exit-zero-warned");
        let _ = std::fs::write(dir.join("bad.md"), [0xff_u8, 0xfe]);
        let flags = vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        assert_eq!(perform(Action::Scan(flags), &env()), 0);
    }
    #[test]
    fn init_is_dispatched() {
        assert_eq!(
            decide(&args(&["init", "--force"])),
            Action::Init(args(&["--force"]))
        );
    }

    #[test]
    fn init_flags_parse() {
        assert_eq!(
            parse_init(&args(&["--force", "--format", "json"]))
                .map(|parsed| (parsed.force, parsed.json))
                .ok(),
            Some((true, true))
        );
    }

    #[test]
    fn an_unknown_init_flag_is_an_error() {
        assert!(parse_init(&args(&["--nope"])).is_err());
        assert!(parse_init(&args(&["-C"])).is_err());
    }

    #[test]
    fn init_writes_a_config_and_names_it() {
        let dir = PathBuf::from("target").join("cli-init").join("fresh");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("CLAUDE.md"), "- a rule\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let text = init_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("root   CLAUDE.md"), "text was {text}");
    }

    /// init then scan, with nothing hand-written in between. That is the
    /// cold start, and it is the whole reason this verb exists.
    #[test]
    fn init_then_scan_works_with_no_hand_written_config() {
        let dir = PathBuf::from("target").join("cli-init").join("cold-start");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ =
            std::fs::write(dir.join("CLAUDE.md"), "- never commit to `main`\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert!(init_command(&flags, &env()).is_ok());
        let scanned = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(scanned.contains("M1"), "scan output was {scanned}");
    }

    #[test]
    fn init_refusing_to_clobber_exits_two() {
        let dir = PathBuf::from("target").join("cli-init").join("clobber");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("rekall.toml"), "[sources]\n");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert_eq!(perform(Action::Init(flags), &env()), USAGE_EXIT);
    }

    #[test]
    fn a_successful_init_exits_zero() {
        let dir = PathBuf::from("target").join("cli-init").join("exit-zero");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let flags = args(&["-C", &dir.to_string_lossy()]);
        assert_eq!(perform(Action::Init(flags), &env()), 0);
    }

    #[test]
    fn init_json_is_parseable() {
        let dir = PathBuf::from("target").join("cli-init").join("json");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let flags = args(&["-C", &dir.to_string_lossy(), "--format", "json"]);
        let text = init_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "text was {text}"
        );
    }
    fn show_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-show").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "- never commit to `main`\n\n- when writing tests, prefer tables\n",
        );
        dir
    }

    #[test]
    fn show_is_dispatched_with_its_id() {
        assert_eq!(
            decide(&args(&["show", "abc"])),
            Action::Show(args(&["abc"]))
        );
    }

    #[test]
    fn show_parses_an_id_and_flags_in_any_order() {
        assert_eq!(
            parse_show(&args(&["--format", "json", "abc"]))
                .map(|parsed| (parsed.id, parsed.json))
                .ok(),
            Some((Some("abc".to_string()), true))
        );
    }

    /// A second positional is a MISTAKE. `show a b` most likely means the
    /// shell split something, and quietly showing only `a` would answer a
    /// question nobody asked.
    #[test]
    fn a_second_id_is_an_error_not_a_second_lookup() {
        assert!(parse_show(&args(&["abc", "def"])).is_err());
    }

    #[test]
    fn show_without_an_id_says_where_to_get_one() {
        let message =
            show_command(&args(&[]), &env()).err().unwrap_or_default();
        assert!(message.contains("rekall scan"), "message was {message}");
    }

    #[test]
    fn show_prints_the_verbatim_statement() {
        let dir = show_project("verbatim");
        let text = show_first(&dir);
        assert!(text.contains("never commit"), "text was {text}");
        assert!(text.contains("class    M1"), "text was {text}");
    }

    /// Scans, takes the first id from the output, and shows it -- the
    /// copy-an-id-from-scan path a user actually takes.
    fn show_first(dir: &Path) -> String {
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.push(id.to_string());
        show_command(&show_flags, &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }

    #[test]
    fn an_ambiguous_prefix_names_the_candidates() {
        let dir = show_project("ambiguous");
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(String::new());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(
            message.contains("matches 2 statements"),
            "message was {message}"
        );
        assert!(
            message.contains("Use more characters"),
            "message was {message}"
        );
    }

    #[test]
    fn an_unknown_id_says_so() {
        let dir = show_project("unknown");
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push("zzzzzzz".to_string());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    #[test]
    fn a_failed_show_exits_two() {
        assert_eq!(perform(Action::Show(args(&["zzz"])), &env()), USAGE_EXIT);
    }
    #[test]
    fn an_unknown_show_flag_is_an_error() {
        assert!(parse_show(&args(&["--nope"])).is_err());
        assert!(parse_show(&args(&["--format"])).is_err());
    }

    #[test]
    fn show_json_is_parseable_and_carries_the_text() {
        let dir = show_project("json");
        let text = show_first_as(&dir, &["--format", "json"]);
        let parsed = serde_json::from_str::<serde_json::Value>(&text).ok();
        assert!(parsed.is_some(), "text was {text}");
        assert!(text.contains("\"text\""), "text was {text}");
    }

    fn show_first_as(dir: &Path, extra: &[&str]) -> String {
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.extend(args(extra));
        show_flags.push(id.to_string());
        show_command(&show_flags, &env())
            .map(|out| out.text)
            .unwrap_or_default()
    }

    #[test]
    fn a_successful_show_exits_zero() {
        let dir = show_project("exit-zero");
        let flags = args(&["-C", &dir.to_string_lossy()]);
        let listed = scan_command(&flags, &env())
            .map(|out| out.text)
            .unwrap_or_default();
        let id = listed.split_whitespace().next().unwrap_or_default();
        let mut show_flags = flags.clone();
        show_flags.push(id.to_string());
        assert_eq!(perform(Action::Show(show_flags), &env()), 0);
    }

    #[test]
    fn show_without_configured_roots_says_what_to_run() {
        let mut flags = args(&["-C", "/tmp"]);
        flags.push("abc".to_string());
        let message = show_command(&flags, &env()).err().unwrap_or_default();
        assert!(message.contains("rekall init"), "message was {message}");
    }
    fn plan_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-plan").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "- never commit to `main`\n\n- always prefer the simpler option\n",
        );
        dir
    }

    fn plan_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        plan_command(&flags, &env())
    }

    fn first_scan_id(dir: &Path) -> String {
        scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
            .map(|out| out.text)
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn plan_is_dispatched() {
        assert_eq!(
            decide(&args(&["plan", "abc"])),
            Action::Plan(args(&["abc"]))
        );
    }

    #[test]
    fn plan_without_ids_says_where_to_get_them() {
        let message = plan_in(Path::new("."), &[]).err().unwrap_or_default();
        assert!(message.contains("rekall scan"), "message was {message}");
    }

    #[test]
    fn plan_names_the_delete_the_write_and_the_wiring() {
        let dir = plan_project("human");
        let id = first_scan_id(&dir);
        let text = plan_in(&dir, &[&id])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("delete  CLAUDE.md:1-1"), "{text}");
        assert!(text.contains("write   .rekall/rules/"), "{text}");
        assert!(text.contains("wire "), "{text}");
    }

    /// Every id resolves BEFORE any step is built, so a typo in the second
    /// id does not hand back a partial plan for the first.
    #[test]
    fn one_bad_id_produces_no_plan_at_all() {
        let dir = plan_project("partial");
        let id = first_scan_id(&dir);
        let outcome = plan_in(&dir, &[&id, "zzzzzzz"]);
        assert!(outcome.is_err());
    }

    #[test]
    fn planning_an_unclassified_statement_is_refused() {
        let dir = plan_project("unclassified");
        let listed =
            scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
                .map(|out| out.text)
                .unwrap_or_default();
        let last = listed.lines().last().unwrap_or_default();
        let id = last.split_whitespace().next().unwrap_or_default();
        let message = plan_in(&dir, &[id]).err().unwrap_or_default();
        assert!(message.contains("unclassified"), "message was {message}");
    }

    #[test]
    fn an_unknown_plan_flag_is_an_error() {
        assert!(parse_plan(&args(&["--nope"])).is_err());
        assert!(parse_plan(&args(&["--out"])).is_err());
    }

    /// `--out` makes the plan an ARTIFACT: reviewable, diffable, and
    /// committable before a byte of corpus moves.
    #[test]
    fn out_writes_a_plan_file_that_parses_back() {
        let dir = plan_project("out");
        let id = first_scan_id(&dir);
        let result = plan_in(&dir, &[&id, "--out", "nested/x.plan"]);
        assert!(result.is_ok(), "{result:?}");
        let text = std::fs::read_to_string(dir.join("nested").join("x.plan"))
            .unwrap_or_default();
        assert!(
            toml::from_str::<plan::Plan>(&text).is_ok(),
            "plan was {text}"
        );
    }

    #[test]
    fn writing_a_plan_is_reported_as_a_warning_not_silently() {
        let dir = plan_project("reported");
        let id = first_scan_id(&dir);
        let warnings = plan_in(&dir, &[&id, "--out", "x.plan"])
            .map(|result| result.warnings)
            .unwrap_or_default();
        assert!(
            warnings.first().is_some_and(|w| w.contains("wrote plan")),
            "{warnings:?}"
        );
    }

    #[test]
    fn plan_json_is_parseable() {
        let dir = plan_project("json");
        let id = first_scan_id(&dir);
        let text = plan_in(&dir, &[&id, "--format", "json"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }

    #[test]
    fn a_successful_plan_exits_zero() {
        let dir = plan_project("exit-zero");
        let id = first_scan_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        assert_eq!(perform(Action::Plan(flags), &env()), 0);
    }

    #[test]
    fn a_failed_plan_exits_two() {
        assert_eq!(
            perform(Action::Plan(args(&["zzzzzzz"])), &env()),
            USAGE_EXIT
        );
    }

    #[test]
    fn a_plan_that_cannot_be_written_is_an_error() {
        let dir = plan_project("unwritable");
        let id = first_scan_id(&dir);
        let outcome =
            plan_in(&dir, &[&id, "--out", "/definitely/not/writable/x.plan"]);
        assert!(outcome.is_err());
    }
    #[test]
    fn an_ambiguous_plan_id_names_the_candidates() {
        let dir = plan_project("ambiguous");
        let message = plan_in(&dir, &[""]).err().unwrap_or_default();
        assert!(
            message.contains("matches 2 statements"),
            "message was {message}"
        );
    }

    /// The warning that a plan file was written goes to STDERR and the
    /// plan to stdout, so `rekall plan --out p | less` still shows the plan
    /// and the write is still announced.
    #[test]
    fn writing_a_plan_still_exits_zero() {
        let dir = plan_project("out-exit");
        let id = first_scan_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.extend(args(&["--out", "x.plan"]));
        assert_eq!(perform(Action::Plan(flags), &env()), 0);
    }
    /// `--out` is anchored to `-C` like every other path. A relative
    /// `--out` resolved against the PROCESS directory would drop the plan
    /// somewhere the project it describes cannot see.
    #[test]
    fn a_relative_out_lands_inside_the_project() {
        let dir = plan_project("relative-out");
        let id = first_scan_id(&dir);
        assert!(plan_in(&dir, &[&id, "--out", "x.plan"]).is_ok());
        assert!(
            dir.join("x.plan").is_file(),
            "plan did not land in the project"
        );
    }
    fn apply_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-apply").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n\n- background prose\n",
        );
        dir
    }

    fn apply_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        apply_command(&flags, &env())
    }

    fn rule_id(dir: &Path) -> String {
        scan_command(&args(&["-C", &dir.to_string_lossy()]), &env())
            .map(|out| out.text)
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn apply_is_dispatched() {
        assert_eq!(
            decide(&args(&["apply", "abc"])),
            Action::Apply(args(&["abc"]))
        );
    }

    /// V20: off a tty, silence is not consent.
    #[test]
    fn without_a_terminal_and_without_the_flag_apply_refuses() {
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
    fn apply_without_ids_or_a_plan_says_what_to_run() {
        let dir = apply_project("no-input");
        let message = apply_in(&dir, &["--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("rekall plan"), "message was {message}");
    }

    /// V1: the span is DELETED and a pointer left. Both halves asserted --
    /// a copy would leave the context cost unpaid.
    #[test]
    fn apply_moves_the_statement_and_leaves_a_pointer() {
        let dir = apply_project("moves");
        let id = rule_id(&dir);
        assert!(apply_in(&dir, &[&id, "--auto-approve"]).is_ok());
        let corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert!(
            !corpus.contains("never commit"),
            "source survived: {corpus}"
        );
        assert!(corpus.contains("<!-- rekall"), "no pointer: {corpus}");
        assert!(
            corpus.contains("- background prose"),
            "unrelated prose lost: {corpus}"
        );
    }

    #[test]
    fn apply_writes_the_artifact_and_records_the_ledger() {
        let dir = apply_project("artifact");
        let id = rule_id(&dir);
        assert!(apply_in(&dir, &[&id, "--auto-approve"]).is_ok());
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        let artifact = held
            .find(&id)
            .map(|row| row.artifact.clone())
            .unwrap_or_default();
        assert!(
            dir.join(&artifact).is_file(),
            "artifact missing: {artifact}"
        );
        assert_eq!(held.extracted.len(), 1);
    }

    /// V9: the ledger keeps the ORIGINAL TEXT, which is what makes `revert`
    /// a replay rather than a rewrite.
    #[test]
    fn the_ledger_keeps_the_original_text_verbatim() {
        let dir = apply_project("verbatim");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let held = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(
            held.find(&id).map(|row| row.text.clone()),
            Some("- never commit to `main`".to_string())
        );
    }

    /// V13: applying an id that is already extracted is a NO-OP at exit 0.
    /// Its statement is gone from the corpus -- `apply` replaced it with a
    /// pointer -- so the lookup that would fail must consult the ledger.
    #[test]
    fn re_applying_an_extracted_id_is_a_no_op() {
        let dir = apply_project("idempotent");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let before =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let second = apply_in(&dir, &[&id, "--auto-approve"]);
        assert!(second.is_ok(), "{second:?}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(before, after, "a second apply changed the corpus");
    }

    #[test]
    fn re_applying_exits_zero_and_says_it_skipped() {
        let dir = apply_project("idempotent-exit");
        let id = rule_id(&dir);
        let _ = apply_in(&dir, &[&id, "--auto-approve"]);
        let text = apply_in(&dir, &[&id, "--auto-approve"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("already extracted"), "{text}");
    }

    #[test]
    fn an_id_in_neither_the_corpus_nor_the_ledger_is_still_an_error() {
        let dir = apply_project("unknown");
        let message = apply_in(&dir, &["zzzzzzz", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    /// V19: the whole reason the fingerprint exists.
    #[test]
    fn a_plan_applied_after_the_corpus_moved_is_refused() {
        let dir = apply_project("stale");
        let saved = save_plan(&dir);
        shift_corpus(&dir);
        let message = apply_in(&dir, &[&saved, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("refusing a stale plan"),
            "message was {message}"
        );
    }

    fn save_plan(dir: &Path) -> String {
        let id = rule_id(dir);
        let _ = plan_command(
            &args(&["-C", &dir.to_string_lossy(), &id, "--out", "p.plan"]),
            &env(),
        );
        dir.join("p.plan").to_string_lossy().to_string()
    }

    /// Insert lines ABOVE every span, which is the edit that moves them.
    fn shift_corpus(dir: &Path) {
        let path = dir.join("CLAUDE.md");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::write(&path, format!("# shifted\n\n{text}"));
    }

    #[test]
    fn a_fresh_plan_file_applies() {
        let dir = apply_project("from-plan");
        let id = rule_id(&dir);
        let _ = plan_command(
            &args(&["-C", &dir.to_string_lossy(), &id, "--out", "p.plan"]),
            &env(),
        );
        let plan_path = dir.join("p.plan");
        let result =
            apply_in(&dir, &[&plan_path.to_string_lossy(), "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn an_unknown_apply_flag_is_an_error() {
        assert!(parse_apply(&args(&["--nope"])).is_err());
        assert!(parse_apply(&args(&["--format"])).is_err());
    }

    #[test]
    fn a_successful_apply_exits_zero() {
        let dir = apply_project("exit-zero");
        let id = rule_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.push("--auto-approve".to_string());
        assert_eq!(perform(Action::Apply(flags), &env()), 0);
    }

    #[test]
    fn a_failed_apply_exits_two() {
        assert_eq!(
            perform(
                Action::Apply(args(&["zzzzzzz", "--auto-approve"])),
                &env()
            ),
            USAGE_EXIT
        );
    }

    #[test]
    fn apply_json_is_parseable() {
        let dir = apply_project("json");
        let id = rule_id(&dir);
        let text = apply_in(&dir, &[&id, "--auto-approve", "--format", "json"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }
    /// Without the flag and without a terminal, apply REFUSES rather than
    /// proceeding. Tests are never a tty, so this is the real path CI takes.
    #[test]
    fn apply_without_approval_refuses_and_changes_nothing() {
        let dir = apply_project("unattended");
        let id = rule_id(&dir);
        let before =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let message = apply_in(&dir, &[&id]).err().unwrap_or_default();
        assert!(message.contains("--auto-approve"), "message was {message}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(before, after, "a refused apply edited the corpus");
    }

    /// A plan whose FINGERPRINT still matches but whose span does not fit
    /// -- a hand-edited or corrupt plan. The fingerprint check passes, so
    /// the splice is the last thing standing between that plan and the
    /// wrong bytes. It refuses too.
    #[test]
    fn a_plan_with_an_impossible_span_is_refused_at_the_splice() {
        let dir = apply_project("bad-span");
        let path = forge_plan(&dir, 999);
        let message = apply_in(&dir, &[&path, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("no longer fits"), "message was {message}");
    }

    /// A plan whose fingerprint is CORRECT but whose span is not. Hand
    /// editing a plan, or a corrupt one, gets past V19 -- so the splice is
    /// the last thing standing between it and the wrong bytes.
    fn forge_plan(dir: &Path, line: usize) -> String {
        let corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let forged = format!(
            "format = 1\n\n[[fingerprint]]\nsrc = \"CLAUDE.md\"\ndigest = \"{}\"\n\n\
             [[steps]]\nid = \"forged1\"\nsrc = \"CLAUDE.md\"\nline_start = {line}\n\
             line_end = {line}\ntext = \"x\"\nlabel = \"M1\"\n\
             artifact = \".rekall/rules/x.sh\"\nwiring = \"w\"\n",
            plan::digest_of(&corpus)
        );
        let path = dir.join("forged.plan");
        let _ = std::fs::write(&path, forged);
        path.to_string_lossy().to_string()
    }

    #[test]
    fn apply_reports_what_it_did_on_stderr_in_json_mode() {
        let dir = apply_project("json-warnings");
        let id = rule_id(&dir);
        let warnings =
            apply_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.warnings)
                .unwrap_or_default();
        assert!(
            warnings.iter().any(|w| w.contains("wrote ")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("edited ")),
            "{warnings:?}"
        );
    }

    #[test]
    fn a_json_apply_exits_zero_and_prints_its_warnings() {
        let dir = apply_project("json-exit");
        let id = rule_id(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.extend(args(&["--auto-approve", "--format", "json"]));
        assert_eq!(perform(Action::Apply(flags), &env()), 0);
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

    fn revert_project(name: &str) -> PathBuf {
        let dir = PathBuf::from("target").join("cli-revert").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(
            dir.join("rekall.toml"),
            "[sources]\nroots = [\".\"]\n",
        );
        let _ = std::fs::write(
            dir.join("CLAUDE.md"),
            "# Rules\n\n- never commit to `main`\n\n- background prose\n",
        );
        dir
    }

    fn revert_in(dir: &Path, extra: &[&str]) -> Result<Output, String> {
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.extend(args(extra));
        revert_command(&flags, &env())
    }

    /// Extract one statement and hand back its id and the corpus as it
    /// stood BEFORE the extraction -- which is the thing a revert has to
    /// reproduce.
    fn extracted(dir: &Path) -> (String, String) {
        extracted_from(dir, &dir.join("CLAUDE.md"))
    }

    /// The artifact an extracted id landed, read back from the ledger --
    /// the tests never hard-code the path, because the slug is derived
    /// from the statement's own words.
    fn artifact_of(dir: &Path, id: &str) -> String {
        ledger::load(&ledger::path_in(dir))
            .unwrap_or_default()
            .find(id)
            .map(|row| row.artifact.clone())
            .unwrap_or_default()
    }

    #[test]
    fn revert_is_dispatched() {
        assert_eq!(
            decide(&args(&["revert", "abc"])),
            Action::Revert(args(&["abc"]))
        );
    }

    /// V9: THE PROMISE. Extract, revert, and the file is the file that was
    /// there -- byte for byte. The ledger's verbatim text is what makes it
    /// a replay rather than a rewrite.
    #[test]
    fn revert_restores_the_original_bytes() {
        let dir = revert_project("bytes");
        let (id, before) = extracted(&dir);
        let result = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before);
    }

    /// V1, in the other direction. Restoring the source and leaving the
    /// artifact behind would leave two hand-maintained statements of one
    /// rule -- the copy V1 exists to forbid.
    #[test]
    fn revert_removes_the_artifact_and_the_ledger_row() {
        let dir = revert_project("artifact");
        let (id, _) = extracted(&dir);
        let artifact = artifact_of(&dir, &id);
        assert!(dir.join(&artifact).is_file(), "nothing was extracted");
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(
            !dir.join(&artifact).exists(),
            "the artifact survived: {artifact}"
        );
        let after = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert!(after.extracted.is_empty(), "{after:?}");
    }

    /// V20: off a tty and without the flag, revert refuses -- and the
    /// corpus is untouched. The confirm gate is one rule, not one per verb.
    #[test]
    fn revert_without_approval_refuses_and_changes_nothing() {
        let dir = revert_project("unattended");
        let (id, _) = extracted(&dir);
        let extracted_corpus =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        let message = revert_in(&dir, &[&id]).err().unwrap_or_default();
        assert!(message.contains("--auto-approve"), "message was {message}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(
            after, extracted_corpus,
            "a refused revert edited the corpus"
        );
    }

    /// V13: reverting twice is a no-op at exit 0. The statement is back in
    /// the corpus and its text rehashes to the same id, so "not in the
    /// ledger" means work already undone rather than an unknown id.
    #[test]
    fn reverting_twice_is_a_no_op() {
        let dir = revert_project("idempotent");
        let (id, before) = extracted(&dir);
        let _ = revert_in(&dir, &[&id, "--auto-approve"]);
        let second = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(second.is_ok(), "{second:?}");
        let text = second.map(|out| out.text).unwrap_or_default();
        assert!(text.contains("nothing to revert"), "{text}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before, "a second revert changed the corpus");
    }

    #[test]
    fn an_id_in_neither_the_ledger_nor_the_corpus_is_an_error() {
        let dir = revert_project("unknown");
        let message = revert_in(&dir, &["zzzzzzz", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no statement matches"),
            "message was {message}"
        );
    }

    /// A ledger holding two rows that share a prefix, written by hand
    /// because two REAL extractions get two hex ids that are not
    /// guaranteed to collide on any character.
    fn write_twin_rows(dir: &Path) {
        let _ = std::fs::create_dir_all(dir.join(ledger::DIR));
        let _ = std::fs::write(
            ledger::path_in(dir),
            "[[extracted]]\nid = \"aaa1111\"\nsrc = \"CLAUDE.md\"\n\
             line_start = 1\nline_end = 1\ntext = \"x\"\nartifact = \"a.sh\"\n\n\
             [[extracted]]\nid = \"aaa2222\"\nsrc = \"CLAUDE.md\"\n\
             line_start = 2\nline_end = 2\ntext = \"y\"\nartifact = \"b.sh\"\n",
        );
    }

    /// Section I: a prefix that matches two ids is a usage error, not a
    /// coin toss -- and it says WHICH two, so the fix is to type more
    /// characters rather than to guess.
    #[test]
    fn an_ambiguous_prefix_names_the_ids_it_matched() {
        let dir = revert_project("ambiguous");
        write_twin_rows(&dir);
        let message = revert_in(&dir, &["aaa", "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("Use more characters"), "{message}");
        assert!(message.contains("aaa1111"), "{message}");
    }

    /// The pointer is the ADDRESS. If someone deleted it by hand there is
    /// no single place the text belongs, so revert refuses -- and names
    /// what to do instead rather than only what went wrong (V28).
    #[test]
    fn a_hand_edited_pointer_refuses_and_names_the_fix() {
        let dir = revert_project("no-pointer");
        let (id, _) = extracted(&dir);
        let _ = std::fs::write(dir.join("CLAUDE.md"), "# Rules\n\n- prose\n");
        let message = revert_in(&dir, &[&id, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(
            message.contains("no longer holds the pointer"),
            "message was {message}"
        );
        assert!(message.contains("rekall log"), "no fix named: {message}");
    }

    /// An artifact someone already deleted is REPORTED, not an error: the
    /// corpus still ends in the state the revert promised.
    #[test]
    fn an_artifact_that_is_already_gone_is_reported_not_fatal() {
        let dir = revert_project("gone");
        let (id, before) = extracted(&dir);
        let _ = std::fs::remove_file(dir.join(artifact_of(&dir, &id)));
        let text = revert_in(&dir, &[&id, "--auto-approve"])
            .map(|out| out.text)
            .unwrap_or_default();
        assert!(text.contains("was already gone"), "{text}");
        let after =
            std::fs::read_to_string(dir.join("CLAUDE.md")).unwrap_or_default();
        assert_eq!(after, before);
    }

    /// An artifact that will NOT delete is an error, and the ledger row is
    /// kept on purpose: the extraction is not fully undone, and a ledger
    /// that said it was would hide the leftover from `check` too. A
    /// directory standing where the file should be is the portable way to
    /// make the removal fail.
    #[test]
    fn an_artifact_that_cannot_be_removed_is_an_error() {
        let dir = revert_project("stuck");
        let (id, _) = extracted(&dir);
        let artifact = dir.join(artifact_of(&dir, &id));
        let _ = std::fs::remove_file(&artifact);
        let _ = std::fs::create_dir_all(artifact.join("in-the-way"));
        let message = revert_in(&dir, &[&id, "--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("could not be removed"), "{message}");
        let after = ledger::load(&ledger::path_in(&dir)).unwrap_or_default();
        assert_eq!(after.extracted.len(), 1, "the row was dropped anyway");
    }

    #[test]
    fn revert_json_is_parseable() {
        let dir = revert_project("json");
        let (id, _) = extracted(&dir);
        let text =
            revert_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.text)
                .unwrap_or_default();
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );
    }

    #[test]
    fn a_json_revert_reports_what_it_did_on_stderr() {
        let dir = revert_project("json-warnings");
        let (id, _) = extracted(&dir);
        let warnings =
            revert_in(&dir, &[&id, "--auto-approve", "--format", "json"])
                .map(|out| out.warnings)
                .unwrap_or_default();
        assert!(
            warnings.iter().any(|w| w.contains("restored ")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("removed ")),
            "{warnings:?}"
        );
    }

    #[test]
    fn revert_without_an_id_says_what_to_run() {
        let dir = revert_project("no-input");
        let message = revert_in(&dir, &["--auto-approve"])
            .err()
            .unwrap_or_default();
        assert!(message.contains("rekall log"), "message was {message}");
    }

    #[test]
    fn revert_takes_one_id_and_a_second_is_an_error() {
        assert!(parse_revert(&args(&["aaa", "bbb"])).is_err());
        assert!(parse_revert(&args(&["--nope"])).is_err());
        assert!(parse_revert(&args(&["--format"])).is_err());
    }

    #[test]
    fn a_successful_revert_exits_zero() {
        let dir = revert_project("exit-zero");
        let (id, _) = extracted(&dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id);
        flags.push("--auto-approve".to_string());
        assert_eq!(perform(Action::Revert(flags), &env()), 0);
    }

    #[test]
    fn a_failed_revert_exits_two() {
        assert_eq!(
            perform(
                Action::Revert(args(&["zzzzzzz", "--auto-approve"])),
                &env()
            ),
            USAGE_EXIT
        );
    }

    /// The revert of a corpus file in a NESTED directory. The artifact is
    /// resolved against the project root, and recovering that root from
    /// the source path is the step that would silently go wrong.
    #[test]
    fn a_nested_source_file_resolves_its_artifact_from_the_root() {
        let dir = nested_project("nested");
        let nested = dir.join("docs").join("AGENTS.md");
        let (id, before) = extracted_from(&dir, &nested);
        let artifact = artifact_of(&dir, &id);
        assert!(dir.join(&artifact).is_file(), "nothing was extracted");
        let result = revert_in(&dir, &[&id, "--auto-approve"]);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(
            std::fs::read_to_string(&nested).unwrap_or_default(),
            before
        );
        assert!(
            !dir.join(&artifact).exists(),
            "the artifact resolved against the wrong root: {artifact}"
        );
    }

    /// A corpus whose only source file is one directory down.
    fn nested_project(name: &str) -> PathBuf {
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
    fn extracted_from(dir: &Path, src: &Path) -> (String, String) {
        let before = std::fs::read_to_string(src).unwrap_or_default();
        let id = rule_id(dir);
        let mut flags = args(&["-C", &dir.to_string_lossy()]);
        flags.push(id.clone());
        flags.push("--auto-approve".to_string());
        let _ = apply_command(&flags, &env());
        (id, before)
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
}
