use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_lines, is_ident};
use crate::modules::crate_module;
use claims::{claims, module_path, named, Claim};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

mod claims;

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

fn test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.split('/')
        .any(|seg| matches!(seg, "tests" | "benches" | "examples"))
        || name == "tests.rs"
        || name.ends_with("_tests.rs")
}

pub fn check(
    files: &[File],
    crates: &[String],
    depends: &dyn Fn(&str, &str) -> bool,
) -> Vec<Finding> {
    let claimed: Vec<(&File, &str, Vec<Claim>)> = files
        .iter()
        .filter(|f| !test_file(f.path))
        .filter_map(|f| Some((f, crate_module(f.path)?, claims(f))))
        .collect();
    let hook_modules: BTreeSet<(&str, &str)> = claimed
        .iter()
        .flat_map(|(_, m, cs)| cs.iter().filter_map(|c| Some((*m, c.module.as_deref()?))))
        .collect();
    let hook = |f: &File| {
        let at = crate_module(f.path).zip(module_path(f.path));
        at.is_some_and(|(m, p)| hook_modules.contains(&(m, p.as_str())))
    };
    let mut found = Vec::new();
    for (file, own, file_claims) in &claimed {
        for claim in file_claims {
            let at = |message: String| Finding {
                path: file.path.to_string(),
                line: claim.line,
                rule: RULE,
                message,
            };
            let caller = files.iter().find_map(|f| {
                package(f.path).filter(|p| p != own && depends(p, own))?;
                (!test_file(f.path) && !hook(f)).then_some(())?;
                let line = live_call(f, claim, own)?;
                Some(format!("{}:{line}", f.path))
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
                let scope = |f: &&File| match named {
                    "" => f.path.starts_with("tests/"),
                    m => crate_module(f.path) == Some(m),
                };
                let code = |f: &File| code_lines(f.source).iter().any(|l| word(l, claim.name));
                if !files.iter().filter(scope).any(code) {
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

/// A call outside tests, as far as names show it: a module by its whole path, an
/// associated item as `Type::name`, a free one as `<module>::name` or by a `use`; a
/// re-export is none, and `.name(` is not seen.
fn live_call(file: &File, claim: &Claim, own: &str) -> Option<usize> {
    let code = code_lines(file.source);
    let roots = [format!("rusty_{own}"), format!("rusty::{own}")];
    let crate_named = code.iter().any(|l| roots.iter().any(|r| word(l, r)));
    let owner_named = claim.owner.is_none_or(|o| code.iter().any(|l| word(l, o)));
    if !crate_named || !owner_named {
        return None;
    }
    let name = claim.name;
    code.iter().enumerate().find_map(|(i, line)| {
        let tested = file.test.get(i).copied().unwrap_or(false);
        if tested || line.trim_start().starts_with("pub use") {
            return None;
        }
        let hit = match (&claim.module, claim.owner) {
            (Some(path), _) => roots.iter().any(|r| word(line, &format!("{r}::{path}"))),
            (None, Some(owner)) => word(line, &format!("{owner}::{name}")),
            (None, None) => {
                let mut heads = claim.parent.split("::").filter(|h| !h.is_empty());
                let qualified = heads.any(|h| last(line, &format!("{h}::{name}")))
                    || roots.iter().any(|r| last(line, &format!("{r}::{name}")));
                let listed = line.rsplit_once('{').map_or("", |(_, l)| l);
                let imported = line.trim_start().starts_with("use ")
                    && roots.iter().any(|r| line.contains(r.as_str()))
                    && (last(line, &format!("::{name}")) || last(listed, name));
                qualified || imported
            }
        };
        hit.then_some(i + 1)
    })
}

fn last(line: &str, path: &str) -> bool {
    line.match_indices(path).any(|(at, _)| {
        let (before, after) = (&line[..at], &line[at + path.len()..]);
        let open = !(path.starts_with(is_ident) && before.ends_with(is_ident));
        open && !after.starts_with(is_ident) && !after.starts_with("::")
    })
}

fn word(line: &str, name: &str) -> bool {
    let first = name.chars().next().is_some_and(is_ident);
    let last = name.chars().next_back().is_some_and(is_ident);
    line.match_indices(name).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + name.len()..].chars().next();
        let touches = |c: Option<char>| c.is_some_and(is_ident);
        !(first && touches(before) || last && touches(after))
    })
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
    let manifests: BTreeMap<&str, String> = std::iter::once("")
        .chain(crates.iter().map(String::as_str))
        .map(|m| {
            let at = match m {
                "" => "Cargo.toml".to_string(),
                m => format!("crates/{m}/Cargo.toml"),
            };
            (
                m,
                fs::read_to_string(scan.base.join(at)).unwrap_or_default(),
            )
        })
        .collect();
    let depends = |caller: &str, on: &str| {
        let text = manifests.get(caller).map_or("", String::as_str);
        super::scan::names(text, &format!("rusty-{on}"))
    };
    check(&files, &crates, &depends)
}

#[cfg(test)]
#[path = "hook_line_tests.rs"]
mod tests;
