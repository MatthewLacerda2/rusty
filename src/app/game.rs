//! src/app/game.rs — GameWorld: the simulation, decoupled from window and GPU.
//!
//! Owns the world of record ([`World`] — a handle to the active `Scene`) and the
//! engine singletons ([`Resources`] — input, nav, console, camera, time, the live
//! rapier world, the script runtime, and the play-mode bookkeeping). It advances
//! both one frame via `tick(dt)`. The windowed shell and the headless harness drive
//! the same `tick`; only the input source and the rendering differ.
//!
//! A system is the canonical `fn(&mut World, &mut Resources)` (#39): `tick` threads
//! both through the schedule, so the borrow checker keeps world storage and engine
//! state apart. The handles inside `Resources` stay `Rc<RefCell<…>>` (the mlua
//! closures capture them).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use crate::core::input::{CursorState, InputState};
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::{Camera, Scene, SceneSnapshot};
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

    /// The navigation graph resource (test seam for `tests/nav/bounds_boot.rs`).
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

    /// Whether the host asked for Play ([`set_playing`](Self::set_playing)); the
    /// World enters it at the next tick's transition. Whether it *is* in Play is
    /// the scene's single play state, `Scene::is_playing` (#1302, #1304).
    pub fn play_requested(&self) -> bool {
        self.resources.application.borrow().host_play()
    }

    /// Ask to enter or leave Play, every host's one way in (the editor transport,
    /// the player, the harness): the next tick's transition acts on it.
    pub fn set_playing(&mut self, playing: bool) {
        self.resources
            .application
            .borrow_mut()
            .set_host_play(playing);
    }

    /// Advance the simulation by `dt` seconds. Returns any play-state transition so
    /// the caller can handle platform-only concerns (cursor grab/visibility). Every
    /// tick reads the UI's texture sizes after the transition, before the schedule in
    /// Play and in the edit frame outside it, so no loop that ticks can skip them.
    pub fn tick(&mut self, dt: f32) -> PlayTransition {
        let transition = self.handle_transition();
        // Publish input written since the last tick as this tick's edges (#416).
        self.resources.input.borrow_mut().begin_tick();
        // `advance` records raw `dt` (unscaled) and the scaled `delta_time`.
        // The game sim integrates the scaled value.
        let scaled_dt = {
            let mut time = self.resources.time.borrow_mut();
            time.advance(dt);
            time.delta_time
        };
        if self.world.scene.borrow().is_playing() {
            // Run the schedule's per-frame stages against (&mut World, &mut
            // Resources). The schedule is moved out of `Resources` for the call (its
            // systems take `&mut Resources`) and restored after, avoiding a
            // self-aliasing borrow. `Startup` runs once, in `enter_play`.
            self.resources.frame_dt = scaled_dt;
            self.refresh_ui_textures();
            self.run_schedule_frame();
        } else {
            self.edit_frame();
        }
        self.sync_render_camera();
        self.sync_shader_time();
        transition
    }

    /// Process only the play-state transition, **without advancing the sim** (#283):
    /// a paused, no-step frame, where a Play/Stop must still take effect. Outside
    /// Play the caller runs [`edit_frame`](Self::edit_frame) after it, which re-reads
    /// UI texture sizes after a Stop; entering Play re-reads them here.
    pub fn poll_transition(&mut self) -> PlayTransition {
        let transition = self.handle_transition();
        self.refreshed_after(transition)
    }

    /// Run the schedule's per-frame stages. The schedule is moved out of `Resources`
    /// for the duration so its systems can take `&mut Resources`.
    fn run_schedule_frame(&mut self) {
        let schedule = std::mem::take(&mut self.resources.schedule);
        schedule.run_frame(&mut self.world, &mut self.resources);
        self.resources.schedule = schedule;
    }

    /// Number of play-mode frames simulated since the last `enter_play`.
    pub fn play_frame(&self) -> u64 {
        self.resources.play_frame
    }

    fn handle_transition(&mut self) -> PlayTransition {
        // A script's `Editor.EnterPlay/ExitPlay` (#1360) asks like any host does.
        let scripted = self.resources.application.borrow_mut().take_play_request();
        if let Some(playing) = scripted {
            self.set_playing(playing);
        }
        let asked = self.play_requested();
        let playing = self.world.scene.borrow().is_playing();
        if asked && !playing {
            // Before `Start`, so a script can override the locked+hidden default.
            self.resources
                .input
                .borrow_mut()
                .reset_cursor(CursorState::PLAY);
            self.enter_play();
            PlayTransition::Entered
        } else if !asked && playing {
            self.resources
                .input
                .borrow_mut()
                .reset_cursor(CursorState::FREE);
            self.exit_play();
            PlayTransition::Exited
        } else {
            PlayTransition::None
        }
    }

    fn enter_play(&mut self) {
        self.refresh_then_hold_imports(); // #1253
        self.resources.play_frame = 0;
        self.resources.time.borrow_mut().reset();
        // Reload animation graphs fresh each Play session (#316), so asset edits
        // made in between are picked up without restarting the engine.
        self.resources.animation_graphs.clear();
        // Start the audio log fresh for this Play session (#212) and silence any
        // voice that lingered from a prior run.
        {
            let mut audio = self.resources.audio.borrow_mut();
            audio.stop_all();
            audio.clear_log();
        }
        // Snapshot the edit scene so Stop can restore it, discarding play-mode mutations.
        // A standalone run has no edit mode to return to (#431).
        if !self.resources.application.borrow().standalone() {
            self.resources.edit_snapshot = Some(SceneSnapshot::capture(&self.world.scene.borrow()));
            self.capture_play_origin();
        }
        self.world.scene.borrow_mut().set_playing(true); // the one play state (#1302)
        self.resources
            .console
            .borrow_mut()
            .info("Capturing cursor, entering PlayMode!".to_string());

        let init = self.resources.script_manager.init_runtime();
        if !self.report_lua_init(init) {
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
        // save data survives even though the scene snapshot is discarded below), and
        // so are the graphics settings an options menu changed during Play.
        self.resources.store_graphics();
        self.resources.flush_storage();
        self.resources.script_manager.shutdown();
        self.resources.play_frame = 0;
        *self.resources.physics.borrow_mut() = None;
        // Silence every voice on Stop (Unity stops play-mode audio at exit), and
        // discard play-mode mixer changes (#465) as the scene's are below.
        self.resources.audio.borrow_mut().exit_play();
        self.resources.play_hold = None; // conversion on before the restore (#1253)
        self.world.scene.borrow_mut().set_playing(false); // before the restore (#1302)
        if let Some(snapshot) = self.resources.edit_snapshot.take() {
            snapshot.restore(&mut self.world.scene.borrow_mut());
            self.resources
                .nav
                .borrow_mut()
                .bake(&self.world.scene.borrow());
        }
        self.restore_play_origin();
        self.restore_edit_runtime(); // after the restore: it binds the edit scene (#1339)
    }
}

#[cfg(test)]
#[path = "game_tests.rs"]
mod game_tests;

#[cfg(test)]
#[path = "game_loop_tests.rs"]
mod game_loop_tests;
