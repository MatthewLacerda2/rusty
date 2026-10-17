//! What the gameplay VM may read from disk (#1062): `require`, `dofile` and
//! `loadfile`, resolved against the project instead of the process's working
//! directory.
//!
//! `require("ui.range_axis")` searches, in order:
//! 1. `package.preload`, Lua's own first searcher;
//! 2. the project's scripts folder: `<workspace>/assets/scripts/ui/range_axis.lua`,
//!    then `…/ui/range_axis/init.lua`;
//! 3. the engine's bundled scripts (`scene::io::bundled`), so a widget's helper is
//!    found even where the project holds no copy;
//! 4. `package.path`, Lua's stock search, which a game may extend.
//!
//! A relative `dofile` / `loadfile` path is taken from the workspace root.
//!
//! Reading outside the project stays **allowed**: that is the game developer's call
//! (operator, 2026-10-17). But such a file is an input the seed doesn't cover and
//! may be missing on another machine, so the first read of each outside path logs a
//! console warning. Native (C) modules stay off.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mlua::{Function, IntoLuaMulti, Lua, MultiValue, Table, Value};

use crate::core::project::SCRIPTS_DIR;
use crate::scene::io::bundled;
use crate::scripting::console::ConsoleLogs;

/// The project root the VM reads against, and the outside paths it already warned
/// about.
struct Access {
    workspace: PathBuf,
    console: Rc<RefCell<ConsoleLogs>>,
    warned: RefCell<BTreeSet<PathBuf>>,
}

impl Access {
    /// `path` taken from the workspace root (`path` itself when absolute).
    fn resolve(&self, path: &str) -> PathBuf {
        self.workspace.join(path)
    }

    /// The source at `path`, read for `how` (`require("x")`, `dofile`); warns once
    /// when the file lies outside the project.
    fn read(&self, path: &Path, how: &str) -> Option<String> {
        let code = std::fs::read_to_string(path).ok()?;
        self.check(path, how);
        Some(code)
    }

    /// Warn, once per file, when the existing file at `path` is outside the project.
    fn check(&self, path: &Path, how: &str) {
        let real = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let root = if self.workspace.as_os_str().is_empty() {
            Path::new(".")
        } else {
            &self.workspace
        };
        let inside = root.canonicalize().is_ok_and(|root| real.starts_with(root));
        if !inside && self.warned.borrow_mut().insert(real.clone()) {
            self.console.borrow_mut().warn(format!(
                "[Lua] {how} read {}, outside the project: the game now depends on a file \
                 that may not exist on another machine, and the seed does not cover it",
                real.display()
            ));
        }
    }
}

/// Install the project-aware `require` searchers and the `dofile` / `loadfile`
/// wrappers into `lua`, reading against `workspace`. Safe to call again when the
/// workspace changes: the stock functions are kept in the registry and rewrapped.
pub(in crate::scripting) fn install(
    lua: &Lua,
    workspace: &Path,
    console: &Rc<RefCell<ConsoleLogs>>,
) -> Result<(), String> {
    let access = Rc::new(Access {
        workspace: workspace.to_path_buf(),
        console: Rc::clone(console),
        warned: RefCell::new(BTreeSet::new()),
    });
    install_searchers(lua, &access)
        .and_then(|()| wrap_file_reader(lua, &access, "dofile"))
        .and_then(|()| wrap_file_reader(lua, &access, "loadfile"))
        .map_err(|e| e.to_string())
}

fn install_searchers(lua: &Lua, access: &Rc<Access>) -> mlua::Result<()> {
    let package: Table = lua.globals().get("package")?;
    let stock: Table = package.get("searchers")?;
    let preload: Value = match lua.named_registry_value("rusty.preload_searcher")? {
        Value::Nil => stock.get(1)?,
        kept => kept,
    };
    lua.set_named_registry_value("rusty.preload_searcher", &preload)?;
    let searchers = lua.create_sequence_from([
        preload,
        Value::Function(project_searcher(lua, Rc::clone(access))?),
        Value::Function(engine_searcher(lua)?),
        Value::Function(path_searcher(lua, Rc::clone(access))?),
    ])?;
    package.set("searchers", searchers)?;
    package.set("cpath", "")
}

/// `require`'s answer for a module found at `origin`: its chunk, and where it was.
fn found(lua: &Lua, code: &str, origin: &str) -> mlua::Result<MultiValue> {
    let chunk = lua
        .load(code)
        .set_name(format!("@{origin}"))
        .into_function()?;
    (chunk, origin.to_string()).into_lua_multi(lua)
}

fn project_searcher(lua: &Lua, access: Rc<Access>) -> mlua::Result<Function> {
    lua.create_function(move |lua, name: String| {
        let rel = name.replace('.', "/");
        let dir = access.workspace.join(SCRIPTS_DIR);
        let mut tried = Vec::new();
        for file in [format!("{rel}.lua"), format!("{rel}/init.lua")] {
            let path = dir.join(file);
            if let Some(code) = access.read(&path, &format!("require(\"{name}\")")) {
                return found(lua, &code, &path.to_string_lossy());
            }
            tried.push(format!("no file '{}'", path.display()));
        }
        tried.join("\n\t").into_lua_multi(lua)
    })
}

fn engine_searcher(lua: &Lua) -> mlua::Result<Function> {
    lua.create_function(|lua, name: String| {
        let rel = format!("{}.lua", name.replace('.', "/"));
        match bundled::script(&rel) {
            Some(code) => found(lua, code, &format!("engine/scripts/{rel}")),
            None => format!("no engine script 'engine/scripts/{rel}'").into_lua_multi(lua),
        }
    })
}

/// Lua's stock search over `package.path`, read at call time so a game's own
/// additions apply. Whatever it finds outside the project warns.
fn path_searcher(lua: &Lua, access: Rc<Access>) -> mlua::Result<Function> {
    lua.create_function(move |lua, name: String| {
        let package: Table = lua.globals().get("package")?;
        let searchpath: Function = package.get("searchpath")?;
        let path: String = package.get("path")?;
        let (file, missing): (Option<String>, Option<String>) =
            searchpath.call((name.as_str(), path))?;
        let read = file.as_deref().and_then(|file| {
            let code = access.read(Path::new(file), &format!("require(\"{name}\")"))?;
            Some((file, code))
        });
        match read {
            Some((file, code)) => found(lua, &code, file),
            None => missing.unwrap_or_default().into_lua_multi(lua),
        }
    })
}

/// Replace the global `name` (`dofile` / `loadfile`) with one that takes a relative
/// path from the workspace root and warns on an outside file, then calls the stock
/// function. No path (read stdin) is passed through untouched.
fn wrap_file_reader(lua: &Lua, access: &Rc<Access>, name: &str) -> mlua::Result<()> {
    let key = format!("rusty.stock_{name}");
    let stock: Function = match lua.named_registry_value::<Option<Function>>(&key)? {
        Some(stock) => stock,
        None => {
            let stock: Function = lua.globals().get(name)?;
            lua.set_named_registry_value(&key, &stock)?;
            stock
        }
    };
    let access = Rc::clone(access);
    let how = name.to_string();
    let wrapper = lua.create_function(move |lua, mut args: MultiValue| {
        if let Some(Value::String(file)) = args.front() {
            let path = access.resolve(&file.to_str()?);
            if path.is_file() {
                access.check(&path, &how);
            }
            args[0] = Value::String(lua.create_string(path.to_string_lossy().as_ref())?);
        }
        stock.call::<MultiValue>(args)
    })?;
    lua.globals().set(name, wrapper)
}
