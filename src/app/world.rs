//! src/app/world.rs — World: the storage of record handed to every system.
//!
//! Issue #39 makes a system the canonical two-argument `fn(&mut World, &mut
//! Resources)`. `World` is the world half: the active `Scene` (the hecs-backed
//! storage of record, #40). `Resources` is the engine-singleton half. The two are
//! distinct types threaded side by side, so the borrow checker keeps world-storage
//! and engine-state from aliasing at the system boundary — no more one opaque
//! `GameWorld` blob the systems reach through.
//!
//! The `Scene` is still held behind an `Rc<RefCell<…>>`: the mlua script closures
//! capture that exact cell with a `'static` lifetime (they call `Scene.SetPosition`
//! &c. mid-tick), and the editor / renderer share it too. A system therefore reads
//! the scene through a short-lived `borrow()`/`borrow_mut()` — exactly as before —
//! rather than holding a long `&mut Scene` that would deadlock a re-entrant script
//! call. Lifting the script surface fully off `Rc<RefCell>` is the follow-up the
//! issue flags as its hardest part.

use std::cell::RefCell;
use std::rc::Rc;

use crate::scene::Scene;

/// The world of record threaded into every system as `&mut World`. A thin handle to
/// the active `Scene`; the `Rc<RefCell<…>>` is shared with the Lua runtime and the
/// editor, so systems borrow it transiently rather than holding it open across a
/// re-entrant script call.
pub struct World {
    pub scene: Rc<RefCell<Scene>>,
}

impl World {
    /// Wrap a shared scene handle as the world of record.
    pub fn new(scene: Rc<RefCell<Scene>>) -> Self {
        Self { scene }
    }
}

impl super::GameWorld {
    /// The edit frame: what runs in place of the schedule on every frame outside
    /// Play, stepped or frozen ([`paused_frame`](Self::paused_frame) calls it on a
    /// frozen one).
    /// Every edit-mode follower hangs off it: a model Play found changed converts
    /// now (#1253), a model the engine re-wrote reloads into its entities (#1242),
    /// a bone the designer moved still skins (#453), and the UI's texture sizes are
    /// read (#1241).
    pub fn edit_frame(&mut self) {
        crate::asset::convert::convert_held();
        self.refresh_ui_textures();
        let mut scene = self.world.scene.borrow_mut();
        self.resources.model_writes.follow(&mut scene);
        scene.build_skin_palettes();
    }

    /// One paused frame (#283): the wall clock is bypassed and the world moves only
    /// by the queued `Time.Step(n)` steps, each one
    /// [`FIXED_DELTA_TIME`](crate::time::FIXED_DELTA_TIME) tick, the fixed-dt the
    /// headless harness uses, so windowed and headless stepping are frame-identical.
    /// Every queued step drains now. With none queued the sim stays frozen but a
    /// Play/Stop still takes effect, outside Play the edit frame runs on the scene
    /// the transition left (a pause can outlive Stop: `Time::reset` keeps it), and
    /// the audio mix is pushed with no time, as a tick would have pushed it (#1579).
    /// The windowed loop runs it while paused; the headless session after every
    /// command (#1360). Returns the first tick's transition.
    pub fn paused_frame(&mut self) -> super::PlayTransition {
        use super::PlayTransition;
        let mut transition = PlayTransition::None;
        let mut stepped = false;
        while self.time().borrow_mut().take_pending_step() {
            let t = self.tick(crate::time::FIXED_DELTA_TIME);
            if t != PlayTransition::None {
                transition = t;
            }
            stepped = true;
        }
        if !stepped {
            transition = self.poll_transition();
            if !self.play_requested() {
                self.edit_frame();
            }
            self.push_audio(0.0);
        }
        transition
    }
}
