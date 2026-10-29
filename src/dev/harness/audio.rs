//! src/dev/harness/audio.rs — `Harness.RecordAudio` / `Harness.StopAudio` (#1579).
//!
//! A scenario records the game's final mix to a WAV: the harness mixes it offline,
//! one fixed tick at a time (`audio::capture`), so two runs of one scenario write
//! byte-identical files and no sound card is needed. `play --record-audio <path>`
//! records the whole run the same way. A capture still running when the run ends is
//! closed with the results, so its file is always whole.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table};

use super::Harness;

/// The file a capture writes when the scenario names none, under the run's output.
pub const DEFAULT_FILE: &str = "audio.wav";

impl Harness {
    /// Start recording the final mix to `path`: relative to the run's output folder,
    /// [`DEFAULT_FILE`] there when `None`. Returns where it writes.
    pub fn record_audio(&mut self, path: Option<&Path>) -> Result<PathBuf, String> {
        let path = self.out_dir.join(path.unwrap_or(Path::new(DEFAULT_FILE)));
        let world = self.world.borrow();
        world.resources.audio.borrow_mut().start_capture(&path)?;
        drop(world);
        self.log(format!("Recording audio: {}", path.display()));
        Ok(path)
    }

    /// Stop the running capture and close its file, returning where it was written.
    pub fn stop_audio(&mut self) -> Result<PathBuf, String> {
        let world = self.world.borrow();
        let path = world.resources.audio.borrow_mut().stop_capture()?;
        drop(world);
        self.log(format!("Audio written: {}", path.display()));
        Ok(path)
    }

    /// Close a capture the scenario left running, noting it in the console.
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

/// Add `RecordAudio([path])` and `StopAudio()` to the `Harness` table; both return
/// the file's path and raise a Lua error on a refusal.
pub fn register(lua: &Lua, harness: &Rc<RefCell<Harness>>, t: &Table) -> LuaResult<()> {
    let h = Rc::clone(harness);
    t.set(
        "RecordAudio",
        lua.create_function(move |_, path: Option<String>| {
            let path = h.borrow_mut().record_audio(path.as_deref().map(Path::new));
            Ok(path
                .map_err(mlua::Error::RuntimeError)?
                .display()
                .to_string())
        })?,
    )?;
    let h = Rc::clone(harness);
    t.set(
        "StopAudio",
        lua.create_function(move |_, ()| {
            let path = h.borrow_mut().stop_audio();
            Ok(path
                .map_err(mlua::Error::RuntimeError)?
                .display()
                .to_string())
        })?,
    )
}
