//! The command-line arguments every binary shares: `--project <dir>`, a
//! `<flag> <value>` pair, and `-h` / `--help`.

use std::path::PathBuf;

/// The command-line flag every binary takes to name its project.
const PROJECT_FLAG: &str = "--project";

/// The flags that ask a binary for its usage.
const HELP_FLAGS: [&str; 2] = ["--help", "-h"];

/// `-h` or `--help` among `args`: print `usage` to stdout and exit 0. Every binary
/// calls it before it opens a project, a window or a session, so `<bin> --help` is a
/// safe first look at its options (`tests/bin_help.rs` holds it for each one).
pub fn answer_help(args: &[String], usage: &str) {
    if asks_for_help(args) {
        println!("{usage}");
        std::process::exit(0);
    }
}

/// Whether `args` holds `-h` or `--help`.
fn asks_for_help(args: &[String]) -> bool {
    args.iter().any(|a| HELP_FLAGS.contains(&a.as_str()))
}

/// Remove `--project <dir>` (or `--project=<dir>`) from `args` and return the
/// directory. `Err` when the flag has no value.
pub fn take_flag(args: &mut Vec<String>) -> Result<Option<PathBuf>, String> {
    Ok(take_value(args, PROJECT_FLAG)?.map(PathBuf::from))
}

/// Remove `<flag> <value>` (or `<flag>=<value>`) from `args` and return the value;
/// `Ok(None)` when the flag is absent. `Err` when it has no value: nothing follows
/// it, another flag does, or `<flag>=` is empty.
pub fn take_value(args: &mut Vec<String>, flag: &str) -> Result<Option<String>, String> {
    let joined = format!("{flag}=");
    let Some(at) = args
        .iter()
        .position(|a| a == flag || a.starts_with(&joined))
    else {
        return Ok(None);
    };
    let taken = args.remove(at);
    let value = match taken.strip_prefix(&joined) {
        Some(value) => value.to_string(),
        None if at < args.len() && !args[at].starts_with("--") => args.remove(at),
        None => String::new(),
    };
    if value.is_empty() {
        return Err(format!("{flag} needs a value"));
    }
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::asks_for_help;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn either_help_flag_anywhere_asks_for_the_usage() {
        assert!(asks_for_help(&args(&["--help"])));
        assert!(asks_for_help(&args(&["--project", "game", "-h"])));
        assert!(!asks_for_help(&args(&[])));
        assert!(!asks_for_help(&args(&["--helpful", "-hx", "help"])));
    }
}
