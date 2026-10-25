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
//! - **`require`, `dofile` and `loadfile` read against the project** (#996,
//!   #1062; [`files`]): `require` searches the project's scripts folder, then the
//!   engine's bundled scripts, then `package.path`; a read outside the project is
//!   allowed but warns once per file in the console. Native (C) modules are off.
//!
//! The console REPL evaluates against this same VM, so it gets the same
//! stdlib — one surface, three callers. The `play` harness's scenario VM is a
//! separate VM built from the same [`sandboxed_vm`] (#1408): a play-test's
//! results are a pure function of its scenario too.

use mlua::{Lua, LuaOptions, StdLib};

mod files;

pub(super) use files::install as install_file_access;

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

/// A fresh deterministic VM: the safe stdlib minus `os` / `io`, with the seeded
/// `math.random` shims installed. `math.random` draws through whatever `Random`
/// the caller registers; the scenario VM registers the harness world's own.
pub(crate) fn sandboxed_vm() -> Result<Lua, String> {
    let libs = StdLib::ALL_SAFE ^ (StdLib::IO | StdLib::OS);
    let lua = Lua::new_with(libs, LuaOptions::default()).map_err(|e| e.to_string())?;
    lua.load(MATH_SHIMS)
        .set_name("=sandbox")
        .exec()
        .map_err(|e| e.to_string())?;
    Ok(lua)
}

/// A fresh gameplay VM: [`sandboxed_vm`], marked as the one the timer scheduler
/// runs.
pub(super) fn new_sim_vm() -> Result<Lua, String> {
    let lua = sandboxed_vm()?;
    lua.set_named_registry_value(SIM_VM, true)
        .map_err(|e| e.to_string())?;
    Ok(lua)
}

/// The registry mark of a gameplay VM, the one VM the timer scheduler runs.
const SIM_VM: &str = "rusty.sim_vm";

/// Refuse a callback `verb` would hand the timer scheduler from any VM but a
/// gameplay one (#1384). The scheduler resumes and calls its work in the gameplay
/// VM, and a Lua function never leaves the VM that made it: one from the scenario
/// VM would never run.
pub(crate) fn require_sim_vm(lua: &Lua, verb: &str) -> mlua::Result<()> {
    if lua.named_registry_value::<Option<bool>>(SIM_VM)? == Some(true) {
        return Ok(());
    }
    Err(mlua::Error::RuntimeError(format!(
        "{verb}: its function belongs to this VM, and timers run in the gameplay VM; \
         a scenario drives gameplay by stepping the world"
    )))
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
