//! src/bin/game.rs — generate and play-test a test game (dev-only, `--features dev`, #2029).
//!
//! ```text
//! Usage:
//!   cargo run --bin game --features dev -- <name> [--out <dir>] [--param key=value]... [--test] [--games <dir>]
//!   cargo run --bin game --features dev -- --list [--games <dir>]
//! ```
//!
//! Builds the game `games/<name>/` describes into a project folder (default the
//! worktree's `target/games/<name>/`; `make game NAME=<name>`), by running its
//! `recipe.lua` in edit mode with the `--param`s (`dev::game::generate`). With
//! `--test` it also proves the generation deterministic, runs every scenario,
//! replays the demo each one recorded, and takes the shots (`dev::game::check`; `make game-test NAME=<name>`), printing one
//! line per step and exiting 1 when one failed. `--games <dir>` reads the games from
//! another folder than the checkout's `games/`. `session` and `play` are the two
//! halves of this, but neither runs a recipe file with parameters and reports its
//! failure in an exit status, which a Makefile target and a test need.

use std::path::PathBuf;
use std::process::exit;

use rusty::dev::game::{self, Params};

const USAGE: &str =
    "usage: game <name> [--out <dir>] [--param key=value]... [--test] [--games <dir>]
       game --list [--games <dir>]";

fn main() {
    rusty::logging::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let games = rusty::core::project::take_value(&mut args, "--games")
        .unwrap_or_else(|e| usage(&e))
        .map_or_else(game::games_dir, |dir| {
            std::path::absolute(dir).unwrap_or_default()
        });
    if args == ["--list"] {
        for found in game::all(&games).unwrap_or_else(|e| fail(&e)) {
            println!("{}", found.name);
        }
        return;
    }
    let out = rusty::core::project::take_value(&mut args, "--out").unwrap_or_else(|e| usage(&e));
    let test = take_switch(&mut args, "--test");
    let mut pairs = Vec::new();
    while let Some(pair) =
        rusty::core::project::take_value(&mut args, "--param").unwrap_or_else(|e| usage(&e))
    {
        pairs.push(pair);
    }
    let [name] = args.as_slice() else { usage("") };
    let params = Params::parse(pairs.iter().map(String::as_str)).unwrap_or_else(|e| usage(&e));
    let found = game::find(&games, name).unwrap_or_else(|e| fail(&e));
    let out = out.map_or_else(|| game::default_out(name), PathBuf::from);

    if !test {
        game::generate(&found, &out, &params).unwrap_or_else(|e| fail(&e));
        println!("game: generated {} into {}", found.name, out.display());
        return;
    }
    let report = game::check(&found, &out, &params).unwrap_or_else(|e| fail(&e));
    for line in &report.lines {
        println!("{line}");
    }
    println!(
        "game: {} {}",
        found.name,
        if report.passed { "passed" } else { "FAILED" }
    );
    if !report.passed {
        exit(1);
    }
}

/// Remove `flag` from `args`, saying whether it was there.
fn take_switch(args: &mut Vec<String>, flag: &str) -> bool {
    let before = args.len();
    args.retain(|a| a != flag);
    args.len() != before
}

/// A command line it cannot read: print `error` (or nothing) and the usage, exit 2.
fn usage(error: &str) -> ! {
    match error.is_empty() {
        true => eprintln!("{USAGE}"),
        false => eprintln!("game: {error}\n{USAGE}"),
    }
    exit(2);
}

/// A step that could not run: print `error` and exit 2.
fn fail(error: &str) -> ! {
    eprintln!("game: {error}");
    exit(2);
}
