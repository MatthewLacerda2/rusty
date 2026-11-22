use std::cell::RefCell;
use std::rc::Rc;

use rusty_scene::Scene;

/// Shared with the Lua runtime, whose closures capture the cell: systems borrow it
/// transiently, never across a re-entrant script call.
pub struct World {
    pub scene: Rc<RefCell<Scene>>,
}

impl World {
    pub fn new(scene: Rc<RefCell<Scene>>) -> Self {
        Self { scene }
    }
}

impl super::GameWorld {
    /// What runs in place of the schedule on every frame outside Play.
    pub fn edit_frame(&mut self) {
        rusty_asset::convert::convert_held();
        self.refresh_ui_textures();
        let mut scene = self.world.scene.borrow_mut();
        self.resources.model_writes.follow(&mut scene);
        scene.build_skin_palettes();
    }

    /// One paused frame: only queued `Time.Step(n)` steps advance the world, each one
    /// fixed tick. With none queued the sim stays frozen but Play/Stop still takes
    /// effect. Returns the first tick's transition.
    pub fn paused_frame(&mut self) -> super::PlayTransition {
        use super::PlayTransition;
        let mut transition = PlayTransition::None;
        let mut stepped = false;
        while self.time().borrow_mut().take_pending_step() {
            let t = self.tick(rusty_time::FIXED_DELTA_TIME);
            if t != PlayTransition::None {
                transition = t;
            }
            stepped = true;
        }
        if !stepped {
            transition = self.poll_transition();
            // A pause outlives Stop: the edit frame runs on the scene the transition left.
            if !self.play_requested() {
                self.edit_frame();
            }
            self.push_audio(0.0); // as a tick would push it, with no time passed
        }
        transition
    }
}
