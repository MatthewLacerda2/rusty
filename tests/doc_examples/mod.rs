//! The Lua examples in `docs/api/` run (#937). The drift gate (#280) proves every
//! documented function *exists*; this proves the examples an agent copies *work*:
//! a renamed argument or a changed return shape fails here instead of leaving the
//! doc wrong and every gate green.
//!
//! Each ```lua block runs in a fresh headless edit-mode session, the setup the drift
//! gate uses. Its fence says how (see `docs/api/index.md`, *Examples run*):
//! - ```lua — compiled and run; it must not raise.
//! - ```lua no-run — compiled only: it writes files, calls a provider, or leans on
//!   game-side helpers the doc leaves out.
//! - ```lua ignore — not Lua (a shape sketch); skipped.
//! - `with=a,b:Canvas+Image` — entities the block uses but doesn't create, made
//!   fresh and bound to those globals first, with the listed components.

mod checks;
mod fence;

use std::fs;
use std::path::Path;

use fence::{Block, Mode};
use rusty::dev::session::Session;

#[test]
fn every_lua_example_in_docs_api_runs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("api");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("docs/api/")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".md"))
        .collect();
    names.sort();

    let (mut ran, mut failures) = (0, Vec::new());
    for name in &names {
        let doc = fs::read_to_string(dir.join(name)).expect("doc file");
        let blocks = fence::lua_blocks(&doc).unwrap_or_else(|e| panic!("{name}: {e}"));
        for block in blocks {
            ran += usize::from(block.mode == Mode::Run);
            if let Err(e) = check(&block) {
                failures.push(format!("docs/api/{name}:{}: {e}", block.line));
            }
        }
    }
    assert!(ran > 0, "no runnable ```lua block found under docs/api/");
    assert!(
        failures.is_empty(),
        "{} docs/api/ example(s) fail — fix the example, or mark its fence \
         (`no-run`, `ignore`, `with=…`; see tests/doc_examples/mod.rs):\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// Compile `block`, then run it unless its fence says not to.
pub fn check(block: &Block) -> Result<(), String> {
    if block.mode == Mode::Ignore {
        return Ok(());
    }
    // As the evaluator does: a block may be one expression (a table shape).
    let lua = mlua::Lua::new();
    let expr = format!("return {}", block.source);
    if lua.load(&expr).into_function().is_err() {
        lua.load(&block.source)
            .into_function()
            .map_err(|e| format!("does not compile: {e}"))?;
    }
    if block.mode == Mode::NoRun {
        return Ok(());
    }
    let session = Session::new("").expect("session boots in edit mode");
    for (name, components) in &block.with {
        let mut setup = format!("{name} = Scene.CreateEntity(\"{name}\")");
        for c in components {
            setup.push_str(&format!("\nScene.AddComponent({name}, \"{c}\")"));
        }
        session
            .eval(&setup)
            .map_err(|e| format!("`with={name}` setup failed: {e}"))?;
    }
    session.eval(&block.source).map(drop)
}
