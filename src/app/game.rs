//! src/app/game.rs — GameWorld: the simulation, decoupled from window and GPU.
//!
//! Owns the scene, input, navigation, console, script runtime and camera, and
//! advances them one frame via `tick(dt)`. The windowed front-end (main.rs) and a
//! future headless harness both drive the same `tick`; only the input source and
//! the rendering differ. Storage is still the legacy `Scene` (Phase 0); the hecs
//! migration is Phase 1.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::core::input::InputState;
use crate::core::scene::Scene;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::SceneSnapshot;
use crate::scripting::{ConsoleLogs, ScriptManager};
use crate::time::Time;

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
    pub camera: Rc<RefCell<Camera>>,
    pub time: Rc<RefCell<Time>>,
    pub is_playing: bool,
    pub(super) was_playing: bool,
    pub(super) pathfinding_points: Vec<Vec3>,
    /// Play-mode frame counter. Drives nav rebaking off a deterministic tick count
    /// instead of the wall clock, so a fixed-timestep replay is bit-for-bit stable.
    pub(super) play_frame: u64,
    /// Edit-mode scene captured on Play and restored on Stop, so play-mode
    /// mutations never leak back into the authoritative edit scene (Unity-style).
    pub(super) edit_snapshot: Option<SceneSnapshot>,
    /// rapier3d simulation, rebuilt from the scene on entering Play and torn down
    /// on Stop. `None` in edit mode (no body/collision sim runs there). Shared
    /// (via `Rc`) with the script runtime so `Physics.Raycast`/`Shoot` cast
    /// against the very same world the engine hitscan does (#31).
    pub(super) physics: Rc<RefCell<Option<PhysicsWorld>>>,
}

impl GameWorld {
    pub fn new(
        scene: Rc<RefCell<Scene>>,
        input: Rc<RefCell<InputState>>,
        nav: Rc<RefCell<NavigationGraph>>,
        console: Rc<RefCell<ConsoleLogs>>,
    ) -> Self {
        let camera = Rc::new(RefCell::new(Camera::new(
            Vec3::new(0.0, 5.0, -10.0),
            90.0,
            -20.0,
        )));
        let time = Rc::new(RefCell::new(Time::new()));
        let script_manager = ScriptManager::new(
            Rc::clone(&scene),
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
            Rc::clone(&camera),
            Rc::clone(&time),
        );
        Self {
            scene,
            input,
            nav,
            console,
            script_manager,
            camera,
            time,
            is_playing: false,
            was_playing: false,
            pathfinding_points: Vec::new(),
            play_frame: 0,
            edit_snapshot: None,
            physics: Rc::new(RefCell::new(None)),
        }
    }

    /// Advance the simulation by `dt` seconds. Returns any play-state transition so
    /// the caller can handle platform-only concerns (cursor grab/visibility).
    pub fn tick(&mut self, dt: f32) -> PlayTransition {
        let transition = self.handle_transition();
        // `advance` records raw `dt` (unscaled) and the scaled `delta_time`.
        // The game sim integrates the scaled value; the editor camera uses raw.
        let scaled_dt = {
            let mut time = self.time.borrow_mut();
            time.advance(dt);
            time.delta_time
        };
        if self.is_playing {
            super::play::run(self, scaled_dt);
        } else {
            self.editor_fly(dt);
        }
        transition
    }

    pub fn pathfinding_points(&self) -> &[Vec3] {
        &self.pathfinding_points
    }

    /// Number of play-mode frames simulated since the last `enter_play`.
    pub fn play_frame(&self) -> u64 {
        self.play_frame
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
        self.play_frame = 0;
        self.time.borrow_mut().reset();
        // Snapshot the authoritative edit scene so Stop can restore it, discarding
        // every play-mode mutation (script/physics moves, health, spawns/despawns).
        self.edit_snapshot = Some(SceneSnapshot::capture(&self.scene.borrow()));
        {
            let scene = self.scene.borrow();
            let player = scene
                .find_entity_by_name("Player")
                .and_then(|id| scene.get_entity(id));
            if let Some(player) = player {
                let mut cam = self.camera.borrow_mut();
                cam.position = player.transform.position + Vec3::new(0.0, 1.5, -4.5);
                cam.yaw = 90.0;
                cam.pitch = -10.0;
            }
        }
        self.console
            .borrow_mut()
            .info("Capturing cursor, entering PlayMode!".to_string());

        if let Err(err) = self.script_manager.init_runtime(&self.physics) {
            self.console
                .borrow_mut()
                .error(format!("Lua init error: {}", err));
            return;
        }
        let to_load: Vec<(u32, String)> = self
            .scene
            .borrow()
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

        // Build the rapier world from the (post-start) scene: bodies + colliders
        // for every entity with a ColliderComponent. Stepped each frame in play.
        // Shared with the script runtime's Physics.Raycast/Shoot bindings.
        *self.physics.borrow_mut() = Some(PhysicsWorld::from_scene(&self.scene.borrow()));
    }

    fn exit_play(&mut self) {
        self.script_manager.shutdown();
        self.pathfinding_points.clear();
        self.play_frame = 0;
        *self.physics.borrow_mut() = None;
        // Restore the edit scene captured on Play, discarding play-mode state.
        if let Some(snapshot) = self.edit_snapshot.take() {
            snapshot.restore(&mut self.scene.borrow_mut());
            self.nav.borrow_mut().bake(&self.scene.borrow());
        }
    }

    /// Editor-mode free-fly camera (no entity simulation).
    fn editor_fly(&mut self, dt: f32) {
        let inp = self.input.borrow();
        let mut cam = self.camera.borrow_mut();
        let mut move_dir = Vec3::ZERO;
        if inp.is_key_down("W") {
            move_dir += cam.forward();
        }
        if inp.is_key_down("S") {
            move_dir -= cam.forward();
        }
        if inp.is_key_down("A") {
            move_dir -= cam.right();
        }
        if inp.is_key_down("D") {
            move_dir += cam.right();
        }
        if move_dir.length_squared() > 0.001 {
            cam.position += move_dir.normalize() * 10.0 * dt;
        }

        let look = 90.0 * dt;
        if inp.is_key_down("LEFT") {
            cam.yaw -= look;
        }
        if inp.is_key_down("RIGHT") {
            cam.yaw += look;
        }
        if inp.is_key_down("UP") {
            cam.pitch += look;
        }
        if inp.is_key_down("DOWN") {
            cam.pitch -= look;
        }
        cam.pitch = cam.pitch.clamp(-80.0, 80.0);
    }
}
