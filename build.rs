//! Build script: assemble the scripting-API reference (#569).
//!
//! The reference lives in `docs/api/` — `index.md` plus one file per Lua namespace, so
//! two branches documenting different namespaces never touch the same file. The drift
//! gates and the MCP resource still want it whole, so this concatenates the directory
//! into `$OUT_DIR/scripting-api.md`: the index first, then every namespace file in the
//! order the index links them, then any file the index forgot (sorted). Consumers embed
//! it with `include_str!(concat!(env!("OUT_DIR"), "/scripting-api.md"))`.

use std::{env, fs, path::Path};

fn main() {
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
