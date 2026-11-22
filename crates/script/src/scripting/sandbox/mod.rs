use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use mlua::{Lua, LuaOptions, StdLib};

use super::console::ConsoleLogs;

mod files;

pub(super) use files::install as install_file_access;

/// These run before the API registers `Random`, so each call looks `Random` up then.
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

/// Kept out of the sim by the determinism rule: `os` reads the clock, `io` the disk.
pub(super) const WITHHELD: [(&str, StdLib); 2] = [("io", StdLib::IO), ("os", StdLib::OS)];

/// Both VMs load this one set.
fn script_libs() -> StdLib {
    WITHHELD
        .iter()
        .fold(StdLib::ALL_SAFE, |libs, (_, lib)| libs & !*lib)
}

fn vm_with_script_libs() -> mlua::Result<Lua> {
    Lua::new_with(script_libs(), LuaOptions::default())
}

pub fn sandboxed_vm() -> Result<Lua, String> {
    let lua = vm_with_script_libs().map_err(|e| e.to_string())?;
    lua.load(MATH_SHIMS)
        .set_name("=sandbox")
        .exec()
        .map_err(|e| e.to_string())?;
    Ok(lua)
}

pub(super) fn new_sim_vm() -> Result<Lua, String> {
    let lua = sandboxed_vm()?;
    lua.set_named_registry_value(SIM_VM, true)
        .map_err(|e| e.to_string())?;
    Ok(lua)
}

const SIM_VM: &str = "rusty.sim_vm";

pub(crate) fn require_sim_vm(lua: &Lua, verb: &str) -> mlua::Result<()> {
    if lua.named_registry_value::<Option<bool>>(SIM_VM)? == Some(true) {
        return Ok(());
    }
    Err(mlua::Error::RuntimeError(format!(
        "{verb}: its function belongs to this VM, and timers run in the gameplay VM; \
         a scenario drives gameplay by stepping the world"
    )))
}

/// Reads a script's table without the API: `require` and calls give fresh stubs, unknown
/// globals a shared one; withheld libraries stay nil and files resolve as in Play. The seeded
/// `Random` is API, so `math.random` takes a fixed seed: defaults match every run.
pub(super) fn inspection_vm(workspace: &Path) -> mlua::Result<Lua> {
    let lua = vm_with_script_libs()?;
    let dropped = Rc::new(RefCell::new(ConsoleLogs::new()));
    files::install(&lua, workspace, &dropped).map_err(mlua::Error::runtime)?;
    let withheld = lua.create_sequence_from(WITHHELD.map(|(name, _)| name))?;
    let stub_call: mlua::Function = lua
        .load(STUB_GLOBALS)
        .set_name("=inspection")
        .eval::<mlua::Function>()?
        .call(withheld)?;
    lua.globals().set("require", stub_call)?;
    Ok(lua)
}

/// `randomseed` reads its argument as `MATH_SHIMS`' does: none is 0, never the clock.
const STUB_GLOBALS: &str = r#"
return function(names)
local withheld = {}
for _, name in ipairs(names) do withheld[name] = true end
local seed = math.randomseed
math.randomseed = function(s) seed(math.tointeger(s) or math.floor(s or 0)) end
math.randomseed(0)
local stub, shared, open = {}, {}, {}
local function give() return stub end
local function fresh() return setmetatable({}, open) end
local function no() return false end
local ops = { "add", "sub", "mul", "div", "mod", "pow", "unm", "idiv", "concat",
  "band", "bor", "bxor", "shl", "shr", "bnot" }
for _, mt in ipairs({ shared, open }) do
  mt.__index, mt.__call, mt.__lt, mt.__le = give, fresh, no, no
  for _, op in ipairs(ops) do mt["__" .. op] = give end
end
shared.__newindex = function() end
setmetatable(stub, shared)
setmetatable(_G, { __index = function(_, k) if not withheld[k] then return stub end end })
return fresh
end
"#;
