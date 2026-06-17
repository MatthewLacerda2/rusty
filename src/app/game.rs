//! src/app/game.rs — GameWorld: the simulation, decoupled from window and GPU.
//!
//! Owns the world of record ([`World`] — a handle to the active `Scene`) and the
//! engine singletons ([`Resources`] — input, nav, console, camera, time, the live
//! rapier world, the script runtime, and the play-mode bookkeeping). It advances
//! both one frame via `tick(dt)`. The windowed front-end (main.rs) and the headless
//! harness drive the same `tick`; only the input source and the rendering differ.
//!
//! A system is the canonical two-argument `fn(&mut World, &mut Resources)` (#39):
//! `tick` threads `(&mut self.world, &mut self.resources)` through the schedule, so
//! the borrow checker keeps world-storage and engine-state distinct at the system
//! boundary. The handles inside `Resources` are still `Rc<RefCell<…>>` (the mlua
//! closures capture them); converting that script surface off `Rc<RefCell>` is the
//! follow-up the issue flags as its hardest part.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::{Scene, SceneSnapshot};
use crate::scripting::{ConsoleLogs, ScriptManager};
use crate::time::Time;

use super::{Resources, World};

/// Reported by `tick` so the platform layer can react (e.g. grab the cursor)
/// without the simulation knowing about the window.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlayTransition {
    None,
    Entered,
    Exited,
}

/// The simulation: the world of record plus its engine singletons. Splits the old
/// monolithic struct into [`World`] (storage) and [`Resources`] (engine state) so
/// the two are threaded into systems as distinct `&mut` references.
pub struct GameWorld {
    pub world: World,
    pub resources: Resources,
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
        let resources = Resources::new(
            Rc::clone(&scene),
            input,
            nav,
            console,
            Rc::clone(&camera),
            Rc::clone(&time),
        );
        Self {
            world: World::new(scene),
            resources,
        }
    }

    // --- Accessors: the shared engine-state handles. The editor, renderer and the
    //     headless dev tools borrow these cells directly, exactly as before. ---

    /// The active scene (the world of record).
    pub fn scene(&self) -> &Rc<RefCell<Scene>> {
        &self.world.scene
    }

    /// The input resource.
    pub fn input(&self) -> &Rc<RefCell<InputState>> {
        &self.resources.input
    }

    /// The navigation graph resource.
    pub fn nav(&self) -> &Rc<RefCell<NavigationGraph>> {
        &self.resources.nav
    }

    /// The console log buffer resource.
    pub fn console(&self) -> &Rc<RefCell<ConsoleLogs>> {
        &self.resources.console
    }

    /// The active camera resource.
    pub fn camera(&self) -> &Rc<RefCell<Camera>> {
        &self.resources.camera
    }

    /// The frame-clock resource.
    pub fn time(&self) -> &Rc<RefCell<Time>> {
        &self.resources.time
    }

    /// The live script runtime.
    pub fn script_manager(&self) -> &ScriptManager {
        &self.resources.script_manager
    }

    /// Whether the simulation is in play mode.
    pub fn is_playing(&self) -> bool {
        self.resources.is_playing
    }

    /// Enter or leave play mode (the platform layer flips this on a button / ESC).
    pub fn set_playing(&mut self, playing: bool) {
        self.resources.is_playing = playing;
    }

    /// Advance the simulation by `dt` seconds. Returns any play-state transition so
    /// the caller can handle platform-only concerns (cursor grab/visibility).
    pub fn tick(&mut self, dt: f32) -> PlayTransition {
        let transition = self.handle_transition();
        // `advance` records raw `dt` (unscaled) and the scaled `delta_time`.
        // The game sim integrates the scaled value; the editor camera uses raw.
        let scaled_dt = {
            let mut time = self.resources.time.borrow_mut();
            time.advance(dt);
            time.delta_time
        };
        if self.resources.is_playing {
            // Run the schedule's per-frame stages against (&mut World, &mut
            // Resources). The schedule is moved out of `Resources` for the call (its
            // systems take `&mut Resources`) and restored after, avoiding a
            // self-aliasing borrow. `Startup` runs once, in `enter_play`.
            self.resources.frame_dt = scaled_dt;
            self.run_schedule_frame();
        } else {
            self.editor_fly(dt);
        }
        self.sync_render_camera();
        transition
    }

    /// Run the schedule's per-frame stages. The schedule is moved out of `Resources`
    /// for the duration so its systems can take `&mut Resources`.
    fn run_schedule_frame(&mut self) {
        let schedule = std::mem::take(&mut self.resources.schedule);
        schedule.run_frame(&mut self.world, &mut self.resources);
        self.resources.schedule = schedule;
    }

    pub fn pathfinding_points(&self) -> &[Vec3] {
        &self.resources.pathfinding_points
    }

    /// Number of play-mode frames simulated since the last `enter_play`.
    pub fn play_frame(&self) -> u64 {
        self.resources.play_frame
    }

    fn handle_transition(&mut self) -> PlayTransition {
        let transition = if self.resources.is_playing && !self.resources.was_playing {
            self.enter_play();
            PlayTransition::Entered
        } else if !self.resources.is_playing && self.resources.was_playing {
            self.exit_play();
            PlayTransition::Exited
        } else {
            PlayTransition::None
        };
        self.resources.was_playing = self.resources.is_playing;
        transition
    }

    #[allow(clippy::too_many_lines)] // legacy; burn down in #124
    fn enter_play(&mut self) {
        self.resources.play_frame = 0;
        self.resources.time.borrow_mut().reset();
        // Snapshot the authoritative edit scene so Stop can restore it, discarding
        // every play-mode mutation (script/physics moves, health, spawns/despawns).
        self.resources.edit_snapshot = Some(SceneSnapshot::capture(&self.world.scene.borrow()));
        {
            let scene = self.world.scene.borrow();
            let player = scene
                .find_entity_by_name("Player")
                .and_then(|id| scene.get_entity(id));
            if let Some(player) = player {
                let mut cam = self.resources.camera.borrow_mut();
                cam.position = player.transform.position + Vec3::new(0.0, 1.5, -4.5);
                cam.yaw = 90.0;
                cam.pitch = -10.0;
            }
        }
        self.resources
            .console
            .borrow_mut()
            .info("Capturing cursor, entering PlayMode!".to_string());

        if let Err(err) = self
            .resources
            .script_manager
            .init_runtime(&self.resources.physics)
        {
            self.resources
                .console
                .borrow_mut()
                .error(format!("Lua init error: {}", err));
            return;
        }
        // Each entity can carry many scripts (#83); load each, keyed by its slot.
        let mut to_load: Vec<(u32, usize, String)> = Vec::new();
        for e in self.world.scene.borrow().iter() {
            let rows = e.scripts.iter().enumerate();
            to_load.extend(rows.map(|(i, s)| (e.id, i, s.path.clone())));
        }
        for (id, index, path) in to_load {
            if let Err(e) = self
                .resources
                .script_manager
                .load_entity_script(id, index, &path)
            {
                let msg = format!("Lua compile error (Entity {}): {}", id, e);
                self.resources.console.borrow_mut().error(msg);
            }
        }
        self.resources.script_manager.start_scripts();

        // Build the rapier world from the (post-start) scene: bodies + colliders
        // for every entity with a ColliderComponent. Stepped each frame in play.
        // Shared with the script runtime's Physics.Raycast/Shoot bindings.
        *self.resources.physics.borrow_mut() =
            Some(PhysicsWorld::from_scene(&self.world.scene.borrow()));
        // Run the one-shot `Startup` stage now that the Play session is fully set
        // up. No built-in module registers Startup systems yet; this is the wired
        // hook modules will register into (Unity's `Start`).
        self.resources.frame_dt = 0.0;
        let schedule = std::mem::take(&mut self.resources.schedule);
        schedule.run_startup(&mut self.world, &mut self.resources);
        self.resources.schedule = schedule;
    }

    fn exit_play(&mut self) {
        // Stop is a storage boundary: persist any play-mode writes (PlayerPrefs-style
        // save data survives even though the scene snapshot is discarded below).
        self.resources.flush_storage();
        self.resources.script_manager.shutdown();
        self.resources.pathfinding_points.clear();
        self.resources.play_frame = 0;
        *self.resources.physics.borrow_mut() = None;
        // Restore the edit scene captured on Play, discarding play-mode state.
        if let Some(snapshot) = self.resources.edit_snapshot.take() {
            snapshot.restore(&mut self.world.scene.borrow_mut());
            self.resources
                .nav
                .borrow_mut()
                .bake(&self.world.scene.borrow());
        }
    }

    /// Editor-mode free-fly camera (no entity simulation).
    fn editor_fly(&mut self, dt: f32) {
        let inp = self.resources.input.borrow();
        let mut cam = self.resources.camera.borrow_mut();
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
