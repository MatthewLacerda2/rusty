//! src/scripting/loader/classes.rs — the script-class cache (#1710): a script file
//! is read and compiled once per VM, and every instance of it calls the compiled
//! chunk. Unity's analog is a MonoBehaviour class compiled once, `AddComponent`
//! allocating an instance; Defold and Luau cache a chunk per script asset the same
//! way.
//!
//! A class holds the compiled chunk (a [`Function`] of this VM's Lua state, so the
//! cache lives and dies with the VM: [`super::super::ScriptManager::init_runtime`]
//! and `shutdown` clear it) and the `fields` schema, parsed once in the
//! inspection VM, the same parse the inspector shows. An instance re-runs the chunk
//! for a fresh table, so a script's top level runs once per instance, as before.
//!
//! **Revisions.** A class is keyed by the resolved path and evicted when the write
//! log (`core::fs::written`, #1207) names its file: an engine write, or in the
//! editor an outside edit its file watch saw (#1327). Every Play starts a fresh
//! VM, so it re-reads every script whatever the log says. Live instances keep the
//! table they were made with; the next instance gets the new revision.
//!
//! **Failures.** A file that is missing or unreadable is not cached: the next
//! spawn tries again (`core::fs::written`'s rule, a failed load is a miss). A file
//! that does not compile is cached as its error until its revision changes, so a
//! broken script is compiled once, and each instance still reports the error.

use std::collections::BTreeMap;
use std::rc::Rc;

use mlua::{Function, Lua};

use crate::core::fs::written::{written_since, Stale};
use crate::scripting::schema::{read_fields, ScriptField};

/// One script file at one revision.
struct Class {
    /// The compiled chunk, or the error compiling it gave.
    chunk: Result<Function, String>,
    fields: Rc<[ScriptField]>,
}

/// What an instance is made from: the chunk to call and the schema to apply.
pub(in crate::scripting) struct Instantiate {
    pub chunk: Function,
    pub fields: Rc<[ScriptField]>,
}

/// One VM's classes, by resolved path.
pub(in crate::scripting) struct ScriptClasses {
    /// How far into the write log the classes are current.
    seen: u64,
    classes: BTreeMap<String, Class>,
    /// Script files read from disk, for the test that a thousand instances of one
    /// script read it once.
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
    /// The class of `script_path` (resolved to `file`) in `lua`, read and compiled
    /// on first use. `warn` hears, once per revision, that the `fields` schema could
    /// not be read (#1709).
    pub(in crate::scripting) fn get(
        &mut self,
        lua: &Lua,
        script_path: &str,
        file: &std::path::Path,
        warn: impl FnOnce(String),
    ) -> Result<Instantiate, String> {
        self.evict_written();
        let key = file.to_string_lossy().into_owned();
        if !self.classes.contains_key(&key) {
            let class = self.read(lua, script_path, file, warn)?;
            self.classes.insert(key.clone(), class);
        }
        let class = &self.classes[&key];
        let chunk = class.chunk.clone()?;
        Ok(Instantiate {
            chunk,
            fields: Rc::clone(&class.fields),
        })
    }

    /// Drop every class whose file the write log named since the last look.
    fn evict_written(&mut self) {
        let (seen, written) = written_since(self.seen);
        self.seen = seen;
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
        warn: impl FnOnce(String),
    ) -> Result<Class, String> {
        if !file.exists() {
            return Err(format!("Script file not found: {}", script_path));
        }
        self.reads += 1;
        let code =
            std::fs::read_to_string(file).map_err(|e| format!("Failed to read script: {}", e))?;
        let chunk = compile(lua, script_path, &code)
            .map_err(|e| format!("Syntax error compiling {}: {}", script_path, e));
        // A schema the inspection VM can't read leaves the fields unset: say so.
        let fields = match &chunk {
            Ok(_) => read_fields(&code).unwrap_or_else(|e| {
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

/// Compile `code` as `Chunk::eval` would run it: as an expression first (a file
/// that is a bare table), then as a block. Named after the script, so an error
/// reads `<path>:<line>:`.
fn compile(lua: &Lua, script_path: &str, code: &str) -> mlua::Result<Function> {
    let name = format!("@{script_path}");
    lua.load(format!("return {code}"))
        .set_name(name.as_str())
        .into_function()
        .or_else(|_| lua.load(code).set_name(name).into_function())
}
