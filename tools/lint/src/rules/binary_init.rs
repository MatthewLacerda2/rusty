use super::finding::Finding;
use super::scan::Scan;
use crate::code::bare_lines;

pub const RULE: &str = "binary-init";

fn is_binary_root(path: &str) -> bool {
    if path == "src/main.rs" {
        return true;
    }
    let Some(rest) = path.strip_prefix("src/bin/") else {
        return false;
    };
    match rest.split_once('/') {
        None => rest.ends_with(".rs"),
        Some((_, file)) => file == "main.rs",
    }
}

pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if !is_binary_root(path) {
        return Vec::new();
    }
    let code = bare_lines(source);
    let Some(main) = code.iter().position(|l| l.contains("fn main(")) else {
        return Vec::new();
    };
    let first = code[main + 1..]
        .iter()
        .map(|l| l.trim())
        .find(|l| !l.is_empty());
    if first.is_some_and(|l| l.starts_with("rusty::init();")) {
        return Vec::new();
    }
    vec![Finding {
        path: path.to_string(),
        line: main + 1,
        rule: RULE,
        message: "a binary's `fn main` calls `rusty::init();` first thing: it starts the \
                  logger and, in a `dev` build, warns when engine/ is not the content this \
                  binary was built with; `rusty::logging::init()` alone skips that check and \
                  its run reports read as unstamped"
            .to_string(),
    }]
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "binary_init_tests.rs"]
mod tests;
