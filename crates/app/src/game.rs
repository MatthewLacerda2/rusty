use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;

use rusty_core::input::{CursorState, InputState};
use rusty_edit::SceneSnapshot;
use rusty_navigation::NavigationGraph;
use rusty_scene::{Camera, Scene};
use rusty_script::scripting::{ConsoleLogs, ScriptManager};
use rusty_time::Time;

use super::probe::{timed, LoadProbe};
use super::tap::TapPoint;
use super::{Resources, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayTransition {
    None,
    Entered,
    Exited,
}

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
        let eye = Vec3::new(0.0, 5.0, -10.0);
        let camera = Rc::new(RefCell::new(Camera::new(eye, 90.0, -20.0)));
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

    pub fn scene(&self) -> &Rc<RefCell<Scene>> {
        &self.world.scene
    }

    pub fn input(&self) -> &Rc<RefCell<InputState>> {
        &self.resources.input
    }

    pub fn nav(&self) -> &Rc<RefCell<NavigationGraph>> {
        &self.resources.nav
    }

    pub fn console(&self) -> &Rc<RefCell<ConsoleLogs>> {
        &self.resources.console
    }

    pub fn camera(&self) -> &Rc<RefCell<Camera>> {
        &self.resources.camera
    }

    pub fn time(&self) -> &Rc<RefCell<Time>> {
        &self.resources.time
    }

    pub fn script_manager(&self) -> &ScriptManager {
        &self.resources.script_manager
    }

    pub fn play_requested(&self) -> bool {
        self.resources.application.borrow().host_play()
    }

    pub fn set_playing(&mut self, playing: bool) {
        let mut app = self.resources.application.borrow_mut();
        app.set_host_play(playing);
    }

    pub fn tick(&mut self, dt: f32) -> PlayTransition {
        self.tap(TapPoint::BeforeTick);
        let transition = self.transition_tapped();
        self.probe_tick(true);
        let dt = self.tap_input(dt);
        self.resources.input.borrow_mut().begin_tick();
        // `advance` keeps the raw `dt`; the sim integrates the scaled `delta_time`.
        let scaled_dt = {
            let mut time = self.resources.time.borrow_mut();
            time.advance(dt);
            time.delta_time
        };
        if self.world.scene.borrow().is_playing() {
            self.resources.frame_dt = scaled_dt;
            self.refresh_ui_textures();
            self.run_schedule_frame();
        } else {
            self.edit_frame();
        }
        self.sync_render_camera();
        self.sync_shader_time();
        self.push_audio(dt);
        self.start_loads();
        self.tap_timed(TapPoint::AfterTick);
        self.probe_tick(false);
        transition
    }

    /// Process only the play-state transition, without advancing the sim.
    pub fn poll_transition(&mut self) -> PlayTransition {
        let transition = self.transition_tapped();
        self.tap(TapPoint::AfterPoll);
        self.refreshed_after(transition)
    }

    fn run_schedule_frame(&mut self) {
        let schedule = std::mem::take(&mut self.resources.schedule);
        schedule.run_frame(&mut self.world, &mut self.resources);
        self.resources.schedule = schedule;
    }

    pub fn play_frame(&self) -> u64 {
        self.resources.play_frame
    }

    pub(super) fn handle_transition(&mut self) -> PlayTransition {
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
        let mut probe = self.resources.load_probes.entry.take();
        let entered = self.enter_play_steps(&mut probe);
        if let Some(probe) = probe.as_mut() {
            let path = self.resources.script_manager.scene_path_cell();
            let path = entered.then(|| path.borrow().clone().unwrap_or_default());
            probe.finish(path.as_deref(), &mut self.resources.console.borrow_mut());
        }
        self.resources.load_probes.entry = probe;
    }

    fn enter_play_steps(&mut self, probe: &mut Option<Box<dyn LoadProbe>>) -> bool {
        timed(probe, "refresh", || self.refresh_then_hold_imports());
        self.resources.play_frame = 0;
        self.resources.time.borrow_mut().reset();
        self.resources.animation_graphs.clear();
        {
            let mut audio = self.resources.audio.borrow_mut();
            audio.stop_all();
            audio.clear_log();
        }
        // A standalone run has no edit mode to return to.
        if !self.resources.application.borrow().standalone() {
            self.resources.edit_snapshot = Some(SceneSnapshot::capture(&self.world.scene.borrow()));
            self.capture_play_origin();
        }
        self.apply_project_settings();
        self.world.scene.borrow_mut().set_playing(true);
        self.resources
            .console
            .borrow_mut()
            .info("Capturing cursor, entering PlayMode!".to_string());
        // ALL `Awake`s, then ALL `Start`s, actives only: a disabled-at-load entity
        // defers both until its first active tick.
        let started = timed(probe, "scripts", || {
            let init = self.resources.script_manager.init_runtime();
            let ok = self.report_lua_init(init);
            if ok {
                self.resources.script_manager.init_scripts();
            }
            ok
        });
        if !started {
            return false;
        }
        timed(probe, "physics", || self.build_physics());
        timed(probe, "startup", || self.run_startup_stage());
        true
    }

    fn exit_play(&mut self) {
        // Stop is a storage boundary: save data survives the discarded snapshot.
        self.resources.store_graphics();
        self.resources.flush_storage();
        self.resources.script_manager.shutdown();
        self.resources.play_frame = 0;
        self.drop_physics();
        // Unity stops play-mode audio at exit; mixer changes are discarded as the scene's are.
        self.resources.audio.borrow_mut().exit_play();
        self.resources.play_hold = None;
        self.world.scene.borrow_mut().set_playing(false);
        if let Some(snapshot) = self.resources.edit_snapshot.take() {
            snapshot.restore(&mut self.world.scene.borrow_mut());
            self.resources
                .nav
                .borrow_mut()
                .restore_or_bake(&self.world.scene.borrow());
        }
        self.restore_play_origin();
        self.apply_project_settings();
        self.restore_edit_runtime();
    }
}

#[cfg(test)]
#[path = "game_tests.rs"]
pub(super) mod game_tests;

#[cfg(test)]
#[path = "game_loop_tests.rs"]
mod game_loop_tests;
