//! Lua helpers the namespace API tests share (#939).

use mlua::Lua;

/// Evaluate `code` and render every value it returns with `{:?}`, joined by
/// `", "` — a compact string to assert a getter's results against.
pub fn eval(lua: &Lua, code: &str) -> String {
    let v: mlua::MultiValue = lua.load(code).eval().unwrap();
    let parts: Vec<String> = v.iter().map(|x| format!("{x:?}")).collect();
    parts.join(", ")
}
