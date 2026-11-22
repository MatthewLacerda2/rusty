use mlua::{Lua, LuaOptions, StdLib};

mod files;

pub(super) use files::install as install_file_access;

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

pub fn sandboxed_vm() -> Result<Lua, String> {
    let libs = StdLib::ALL_SAFE ^ (StdLib::IO | StdLib::OS);
    let lua = Lua::new_with(libs, LuaOptions::default()).map_err(|e| e.to_string())?;
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

pub(super) fn inspection_vm() -> Lua {
    let lua = Lua::new();
    let stub = lua.create_function(|lua, _: mlua::MultiValue| lua.create_table());
    if let Ok(stub) = stub {
        lua.globals().set("require", stub).ok();
    }
    lua.load(STUB_GLOBALS).exec().ok();
    lua
}

const STUB_GLOBALS: &str = r#"
local stub = {}
local function give() return stub end
local mt = { __index = give, __call = give, __newindex = function() end }
for _, op in ipairs({ "add", "sub", "mul", "div", "mod", "pow", "unm", "idiv", "concat" }) do
  mt["__" .. op] = give
end
setmetatable(stub, mt)
setmetatable(_G, { __index = give })
"#;
