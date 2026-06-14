//! src/app/game.rs — GameWorld: the simulation, decoupled from window and GPU.
//!
//! Owns the scene, input, navigation, console, script runtime and camera, and
//! advances them one frame via `tick(dt)`. The windowed front-end (main.rs) and a
//! future headless harness both drive the same `tick`; only the input source and
//! the rendering differ. Storage is still the legacy `Scene` (Phase 0); the hecs
//! migration is Phase 1.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use glam::Vec3;

use crate::core::input::InputState;
use crate::core::scene::Scene;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
use crate::scripting::{ConsoleLogs, ScriptManager};

/// Reported by `tick` so the platform layer can react (e.g. grab the cursor)
/// without the simulation knowing about the window.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlayTransition {
    None,
    Entered,
    Exited,
}

pub struct GameWorld {
    pub scene: Rc<RefCell<Scene>>,
    pub input: Rc<RefCell<InputState>>,
    pub nav: Rc<RefCell<NavigationGraph>>,
    pub console: Rc<RefCell<ConsoleLogs>>,
    pub script_manager: ScriptManager,
    pub camera: Camera,
    pub is_playing: bool,
    pub(super) was_playing: bool,
    pub(super) pathfinding_points: Vec<Vec3>,
    pub(super) last_path_bake: Instant,
}

impl GameWorld {
    pub fn new(
        scene: Rc<RefCell<Scene>>,
        input: Rc<RefCell<InputState>>,
        nav: Rc<RefCell<NavigationGraph>>,
        console: Rc<RefCell<ConsoleLogs>>,
    ) -> Self {
        let script_manager = ScriptManager::new(
            Rc::clone(&scene),
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
        );
        Self {
            scene,
            input,
            nav,
            console,
            script_manager,
            camera: Camera::new(Vec3::new(0.0, 5.0, -10.0), 90.0, -20.0),
            is_playing: false,
            was_playing: false,
            pathfinding_points: Vec::new(),
            last_path_bake: Instant::now(),
        }
    }

    /// Advance the simulation by `dt` seconds. Returns any play-state transition so
    /// the caller can handle platform-only concerns (cursor grab/visibility).
    pub fn tick(&mut self, dt: f32) -> PlayTransition {
        let transition = self.handle_transition();
        if self.is_playing {
            super::play::run(self, dt);
        } else {
            self.editor_fly(dt);
        }
        transition
    }

    pub fn pathfinding_points(&self) -> &[Vec3] {
        &self.pathfinding_points
    }

    fn handle_transition(&mut self) -> PlayTransition {
        let transition = if self.is_playing && !self.was_playing {
            self.enter_play();
            PlayTransition::Entered
        } else if !self.is_playing && self.was_playing {
            self.exit_play();
            PlayTransition::Exited
        } else {
            PlayTransition::None
        };
        self.was_playing = self.is_playing;
        transition
    }

    fn enter_play(&mut self) {
        if let Some(player) = self.scene.borrow().get_entity(2) {
            self.camera.position = player.transform.position + Vec3::new(0.0, 1.5, -4.5);
            self.camera.yaw = 90.0;
            self.camera.pitch = -10.0;
        }
        self.console
            .borrow_mut()
            .info("Capturing cursor, entering PlayMode!".to_string());

        if let Err(err) = self.script_manager.init_runtime() {
            self.console
                .borrow_mut()
                .error(format!("Lua init error: {}", err));
            return;
        }
        let to_load: Vec<(u32, String)> = self
            .scene
            .borrow()
            .entities
            .iter()
            .filter_map(|e| e.script.as_ref().map(|s| (e.id, s.path.clone())))
            .collect();
        for (id, path) in to_load {
            if let Err(e) = self.script_manager.load_entity_script(id, &path) {
                self.console
                    .borrow_mut()
                    .error(format!("Lua compile error (Entity {}): {}", id, e));
            }
        }
        self.script_manager.start_scripts();
    }

    fn exit_play(&mut self) {
        self.script_manager.shutdown();
        self.pathfinding_points.clear();
    }

    /// Editor-mode free-fly camera (no entity simulation).
    fn editor_fly(&mut self, dt: f32) {
        let inp = self.input.borrow();
        let mut move_dir = Vec3::ZERO;
        if inp.is_key_down("W") {
            move_dir += self.camera.forward();
        }
        if inp.is_key_down("S") {
            move_dir -= self.camera.forward();
        }
        if inp.is_key_down("A") {
            move_dir -= self.camera.right();
        }
        if inp.is_key_down("D") {
            move_dir += self.camera.right();
        }
        if move_dir.length_squared() > 0.001 {
            self.camera.position += move_dir.normalize() * 10.0 * dt;
        }

        let look = 90.0 * dt;
        if inp.is_key_down("LEFT") {
            self.camera.yaw -= look;
        }
        if inp.is_key_down("RIGHT") {
            self.camera.yaw += look;
        }
        if inp.is_key_down("UP") {
            self.camera.pitch += look;
        }
        if inp.is_key_down("DOWN") {
            self.camera.pitch -= look;
        }
        self.camera.pitch = self.camera.pitch.clamp(-80.0, 80.0);
    }
}
