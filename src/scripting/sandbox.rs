//! The deterministic Lua sandbox (#443): the gameplay VM's standard library.
//!
//! The sim is a pure function of (seed, inputs, fixed dt), and scripts run inside
//! it, so the stdlib must not hand them a nondeterministic source:
//!
//! - **`os` and `io` are not loaded.** `os.time` / `os.clock` / `os.date` are the
//!   wall clock, `os.getenv` and `io` read the machine. The engine's own surfaces
//!   (`Time`, `Storage`, `Assets`, …) are the deterministic replacements.
//! - **`math.random` / `math.randomseed` are replaced** by shims over the seeded
//!   `Random` namespace, so the Lua habit keeps working but draws from the one
//!   per-World stream instead of Lua 5.4's randomly seeded generator.
//!
//! - **`require` reads the project's `assets/scripts/` only** (#996): a helper
//!   module (`require("ui.range_axis")`) is a file beside the scripts, found from
//!   the workspace root and never from the process's working directory; native
//!   (C) modules are off.
//!
//! The console REPL evaluates against this same VM, so it gets the same
//! stdlib — one surface, three callers.

use std::path::Path;

use mlua::{Lua, LuaOptions, StdLib, Table};

use crate::core::project::SCRIPTS_DIR;

/// `math.random` / `math.randomseed` rewritten over `Random`. Looked up at call
/// time, so a script caching `local random = math.random` still works: `Random`
/// is re-registered for every evaluation. Lua 5.4's argument rules are kept —
/// `math.random(m, n)` is an integer in `[m, n]` (inclusive, unlike `Random.Range`).
const MATH_SHIMS: &str = r#"
local tointeger, maxinteger, mininteger = math.tointeger, math.maxinteger, math.mininteger
math.random = function(m, n)
    if m == nil then return Random.Value() end
    if n == nil then
        if m == 0 then return Random.Range(mininteger, maxinteger) end
        m, n = 1, m
    end
    local lo, hi = tointeger(m), tointeger(n)
    if lo == nil or hi == nil then
        error("bad argument to 'random' (number has no integer representation)", 2)
    end
    if lo > hi then error("bad argument to 'random' (interval is empty)", 2) end
    if hi == maxinteger then return Random.Range(lo - 1, hi) + 1 end
    return Random.Range(lo, hi + 1)
end
math.randomseed = function(seed)
    Random.SetSeed(tointeger(seed) or math.floor(seed or 0))
end
"#;

/// A fresh gameplay VM: the safe stdlib minus `os` / `io`, with the seeded
/// `math.random` shims installed.
pub(super) fn new_sim_vm() -> Result<Lua, String> {
    let libs = StdLib::ALL_SAFE ^ (StdLib::IO | StdLib::OS);
    let lua = Lua::new_with(libs, LuaOptions::default()).map_err(|e| e.to_string())?;
    lua.load(MATH_SHIMS)
        .set_name("=sandbox")
        .exec()
        .map_err(|e| e.to_string())?;
    Ok(lua)
}

/// Point `require` at `workspace`'s scripts folder: `require("ui.range_axis")`
/// loads `<workspace>/assets/scripts/ui/range_axis.lua`, and nothing else is
/// searched.
pub(super) fn point_require(lua: &Lua, workspace: &Path) -> Result<(), String> {
    let pattern = workspace.join(SCRIPTS_DIR).join("?.lua");
    let package: Table = lua.globals().get("package").map_err(|e| e.to_string())?;
    package
        .set("path", pattern.to_string_lossy().as_ref())
        .and_then(|_| package.set("cpath", ""))
        .map_err(|e| e.to_string())
}

/// A throwaway VM for *reading* a script's returned table (its `fields` schema,
/// its callbacks) without running it. `require` hands back an empty table, so a
/// script that loads a helper module at its top level (#996) still evaluates
/// here; nothing is read from disk.
pub(super) fn inspection_vm() -> Lua {
    let lua = Lua::new();
    let stub = lua.create_function(|lua, _: mlua::MultiValue| lua.create_table());
    if let Ok(stub) = stub {
        lua.globals().set("require", stub).ok();
    }
    lua
}
