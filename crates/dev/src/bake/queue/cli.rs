//! `export --bake <scenes|all> [--except <scene>...]` and `export --bake-resume`: the queue
//! with no window, for a cron line or an OS service. One summary line per scene goes to
//! stdout; the bake's progress goes to stderr.

use std::path::Path;
use std::time::Instant;

use super::live::run_beside;
use super::{resolve, Resumed, Runner, Selection};
use crate::bake::progress::{BakeLog, Line};

pub const USAGE: &str =
    "export [--project <dir>] --bake <scene>... | --bake all [--except <scene>...] | --bake-resume";

#[derive(Debug, PartialEq)]
pub enum Command {
    Bake(Selection),
    Resume,
}

/// `None` when no bake flag is given: the args are another `export` verb's.
pub fn parse(args: &[String]) -> Result<Option<Command>, String> {
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if !has("--bake") && !has("--bake-resume") {
        return Ok(None);
    }
    if has("--bake-resume") {
        return match args {
            [one] if one == "--bake-resume" => Ok(Some(Command::Resume)),
            _ => Err("--bake-resume takes nothing else".into()),
        };
    }
    let (mut scenes, mut except, mut into) = (Vec::new(), Vec::new(), None);
    for arg in args {
        match arg.as_str() {
            "--bake" => into = Some(false),
            "--except" => into = Some(true),
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}")),
            path => match into {
                Some(false) => scenes.push(path.to_string()),
                Some(true) => except.push(path.to_string()),
                None => return Err(format!("{path}: a scene goes after --bake")),
            },
        }
    }
    match (scenes.as_slice(), except.is_empty()) {
        ([], _) => Err("--bake names the scenes, or all".into()),
        ([all], _) if all == "all" => Ok(Some(Command::Bake(Selection::All { except }))),
        (_, true) => Ok(Some(Command::Bake(Selection::List(scenes)))),
        (_, false) => Err("--except goes with --bake all".into()),
    }
}

/// The queue run in the open project (the working directory); the process's exit status:
/// 1 when a scene ended `failed`, 2 when the queue could not start.
pub fn run(command: Command) -> i32 {
    let root = Path::new(".");
    let started = match command {
        Command::Bake(selection) => Runner::start(root, resolve(root, &selection)),
        Command::Resume => match Runner::resume(root) {
            Ok(Resumed::Runner(runner)) => Ok(runner),
            Ok(Resumed::Nothing(why)) => {
                println!("bake: nothing to resume: {why}");
                return 0;
            }
            Err(e) => Err(e),
        },
    };
    let mut runner = match started {
        Ok(runner) => runner,
        Err(e) => {
            eprintln!("export: {e}");
            return 2;
        }
    };
    let start = Instant::now();
    let mut log = BakeLog::new(move || start.elapsed(), |l: Line| eprintln!("{}", l.text));
    let ran = run_beside(&mut runner, &[], &mut |line| eprintln!("{line}"), &mut log);
    drop(log);
    if let Err(e) = ran {
        eprintln!("export: {e}");
        return 2;
    }
    for line in runner.summary().lines() {
        println!("bake: {line}");
    }
    i32::from(runner.queue().failed() > 0)
}
