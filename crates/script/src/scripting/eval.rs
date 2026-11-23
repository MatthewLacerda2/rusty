use std::collections::{BTreeMap, BTreeSet};

use super::manager::ScriptManager;

/// Characters of a console line kept as its Undo step's name (`Editor.UndoName`).
const STEP_NAME_CHARS: usize = 60;

impl ScriptManager {
    /// A REPL line is first tried as an expression (`return <line>`) so values are
    /// echoed back; if that fails to compile it is run as a statement (assignments,
    /// `print(...)`, control flow). Multiple returned values are comma-joined.
    pub fn eval(&self, line: &str) -> Result<String, String> {
        let lua = self.lua.as_ref().ok_or_else(|| {
            "no Lua runtime: this host has none outside Play, or it failed to start \
             (see the console's `Lua init error`)"
                .to_string()
        })?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        let line = super::HostCall::Eval(trimmed.to_string());
        self.journal.borrow_mut().push(line);
        // One line, one Undo step; open hand edits become their own. Not in Play.
        let recorded = {
            let mut scene = self.cells.scene.borrow_mut();
            scene.commit_edits(None);
            let recorded = !scene.is_playing();
            if recorded {
                scene.begin_edits();
            }
            recorded
        };
        let result = eval_line(lua, trimmed).map_err(|e| e.to_string());
        if recorded {
            let step: String = trimmed.chars().take(STEP_NAME_CHARS).collect();
            self.cells.scene.borrow_mut().commit_edits(Some(&step));
        }
        result
    }

    /// A test hook: `tests/api_doc_drift.rs` and `dev`'s Lua-surface tests read it.
    pub fn api_surface(&self) -> BTreeMap<String, BTreeSet<String>> {
        let Some(lua) = self.lua.as_ref() else {
            return BTreeMap::new();
        };
        let mut surface = BTreeMap::new();
        let _ = walk_surface(lua, &mut surface);
        surface
    }
}

fn eval_line(lua: &mlua::Lua, line: &str) -> mlua::Result<String> {
    let values = match lua
        .load(format!("return {}", line))
        .eval::<mlua::MultiValue>()
    {
        Ok(v) => v,
        Err(_) => lua.load(line).eval::<mlua::MultiValue>()?,
    };
    let rendered = values
        .iter()
        .map(value_to_string)
        .collect::<Vec<_>>()
        .join(", ");
    Ok(rendered)
}

fn walk_surface(
    lua: &mlua::Lua,
    surface: &mut BTreeMap<String, BTreeSet<String>>,
) -> mlua::Result<()> {
    for pair in lua.globals().pairs::<String, mlua::Value>() {
        let (name, value) = pair?;
        let mlua::Value::Table(table) = value else {
            continue;
        };
        if LUA_STDLIB_GLOBALS.contains(&name.as_str()) {
            continue;
        }
        let mut fns = BTreeSet::new();
        for entry in table.pairs::<String, mlua::Value>() {
            let (key, val) = entry?;
            if matches!(val, mlua::Value::Function(_)) {
                fns.insert(key);
            }
        }
        surface.insert(name, fns);
    }
    Ok(())
}

/// Lua 5.4's standard globals, left out of the surface walk so the
/// drift gate compares only engine namespaces (capitalized `Debug` is the engine's,
/// not the lowercase stdlib `debug`).
const LUA_STDLIB_GLOBALS: &[&str] = &[
    "_G",
    "_VERSION",
    "assert",
    "collectgarbage",
    "dofile",
    "error",
    "getmetatable",
    "ipairs",
    "load",
    "loadfile",
    "next",
    "pairs",
    "pcall",
    "print",
    "rawequal",
    "rawget",
    "rawlen",
    "rawset",
    "select",
    "setmetatable",
    "tonumber",
    "tostring",
    "type",
    "xpcall",
    "require",
    "string",
    "table",
    "math",
    "io",
    "os",
    "coroutine",
    "utf8",
    "package",
    "debug",
];

fn value_to_string(value: &mlua::Value) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => n.to_string(),
        mlua::Value::String(s) => s.to_str().map(|s| s.to_string()).unwrap_or_default(),
        mlua::Value::Table(_) => "<table>".to_string(),
        mlua::Value::Function(_) => "<function>".to_string(),
        other => format!("<{}>", other.type_name()),
    }
}
