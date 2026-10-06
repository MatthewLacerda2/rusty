//! Build script: assemble the scripting-API reference (#569).
//!
//! The reference lives in `docs/api/` — `index.md` plus one file per Lua namespace, so
//! two branches documenting different namespaces never touch the same file. The drift
//! gates and the MCP resource still want it whole, so this concatenates the directory
//! into `$OUT_DIR/scripting-api.md`: the index first, then every namespace file in the
//! order the index links them, then any file the index forgot (sorted). Consumers embed
//! it with `include_str!(concat!(env!("OUT_DIR"), "/scripting-api.md"))`.
//!
//! It also embeds the engine's git commit as `RUSTY_ENGINE_COMMIT` (#853), which a
//! project's `project.rusty` records — see [`engine_commit`].

use std::{env, fs, path::Path, process::Command};

fn main() {
    engine_commit();
    let dir = Path::new("docs/api");
    println!("cargo:rerun-if-changed={}", dir.display());
    let index = fs::read_to_string(dir.join("index.md")).expect("docs/api/index.md");

    let mut order: Vec<String> = index
        .split("](")
        .skip(1)
        .filter_map(|rest| rest.split_once(')'))
        .map(|(target, _)| target.to_string())
        .filter(|t| t.ends_with(".md") && !t.contains('/') && dir.join(t).is_file())
        .collect();
    let mut rest: Vec<String> = fs::read_dir(dir)
        .expect("docs/api/")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".md") && n != "index.md" && !order.contains(n))
        .collect();
    rest.sort();
    order.dedup();
    order.extend(rest);

    let mut doc = index;
    for name in &order {
        let text = fs::read_to_string(dir.join(name)).expect("docs/api file");
        doc.push_str("\n---\n\n");
        doc.push_str(&text);
    }
    let out = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("scripting-api.md");
    fs::write(out, doc).expect("write the assembled scripting-API reference");
}

/// Export `RUSTY_ENGINE_COMMIT`: `git rev-parse HEAD`, suffixed `-dirty` when the tree
/// had changes, or `unknown` without git (a source tarball still builds).
///
/// It reruns only when HEAD moves — `HEAD` itself (a checkout) and the ref it points
/// to (a commit) — never on every edited file, so normal edits don't rerun it. The
/// price: `-dirty` is as fresh as the last commit or checkout, not the last edit.
fn engine_commit() {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git").args(args).output().ok()?;
        let text = String::from_utf8(out.stdout).ok()?;
        out.status.success().then(|| text.trim().to_string())
    };
    let Some(commit) = git(&["rev-parse", "HEAD"]) else {
        println!("cargo:rustc-env=RUSTY_ENGINE_COMMIT=unknown");
        return;
    };
    let dirty =
        git(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    let suffix = if dirty { "-dirty" } else { "" };
    println!("cargo:rustc-env=RUSTY_ENGINE_COMMIT={commit}{suffix}");

    // A rerun-if-changed path that doesn't exist reruns every build, so only name
    // the files git really has (a packed ref has no loose file; packed-refs covers it).
    let head_ref = git(&["symbolic-ref", "-q", "HEAD"]);
    let watched = ["HEAD", head_ref.as_deref().unwrap_or("HEAD"), "packed-refs"];
    for name in watched {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            if Path::new(&path).exists() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
}
