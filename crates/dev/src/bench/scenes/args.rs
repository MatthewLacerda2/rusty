//! `bench-scenes`' command line (`src/bin/bench_scenes.rs`), read into [`Args`]:
//! what the parent is asked for and what it passes each child.

use std::path::PathBuf;

use rusty_core::project::{take_flag, take_value};
use rusty_core::settings::video::parse_size;

use super::set::Overrides;

/// One invocation's flags, the out dir left in [`Args::rest`].
#[derive(Debug, Default)]
pub struct Args {
    pub project: Option<PathBuf>,
    /// `--scenes a,b`: the scenes a parent benches.
    pub list: Option<String>,
    /// `--scene a`: the one scene a child benches.
    pub one: Option<String>,
    pub size: Option<(u32, u32)>,
    pub runs: Option<u32>,
    pub before: Option<PathBuf>,
    /// A child's `--import-out` and `--import-ms` (`scenes::import`).
    pub import_out: Option<PathBuf>,
    pub import_ms: Option<f64>,
    pub profile: bool,
    pub import: bool,
    /// `--set key=value`, each (`super::set`).
    pub set: Overrides,
    /// `--pending-imports`: list the images an import would encode and exit.
    pub pending_imports: bool,
    /// What is left: the out dir.
    pub rest: Vec<String>,
}

/// Read `args`, refusing a flag without its value or a malformed one.
pub fn parse(mut args: Vec<String>) -> Result<Args, String> {
    let project = take_flag(&mut args)?;
    let size = take_value(&mut args, "--size")?
        .map(|s| parse_size(&s))
        .transpose()?;
    let runs = take_value(&mut args, "--runs")?.map(|r| match r.parse::<u32>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("--runs wants a whole number above 0, not {r}")),
    });
    let mut pairs = Vec::new();
    while let Some(pair) = take_value(&mut args, "--set")? {
        pairs.push(pair);
    }
    let import_ms = take_value(&mut args, "--import-ms")?.and_then(|v| v.parse::<f64>().ok());
    Ok(Args {
        project,
        list: take_value(&mut args, "--scenes")?,
        one: take_value(&mut args, "--scene")?,
        size,
        runs: runs.transpose()?,
        before: take_value(&mut args, "--before")?.map(PathBuf::from),
        import_out: take_value(&mut args, "--import-out")?.map(PathBuf::from),
        import_ms,
        profile: take_switch(&mut args, "--profile"),
        import: take_switch(&mut args, "--import"),
        set: Overrides::parse(&pairs)?,
        pending_imports: take_switch(&mut args, "--pending-imports"),
        rest: args,
    })
}

/// Remove `flag` from `args`, saying whether it was there.
fn take_switch(args: &mut Vec<String>, flag: &str) -> bool {
    let at = args.iter().position(|a| a == flag);
    at.map(|i| args.remove(i)).is_some()
}
