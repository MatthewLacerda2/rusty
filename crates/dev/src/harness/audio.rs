use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table};

use super::Harness;
use rusty_core::paths::to_slash;

pub const DEFAULT_FILE: &str = "audio.wav";

impl Harness {
    pub fn record_audio(&mut self, path: Option<&Path>) -> Result<PathBuf, String> {
        let path = self.out_dir.join(path.unwrap_or(Path::new(DEFAULT_FILE)));
        let world = self.world.borrow();
        world.resources.audio.borrow_mut().start_capture(&path)?;
        drop(world);
        self.log(format!("Recording audio: {}", path.display()));
        Ok(path)
    }

    pub fn stop_audio(&mut self) -> Result<PathBuf, String> {
        let world = self.world.borrow();
        let path = world.resources.audio.borrow_mut().stop_capture()?;
        drop(world);
        self.log(format!("Audio written: {}", path.display()));
        Ok(path)
    }

    pub(super) fn finish_audio(&self) {
        let world = self.world.borrow();
        let mut audio = world.resources.audio.borrow_mut();
        if audio.capture_path().is_none() {
            return;
        }
        let mut console = self.console.borrow_mut();
        match audio.stop_capture() {
            Ok(path) => console.info(format!("[Harness] Audio written: {}", path.display())),
            Err(e) => console.error(e),
        }
    }
}

pub fn register(lua: &Lua, harness: &Rc<RefCell<Harness>>, t: &Table) -> LuaResult<()> {
    let h = Rc::clone(harness);
    t.set(
        "RecordAudio",
        lua.create_function(move |_, path: Option<String>| {
            let path = h.borrow_mut().record_audio(path.as_deref().map(Path::new));
            Ok(to_slash(&path.map_err(mlua::Error::RuntimeError)?))
        })?,
    )?;
    let h = Rc::clone(harness);
    t.set(
        "StopAudio",
        lua.create_function(move |_, ()| {
            let path = h.borrow_mut().stop_audio();
            Ok(to_slash(&path.map_err(mlua::Error::RuntimeError)?))
        })?,
    )
}
