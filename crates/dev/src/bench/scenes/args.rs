//! `bench-scenes`' flags. `src/bin/bench_scenes/children.rs` passes each child process its own:
//! rename a flag in both.

use std::path::PathBuf;

use rusty_core::project::{take_flag, take_value};
use rusty_core::settings::video::parse_size;

use super::set::Overrides;

#[derive(Debug, Default)]
pub struct Args {
    pub project: Option<PathBuf>,
    pub list: Option<String>,
    pub one: Option<String>,
    pub size: Option<(u32, u32)>,
    pub runs: Option<u32>,
    pub load_runs: Option<u32>,
    pub load_out: Option<PathBuf>,
    pub loads: Option<PathBuf>,
    pub before: Option<PathBuf>,
    pub import_out: Option<PathBuf>,
    pub import_ms: Option<f64>,
    pub editor_out: Option<PathBuf>,
    pub editor: Option<PathBuf>,
    pub no_editor: bool,
    pub profile: bool,
    pub import: bool,
    pub script: bool,
    pub set: Overrides,
    pub pending_imports: bool,
    pub rays: bool,
    pub rest: Vec<String>,
}

pub fn parse(mut args: Vec<String>) -> Result<Args, String> {
    let project = take_flag(&mut args)?;
    let size = take_value(&mut args, "--size")?
        .map(|s| parse_size(&s))
        .transpose()?;
    let runs = whole(take_value(&mut args, "--runs")?, "--runs");
    let load_runs = whole(take_value(&mut args, "--load-runs")?, "--load-runs");
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
        load_runs: load_runs.transpose()?,
        load_out: take_value(&mut args, "--load-out")?.map(PathBuf::from),
        loads: take_value(&mut args, "--loads")?.map(PathBuf::from),
        before: take_value(&mut args, "--before")?.map(PathBuf::from),
        import_out: take_value(&mut args, "--import-out")?.map(PathBuf::from),
        import_ms,
        editor_out: take_value(&mut args, "--editor-out")?.map(PathBuf::from),
        editor: take_value(&mut args, "--editor")?.map(PathBuf::from),
        no_editor: take_switch(&mut args, "--no-editor"),
        profile: take_switch(&mut args, "--profile"),
        import: take_switch(&mut args, "--import"),
        script: take_switch(&mut args, "--script"),
        set: Overrides::parse(&pairs)?,
        pending_imports: take_switch(&mut args, "--pending-imports"),
        rays: take_switch(&mut args, "--rays"),
        rest: args,
    })
}

fn whole(value: Option<String>, flag: &str) -> Option<Result<u32, String>> {
    value.map(|r| match r.parse::<u32>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!("{flag} wants a whole number above 0, not {r}")),
    })
}

fn take_switch(args: &mut Vec<String>, flag: &str) -> bool {
    let at = args.iter().position(|a| a == flag);
    at.map(|i| args.remove(i)).is_some()
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
