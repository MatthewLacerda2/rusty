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

    /// Bring the Lua runtime live **in edit mode**, without entering Play.
    ///
    /// Play's `enter_play` initialises the runtime as a side effect of starting
    /// the simulation (snapshot, physics build, lifecycle `start`). A headless
    /// edit-mode session needs the *evaluator* live against the authoritative edit
    /// scene without any of that: no snapshot is taken, no entity scripts are
    /// loaded, and `physics` stays `None` (edit mode has no rapier world). The
    /// runtime binds the same shared `scene`/`input`/`nav`/… cells, so every
    /// `api::` namespace resolves against the live world and mutations stick.
    ///
    /// Idempotent: a no-op once the runtime is live (re-initialising would drop
    /// any session state held in the VM). Errors surface as the Lua init message.
    pub fn init_edit_runtime(&mut self) -> Result<(), String> {
        if self.resources.script_manager.is_live() {
            return Ok(());
        }
        self.resources
            .script_manager
            .init_runtime(&self.resources.physics)
    }

    /// Whether the simulation is in play mode.
    pub fn is_playing(&self) -> bool {
        self.resources.is_playing
    }

    /// Enter or leave play mode (the platform layer flips this on a button / ESC).
    pub fn set_playing(&mut self, playing: bool) {
        self.resources.is_playing = playing;
        // Mirror into the script runtime's shared cell so `Debug.Snapshot` reports
        // the live play-state (the evaluator doesn't hold `Resources`).
        *self.resources.script_manager.play_state_cell().borrow_mut() = playing;
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

    /// Process only the play-state transition (enter/exit play) **without advancing
    /// the sim** (issue #283). The windowed "playing-but-stepped" loop calls this on a
    /// paused, no-step frame: the world is frozen, but a Play/Stop pressed while paused
    /// must still take effect (Stop → `exit_play` restores the edit snapshot). Returns
    /// the transition so the platform layer can grab/release the cursor exactly as a
    /// normal `tick` would. No clock advance, no schedule run, no editor fly.
    pub fn poll_transition(&mut self) -> PlayTransition {
        self.handle_transition()
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

    fn enter_play(&mut self) {
        self.resources.play_frame = 0;
        self.resources.time.borrow_mut().reset();
        // Start the audio log fresh for this Play session (#212) and silence any
        // voice that lingered from a prior run.
        {
            let mut audio = self.resources.audio.borrow_mut();
            audio.stop_all();
            audio.clear_log();
        }
        // Snapshot the edit scene so Stop can restore it, discarding play-mode mutations.
        self.resources.edit_snapshot = Some(SceneSnapshot::capture(&self.world.scene.borrow()));
        self.snap_camera_to_player();
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
        // Load every scene entity's scripts, then run the two-phase init (#322):
        // ALL `Awake`s, then ALL `Start`s, actives only — a disabled-at-load
        // entity defers both until its first active tick.
        self.resources.script_manager.init_scripts();

        // Build the rapier world from the (post-start) scene: bodies + colliders
        // for every entity with a ColliderComponent. Stepped each frame in play.
        // Shared with the script runtime's Physics.Raycast binding.
        *self.resources.physics.borrow_mut() =
            Some(PhysicsWorld::from_scene(&self.world.scene.borrow()));

        self.run_startup_stage();
    }

    fn exit_play(&mut self) {
        // Stop is a storage boundary: persist any play-mode writes (PlayerPrefs-style
        // save data survives even though the scene snapshot is discarded below).
        self.resources.flush_storage();
        self.resources.script_manager.shutdown();
        self.resources.pathfinding_points.clear();
        self.resources.play_frame = 0;
        *self.resources.physics.borrow_mut() = None;
        // Silence every voice on Stop (Unity stops play-mode audio at exit).
        self.resources.audio.borrow_mut().stop_all();
        // Restore the edit scene captured on Play, discarding play-mode state.
        if let Some(snapshot) = self.resources.edit_snapshot.take() {
            snapshot.restore(&mut self.world.scene.borrow_mut());
            self.resources
                .nav
                .borrow_mut()
                .bake(&self.world.scene.borrow());
        }
    }
}

#[cfg(test)]
#[path = "game_tests.rs"]
mod game_tests;

#[cfg(test)]
#[path = "game_play_tests.rs"]
mod game_play_tests;
