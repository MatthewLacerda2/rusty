use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mlua::{Function, IntoLuaMulti, Lua, MultiValue, Table, Value};

use crate::scripting::console::ConsoleLogs;
use rusty_core::paths::{self, to_slash};
use rusty_core::project::SCRIPTS_DIR;
use rusty_edit::io::bundled;

struct Access {
    workspace: PathBuf,
    console: Rc<RefCell<ConsoleLogs>>,
    warned: RefCell<BTreeSet<PathBuf>>,
}

impl Access {
    fn resolve(&self, path: &str) -> PathBuf {
        self.workspace.join(path)
    }

    fn read(&self, path: &Path, how: &str) -> Option<String> {
        let code = std::fs::read_to_string(path).ok()?;
        self.check(path, how);
        Some(code)
    }

    fn check(&self, path: &Path, how: &str) {
        let Some(real) = paths::outside_root(path, &self.workspace) else {
            return;
        };
        let shown = paths::shown(&real);
        if self.warned.borrow_mut().insert(real) {
            self.console.borrow_mut().warn(format!(
                "[Lua] {how} read {shown}, outside the project: the game now depends on a \
                 file that may not exist on another machine, and the seed does not cover it"
            ));
        }
    }
}

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

fn found(lua: &Lua, code: &str, origin: &Path) -> mlua::Result<MultiValue> {
    let origin = to_slash(origin);
    let chunk = lua
        .load(code)
        .set_name(format!("@{origin}"))
        .into_function()?;
    (chunk, origin).into_lua_multi(lua)
}

fn project_searcher(lua: &Lua, access: Rc<Access>) -> mlua::Result<Function> {
    lua.create_function(move |lua, name: String| {
        // A `/` or `..` part would let `Path::join` leave the scripts folder; an error, not a
        // miss, so `package.path` is not tried either.
        let plain = |segment: &str| {
            let mut parts = Path::new(segment).components();
            matches!(parts.next(), Some(std::path::Component::Normal(_))) && parts.next().is_none()
        };
        if !name.split('.').all(plain) {
            return Err(mlua::Error::runtime(format!(
                "bad module name '{name}': each '.'-separated part must be a plain file name"
            )));
        }
        let rel = name.replace('.', "/");
        let dir = access.workspace.join(SCRIPTS_DIR);
        let mut tried = Vec::new();
        for file in [format!("{rel}.lua"), format!("{rel}/init.lua")] {
            let path = dir.join(&file);
            if let Some(code) = access.read(&path, &format!("require(\"{name}\")")) {
                return found(lua, &code, &path);
            }
            tried.push(format!("no file '{}'", to_slash(&path)));
        }
        tried.join("\n\t").into_lua_multi(lua)
    })
}

fn engine_searcher(lua: &Lua) -> mlua::Result<Function> {
    lua.create_function(|lua, name: String| {
        let rel = format!("{}.lua", name.replace('.', "/"));
        match bundled::script(&rel) {
            Some(code) => found(lua, code, Path::new(&format!("engine/scripts/{rel}"))),
            None => format!("no engine script 'engine/scripts/{rel}'").into_lua_multi(lua),
        }
    })
}

/// Calls `package.searchpath` through `package`, which `dev::demo::calls` must keep skipping,
/// or a scenario's fall-through `require` is journalled as a game act.
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
            Some((file, code)) => found(lua, &code, Path::new(file)),
            None => missing.unwrap_or_default().into_lua_multi(lua),
        }
    })
}

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
            args[0] = Value::String(lua.create_string(opened_path(&path))?);
        }
        stock.call::<MultiValue>(args)
    })?;
    lua.globals().set(name, wrapper)
}

/// Untouched: `\` is a filename character on macOS and Linux, so respelling it opens
/// another file.
fn opened_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
