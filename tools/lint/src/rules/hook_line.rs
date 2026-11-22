use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_lines, has_word};
use crate::modules::crate_module;
use crate::size::is_test_file;
use claims::{claims, module_path, named, Claim};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

mod claims;
mod paths;

pub const RULE: &str = "test-hook-line";

pub struct File<'a> {
    pub path: &'a str,
    pub source: &'a str,
    pub test: &'a [bool],
}

/// A file's crate, `""` for the root package.
fn package(path: &str) -> Option<&str> {
    match crate_module(path) {
        Some(m) => Some(m),
        None if path.starts_with("src/") || path.starts_with("tests/") => Some(""),
        None => None,
    }
}

/// One file, read once for every claim: its code's lines and the paths its live code names.
struct Read<'f, 'a> {
    file: &'f File<'a>,
    code: Vec<String>,
    paths: Vec<(usize, String)>,
}

pub fn check(
    files: &[File],
    crates: &[String],
    depends: &dyn Fn(&str, &str) -> bool,
) -> Vec<Finding> {
    let claimed: Vec<(&File, &str, Vec<Claim>)> = files
        .iter()
        .filter(|f| !is_test_file(f.path))
        .filter_map(|f| Some((f, crate_module(f.path)?, claims(f))))
        .filter(|(_, _, c)| !c.is_empty())
        .collect();
    if claimed.is_empty() {
        return Vec::new();
    }
    let hook_modules: BTreeSet<(&str, &str)> = claimed
        .iter()
        .flat_map(|(_, m, cs)| cs.iter().filter_map(|c| Some((*m, c.module.as_deref()?))))
        .collect();
    let hook = |f: &File| {
        let at = crate_module(f.path).zip(module_path(f.path));
        at.is_some_and(|(m, p)| hook_modules.contains(&(m, p.as_str())))
    };
    let read: Vec<Read> = files
        .iter()
        .map(|file| {
            let code = code_lines(file.source);
            let live = !is_test_file(file.path) && !hook(file);
            let paths = if live {
                paths::resolved(&code, file.test)
            } else {
                Vec::new()
            };
            let code = code.into_iter().map(|l| l.into_owned()).collect();
            Read { file, code, paths }
        })
        .collect();
    let mut found = Vec::new();
    for (file, own, file_claims) in &claimed {
        for claim in file_claims {
            let at = |message: String| Finding {
                path: file.path.to_string(),
                line: claim.line,
                rule: RULE,
                message,
            };
            let caller = read.iter().find_map(|r| {
                package(r.file.path).filter(|p| p != own && depends(p, own))?;
                let line = live_call(&r.paths, claim, own)?;
                Some(format!("{}:{line}", r.file.path))
            });
            if let Some(caller) = caller {
                found.push(at(format!(
                    "`{}` is called a test hook, but `{caller}` calls it outside tests: a hook \
                     is what only tests call; drop the line, or move that caller",
                    claim.name
                )));
                continue;
            }
            let own_module = |head: &str| {
                let root = format!("crates/{own}/src/{head}");
                let mut paths = files.iter().map(|f| f.path);
                paths.any(|p| p == format!("{root}.rs") || p.starts_with(&format!("{root}/")))
            };
            let callable = matches!(claim.kind, "fn" | "const" | "static" | "macro_rules");
            let named = if callable {
                named(&claim.text, own, crates, &own_module)
            } else {
                Vec::new()
            };
            for named in named {
                let scope = |r: &&Read| match named {
                    "" => r.file.path.starts_with("tests/"),
                    m => crate_module(r.file.path) == Some(m),
                };
                let code = |r: &Read| r.code.iter().any(|l| has_word(l, claim.name));
                if !read.iter().filter(scope).any(code) {
                    let who = if named.is_empty() {
                        "the root's `tests/`"
                    } else {
                        named
                    };
                    found.push(at(format!(
                        "the hook line names {who}, whose code never names `{}`: name the \
                         crates whose tests call it",
                        claim.name
                    )));
                }
            }
        }
    }
    found
}

/// The first line whose path resolves into the hook's crate and reaches it: a module by
/// its path, an associated item as `Type::name`, a free item from its module or one above
/// it (a re-export up the tree). `.name(` is not seen.
fn live_call(paths: &[(usize, String)], claim: &Claim, own: &str) -> Option<usize> {
    let root = format!("rusty_{own}::");
    let reaches = |path: &str| {
        let Some(rest) = path.strip_prefix(&root) else {
            return false;
        };
        let (module, name) = rest.rsplit_once("::").unwrap_or(("", rest));
        match (&claim.module, claim.owner) {
            (Some(m), _) => rest == m || rest.starts_with(&format!("{m}::")),
            (None, Some(owner)) => name == claim.name && module.rsplit("::").next() == Some(owner),
            (None, None) => {
                let above = claim.parent == module
                    || module.is_empty()
                    || claim.parent.starts_with(&format!("{module}::"));
                name == claim.name && above
            }
        }
    };
    paths
        .iter()
        .find(|(_, p)| reaches(p))
        .map(|(line, _)| *line)
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    let files: Vec<File> = scan
        .files
        .iter()
        .map(|p| File {
            path: p,
            source: scan.source(p),
            test: scan.test_lines(p),
        })
        .collect();
    let crates = crate::modules::all(scan.root());
    let deps: BTreeMap<&str, BTreeSet<String>> = std::iter::once("")
        .chain(crates.iter().map(String::as_str))
        .map(|m| {
            let at = match m {
                "" => "Cargo.toml".to_string(),
                m => format!("crates/{m}/Cargo.toml"),
            };
            let text = fs::read_to_string(scan.base.join(at)).unwrap_or_default();
            let mut named = crate::layers::manifest_dependencies(&text);
            named.extend(crate::layers::manifest_dev_dependencies(&text));
            (m, named.into_iter().collect())
        })
        .collect();
    let depends = |caller: &str, on: &str| {
        let key = format!("rusty_{}", on.replace('-', "_"));
        deps.get(caller).is_some_and(|d| d.contains(&key))
    };
    check(&files, &crates, &depends)
}

#[cfg(test)]
#[path = "hook_line_tests.rs"]
mod tests;
