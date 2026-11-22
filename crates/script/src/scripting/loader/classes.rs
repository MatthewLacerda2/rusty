//! A file is compiled once per VM; each instance runs the bytecode in a closure of its own,
//! so a top-level `_ENV` stays private. A write-log entry naming the file evicts its class;
//! live instances keep their table. A missing or unreadable file is never cached; a compile
//! error is cached until its file is written again.

use std::collections::BTreeMap;
use std::rc::Rc;

use mlua::chunk::ChunkMode;
use mlua::{Function, Lua, Table};

use crate::scripting::schema::{read_fields, ScriptField};
use rusty_core::fs::written::{written_since, Stale};

struct Class {
    chunk: Result<Rc<[u8]>, String>,
    fields: Rc<[ScriptField]>,
}

pub(in crate::scripting) struct Instantiate {
    chunk: Rc<[u8]>,
    pub fields: Rc<[ScriptField]>,
}

impl Instantiate {
    pub(in crate::scripting) fn run(&self, lua: &Lua) -> mlua::Result<Table> {
        // Unverified bytecode: only what `read` dumped in this VM, never a file's.
        let chunk = lua.load(&*self.chunk).set_mode(ChunkMode::Binary);
        chunk.into_function()?.call(())
    }
}

pub(in crate::scripting) struct ScriptClasses {
    seen: u64,
    classes: BTreeMap<String, Class>,
    /// A test hook: the class tests count file reads with it.
    pub(in crate::scripting) reads: u64,
}

impl Default for ScriptClasses {
    fn default() -> Self {
        Self {
            seen: written_since(u64::MAX).0,
            classes: BTreeMap::new(),
            reads: 0,
        }
    }
}

impl ScriptClasses {
    pub(in crate::scripting) fn get(
        &mut self,
        lua: &Lua,
        script_path: &str,
        file: &std::path::Path,
        workspace: &std::path::Path,
        warn: impl FnOnce(String),
    ) -> Result<Instantiate, String> {
        self.evict_written();
        let key = file.to_string_lossy().into_owned();
        if !self.classes.contains_key(&key) {
            let class = self.read(lua, script_path, file, workspace, warn)?;
            self.classes.insert(key.clone(), class);
        }
        let class = &self.classes[&key];
        Ok(Instantiate {
            chunk: class.chunk.clone()?,
            fields: Rc::clone(&class.fields),
        })
    }

    fn evict_written(&mut self) {
        let (seen, mut written) = written_since(self.seen);
        self.seen = seen;
        // Filtered first, since `Stale` canonicalizes every note.
        written.retain(|p| rusty_core::paths::has_extension(p, "lua"));
        if written.is_empty() || self.classes.is_empty() {
            return;
        }
        let stale = Stale::new(written);
        self.classes.retain(|path, _| !stale.matches(path));
    }

    fn read(
        &mut self,
        lua: &Lua,
        script_path: &str,
        file: &std::path::Path,
        workspace: &std::path::Path,
        warn: impl FnOnce(String),
    ) -> Result<Class, String> {
        if !file.exists() {
            return Err(format!("Script file not found: {}", script_path));
        }
        self.reads += 1;
        let code =
            std::fs::read_to_string(file).map_err(|e| format!("Failed to read script: {}", e))?;
        let chunk = compile(lua, script_path, &code)
            .map(|f| Rc::from(f.dump(false)))
            .map_err(|e| format!("Syntax error compiling {}: {}", script_path, e));
        let fields = match &chunk {
            Ok(_) => read_fields(workspace, script_path, &code).unwrap_or_else(|e| {
                warn(format!(
                    "{script_path}: its `fields` could not be read, so the inspector \
                     shows none and none are set: {e}"
                ));
                Vec::new()
            }),
            Err(_) => Vec::new(),
        };
        Ok(Class {
            chunk,
            fields: fields.into(),
        })
    }
}

/// Expression first, so a bare-table file returns it; then a block.
fn compile(lua: &Lua, script_path: &str, code: &str) -> mlua::Result<Function> {
    let name = format!("@{script_path}");
    lua.load(format!("return {code}"))
        .set_name(name.as_str())
        .into_function()
        .or_else(|_| lua.load(code).set_name(name).into_function())
}
