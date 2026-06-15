//! src/app/game.rs — GameWorld: the simulation, decoupled from window and GPU.
//!
//! Owns the world of record ([`World`] — the active `Scene`) and the engine
//! singletons ([`Resources`] — input, nav, console, camera, time, the live rapier
//! world, the script runtime, and the play-mode bookkeeping). It advances both one
//! frame via `tick(dt)`. The windowed front-end (main.rs) and the headless harness
//! drive the same `tick`; only the input source and the rendering differ.
//!
//! A system is the canonical two-argument `fn(&mut World, &mut Resources)` (#39):
//! `tick` threads `(&mut self.world, &mut self.resources)` through the schedule, so
//! the borrow checker keeps world-storage and engine-state distinct at the system
//! boundary. Both halves own PLAIN data — no `Rc`, no `RefCell` (#57). The script
//! runtime no longer captures engine state with a `'static` lifetime: a script run
//! opens a `lua.scope` over the borrowed sim data ([`ScriptCtx`]), so the bindings
//! reach the very same owned values the systems hold.

use glam::Vec3;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
use crate::scene::Scene;
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
    /// Build the simulation from owned engine state. The world of record (the
    /// `Scene`) and every engine singleton are owned outright (#57).
    pub fn new(
        scene: Scene,
        input: InputState,
        nav: NavigationGraph,
        console: ConsoleLogs,
    ) -> Self {
        let resources = Resources::new(input, nav, console);
        Self {
            world: World::new(scene),
            resources,
        }
    }

    // --- Accessors: the owned engine state. The editor, renderer and the headless
    //     dev tools borrow these by reference directly. ---

    /// The active scene (the world of record).
    pub fn scene(&self) -> &Scene {
        &self.world.scene
    }

    /// The active scene, mutably.
    pub fn scene_mut(&mut self) -> &mut Scene {
        &mut self.world.scene
    }

    /// The input resource.
    pub fn input(&self) -> &InputState {
        &self.resources.input
    }

    /// The input resource, mutably (the platform layer feeds key events through it).
    pub fn input_mut(&mut self) -> &mut InputState {
        &mut self.resources.input
    }

    /// The navigation graph resource.
    pub fn nav(&self) -> &NavigationGraph {
        &self.resources.nav
    }

    /// The console log buffer resource.
    pub fn console(&self) -> &ConsoleLogs {
        &self.resources.console
    }

    /// The console log buffer resource, mutably.
    pub fn console_mut(&mut self) -> &mut ConsoleLogs {
        &mut self.resources.console
    }

    /// The active camera resource.
    pub fn camera(&self) -> &Camera {
        &self.resources.camera
    }

    /// The frame-clock resource.
    pub fn time(&self) -> &Time {
        &self.resources.time
    }

    /// The frame-clock resource, mutably.
    pub fn time_mut(&mut self) -> &mut Time {
        &mut self.resources.time
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
            self.resources.time.advance(dt);
            self.resources.time.delta_time
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

    pub(super) fn handle_transition(&mut self) -> PlayTransition {
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
}
