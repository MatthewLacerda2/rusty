//! Seeking a replay (#1597): `Harness.SeekDemo(tick)` fast-forwards to a found
//! moment, the same steps a straight replay takes (the recorded input, the
//! recorded `dt`, the digests checked on the way), and draws nothing.
//!
//! Seeking back restarts the replay from the demo's start state: Play is left,
//! and the world is put back as [`prepare`](super::prepare) found it (its scene
//! by [`Origin`], which the editor's close shares, then the camera, the clock and
//! the Game view size), then prepared again from the file, read before anything
//! is torn down.
//! It is the same world, not a new one: a scenario's API is bound to the world's
//! own cells for the whole run, so a fresh world would leave it reading the old
//! one.

use std::path::{Path, PathBuf};

use super::super::read::Demo;
use super::{install, Origin, Replayer};
use crate::app::GameWorld;
use crate::dev::harness::tick_unless_quit;
use crate::scene::Camera;
use crate::time::Time;

/// What the world was when the replay was prepared: where a seek back restarts.
pub struct Start {
    path: PathBuf,
    scene: Origin,
    scene_path: Option<String>,
    camera: Camera,
    time: Time,
    game_view: Option<(u32, u32)>,
}

impl Start {
    /// `world` as it stands, right before the demo at `path` is prepared on it.
    pub(super) fn of(world: &GameWorld, path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            scene: Origin::of(world),
            scene_path: world.script_manager().scene_path_cell().borrow().clone(),
            camera: world.camera().borrow().clone(),
            time: world.time().borrow().clone(),
            game_view: world.resources.screen.borrow().game_view(),
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    /// Put `world` back as it was, Play left first.
    fn restore(self, world: &mut GameWorld) {
        self.scene.restore(world);
        *world.script_manager().scene_path_cell().borrow_mut() = self.scene_path;
        *world.camera().borrow_mut() = self.camera;
        *world.time().borrow_mut() = self.time;
        // A tick past the start may have resized the Game view.
        (world.resources.screen.borrow_mut()).restore_game_view(self.game_view);
        world.set_playing(true);
    }
}

pub const NO_DEMO: &str = "this run replays no demo: run the scenario with `play --demo <file>`";

/// Replay `world`'s demo up to and including tick `tick`: forward from where it
/// stands, or from its start when `tick` has already run (or the world ran on
/// past the demo's end).
pub fn seek(world: &mut GameWorld, tick: u64) -> Result<(), String> {
    let replayer = world.tap_mut::<Replayer>().ok_or(NO_DEMO)?;
    let (done, of) = (replayer.verdict().ticks, replayer.verdict().of);
    if tick >= of {
        return Err(format!(
            "tick {tick} is past the demo's end: it holds {of} ticks"
        ));
    }
    if done > tick + 1 || replayer.past_end() {
        restart(world)?;
    }
    loop {
        let done = world.tap_mut::<Replayer>().map_or(0, |r| r.verdict().ticks);
        if done > tick {
            return Ok(());
        }
        if !tick_unless_quit(world) {
            return Err(format!(
                "the game quit after tick {done}, before tick {tick}"
            ));
        }
    }
}

/// Start the replay over: the world as it was prepared, prepared again. What the
/// verdict found stays found: a divergence, and that a scenario steps it. What
/// can fail (the file read again, the replay's VM) is done first, so a failed
/// restart leaves the run as it stood.
fn restart(world: &mut GameWorld) -> Result<(), String> {
    let replayer = world.tap_mut::<Replayer>().ok_or(NO_DEMO)?;
    let demo = Demo::read(replayer.demo_path())?;
    let lua = crate::scripting::sandboxed_vm()?;
    let tap = world.take_tap().ok_or(NO_DEMO)?;
    let any: Box<dyn std::any::Any> = tap;
    let old = any.downcast::<Replayer>().map_err(|_| NO_DEMO)?;
    let (start, verdict) = old.into_start();
    start.restore(world);
    install(world, demo, lua);
    let replayer = world.tap_mut::<Replayer>().ok_or(NO_DEMO)?;
    replayer.carry(verdict);
    Ok(())
}
