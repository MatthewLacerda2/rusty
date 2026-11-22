use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Table, Value};

use super::forwards;
use rusty_scene::Scene;
use rusty_script::scripting::{HostCall, HostJournal, Plain, ScriptManager};

/// The `Harness` verbs that change the world without a tick, so a demo must note them: every
/// other verb reads the world or steps it.
pub const HARNESS_VERBS: &[&str] = &["AttachPlayerBot", "LoadStress"];

// Skips `math`: its `random` shims call `Random.*`, which is wrapped, so each draw is noted once.
const WRAP: &str = r#"
local note, check, harness_verbs = ...
local skip = { string = true, table = true, coroutine = true, utf8 = true, math = true,
               Harness = true, _G = true, package = true }
local namespaces = {}
for name, value in pairs(_G) do
    if type(value) == "table" and not skip[name] then namespaces[#namespaces + 1] = name end
end
local function wrap(t, ns, name, f)
    local full = ns .. "." .. name
    rawset(t, name, function(...)
        note(full, ...)
        local r = table.pack(f(...))
        check(full, table.unpack(r, 1, r.n))
        return table.unpack(r, 1, r.n)
    end)
end
for _, ns in ipairs(namespaces) do
    local t = _G[ns]
    local fns = {}
    for name, f in pairs(t) do
        if type(f) == "function" then fns[name] = f end
    end
    for name, f in pairs(fns) do wrap(t, ns, name, f) end
end
local harness = rawget(_G, "Harness")
for _, name in ipairs(harness_verbs) do
    local f = harness and harness[name]
    if type(f) == "function" then wrap(harness, "Harness", name, f) end
end
"#;

/// Nesting a table argument may hold; deeper is taken for a cycle.
const MAX_DEPTH: usize = 32;

pub fn journal_calls(lua: &Lua, journal: &Rc<RefCell<HostJournal>>) -> mlua::Result<()> {
    let notes = Rc::clone(journal);
    let note = lua.create_function(move |_, (function, args): (String, MultiValue)| {
        let plain: Result<Vec<Plain>, &str> = args.iter().map(|v| plain(v, 0)).collect();
        notes.borrow_mut().push(match plain {
            Ok(args) => HostCall::Call { function, args },
            Err(kind) => HostCall::Taint(format!("{function} was passed a {kind}")),
        });
        Ok(())
    })?;
    let checks = Rc::clone(journal);
    let check = lua.create_function(move |_, (function, returned): (String, MultiValue)| {
        // Its handle notes its own field writes and calls (`script_ref`), so it is no taint.
        if function == "Scene.GetScript" {
            return Ok(());
        }
        if let Some(kind) = returned.iter().find_map(|v| plain(v, 0).err()) {
            let what = format!("{function} returned a {kind}, whose calls are not recorded");
            checks.borrow_mut().push(HostCall::Taint(what));
        }
        Ok(())
    })?;
    let verbs = lua.create_sequence_from(HARNESS_VERBS.iter().copied())?;
    lua.load(WRAP)
        .set_name("=demo")
        .call::<()>((note, check, verbs))
}

pub fn plain_value(value: &Value) -> Result<Plain, &'static str> {
    plain(value, 0)
}

pub(super) fn plain(value: &Value, depth: usize) -> Result<Plain, &'static str> {
    Ok(match value {
        Value::Nil => Plain::Nil,
        Value::Boolean(b) => Plain::Bool(*b),
        Value::Integer(i) => Plain::Int(*i),
        Value::Number(x) => Plain::Number(*x),
        Value::String(s) => Plain::Str(s.to_str().map_err(|_| "binary string")?.to_string()),
        Value::Table(t) if depth < MAX_DEPTH => table(t, depth)?,
        Value::Table(_) => return Err("table nested too deep"),
        Value::Function(_) => return Err("function"),
        Value::UserData(_) | Value::LightUserData(_) => return Err("handle (userdata)"),
        _ => return Err("value of another kind"),
    })
}

fn table(t: &Table, depth: usize) -> Result<Plain, &'static str> {
    let len = t.raw_len();
    let mut pairs = Vec::new();
    for pair in t.pairs::<Value, Value>() {
        let (key, value) = pair.map_err(|_| "table")?;
        let value = plain(&value, depth + 1)?;
        let key = match key {
            Value::String(s) => s.to_str().map_err(|_| "binary key")?.to_string(),
            // Zero-padded, so the sort by key below puts indices in numeric order.
            Value::Integer(i) if i >= 1 && i as usize <= len => format!("{i:020}"),
            _ => return Err("table with mixed keys"),
        };
        pairs.push((key, value));
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    if len > 0 && pairs.len() == len {
        return Ok(Plain::List(pairs.into_iter().map(|(_, v)| v).collect()));
    }
    if len > 0 {
        return Err("table with mixed keys");
    }
    Ok(Plain::Map(pairs))
}

pub(super) fn to_lua(lua: &Lua, value: &Plain) -> mlua::Result<Value> {
    Ok(match value {
        Plain::Nil => Value::Nil,
        Plain::Bool(b) => Value::Boolean(*b),
        Plain::Int(i) => Value::Integer(*i),
        Plain::Number(x) => Value::Number(*x),
        Plain::Str(s) => Value::String(lua.create_string(s)?),
        Plain::List(items) => {
            let t = lua.create_table()?;
            for (i, item) in items.iter().enumerate() {
                t.raw_set(i + 1, to_lua(lua, item)?)?;
            }
            Value::Table(t)
        }
        Plain::Map(pairs) => {
            let t = lua.create_table()?;
            for (key, item) in pairs {
                t.raw_set(key.as_str(), to_lua(lua, item)?)?;
            }
            Value::Table(t)
        }
    })
}

pub fn invoke(
    lua: &Lua,
    game: &ScriptManager,
    function: &str,
    args: &[Plain],
) -> Result<(), String> {
    let (namespace, name) = function
        .split_once('.')
        .ok_or_else(|| format!("{function} is not Namespace.Function"))?;
    let made = match forwards::find(namespace, name) {
        Some(entry) => entry.run(
            name,
            || game.with_vm(|glua| call(glua, (namespace, name), args)),
            || call(lua, (namespace, name), args),
        ),
        None => call(lua, (namespace, name), args),
    };
    made.map_err(|e| format!("{function}: {e}"))
}

fn call(lua: &Lua, (namespace, name): (&str, &str), args: &[Plain]) -> mlua::Result<()> {
    let f: Function = lua.globals().get::<Table>(namespace)?.get(name)?;
    let args: MultiValue = args
        .iter()
        .map(|a| to_lua(lua, a))
        .collect::<mlua::Result<_>>()?;
    f.call::<MultiValue>(args).map(drop)
}

pub fn invoke_harness(
    lua: &Lua,
    scene: &RefCell<Scene>,
    verb: &str,
    args: &[Plain],
) -> Result<(), String> {
    let arg = |i: usize| args.get(i).map_or(Ok(Value::Nil), |a| to_lua(lua, a));
    let done = match verb {
        "AttachPlayerBot" => match args.first() {
            Some(Plain::Str(path)) => {
                crate::botplayer::attach_player_bot(&mut scene.borrow_mut(), path);
                Ok(())
            }
            _ => Err("a script path".to_string()),
        },
        "LoadStress" => {
            let opts = match arg(0).map_err(|e| e.to_string())? {
                Value::Table(t) => Some(t),
                _ => None,
            };
            let spec = crate::bench::spec_from_lua(lua, opts).map_err(|e| e.to_string())?;
            crate::bench::stress::load(&mut scene.borrow_mut(), &spec);
            Ok(())
        }
        _ => Err("not a verb a demo replays".to_string()),
    };
    done.map_err(|e| format!("Harness.{verb}: {e}"))
}

pub(super) fn is_play_verb(act: &HostCall) -> bool {
    let named = match act {
        HostCall::Eval(line) => line.trim_end_matches(';').trim_end(),
        HostCall::Call { function, .. } => function,
        HostCall::Taint(_) => return false,
    };
    let named = named.strip_suffix("()").unwrap_or(named);
    matches!(named, "Editor.EnterPlay" | "Editor.ExitPlay")
}

#[cfg(test)]
#[path = "calls_tests.rs"]
mod tests;
