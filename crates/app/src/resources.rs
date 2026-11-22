use std::cell::RefCell;
use std::rc::Rc;

use rusty_audio::AudioMaestro;
use rusty_components::MontageEvent;
use rusty_core::application::Application;
use rusty_core::input::InputState;
use rusty_core::storage::Storage;
use rusty_edit::SceneSnapshot;
use rusty_navigation::NavigationGraph;
use rusty_physics::PhysicsWorld;
use rusty_scene::Camera;
use rusty_scene::Scene;
use rusty_script::scripting::{ConsoleLogs, ScriptManager};
use rusty_time::Time;
use rusty_ui::{EventSystem, ScreenSize, TextureSizeReader, UiLayout};

use super::animation::graph::GraphCache;
use super::Schedule;

pub struct Resources {
    pub input: Rc<RefCell<InputState>>,
    pub nav: Rc<RefCell<NavigationGraph>>,
    pub console: Rc<RefCell<ConsoleLogs>>,
    pub camera: Rc<RefCell<Camera>>,
    pub time: Rc<RefCell<Time>>,
    pub audio: Rc<RefCell<AudioMaestro>>,
    pub script_manager: ScriptManager,
    pub physics: Rc<RefCell<Option<PhysicsWorld>>>,
    /// Empty and pathless in the harness, so headless runs never read a real save.
    pub storage: Rc<RefCell<Storage>>,
    /// A sim input: the game view's pixel size the windowed platform writes each frame;
    /// headless, the video resolution stands in.
    pub screen: Rc<RefCell<ScreenSize>>,
    pub(super) ui_textures: RefCell<TextureSizeReader>,
    pub ui_layout: UiLayout,
    pub event_system: Rc<RefCell<EventSystem>>,
    pub application: Rc<RefCell<Application>>,
    /// Drives nav rebaking, not the wall clock, so replays are bit-stable.
    pub(super) play_frame: u64,
    pub(super) edit_snapshot: Option<SceneSnapshot>,
    pub(super) play_origin: Option<super::scene_load::PlayOrigin>,
    pub(super) edit_runtime: bool,
    pub(super) play_hold: Option<rusty_asset::convert::PlayHold>,
    pub(super) sources_root: std::path::PathBuf,
    pub(super) frame_dt: f32,
    pub(super) schedule: Schedule,
    pub(super) animation_graphs: GraphCache,
    pub(super) animation_events: AnimationQueue,
    pub(super) controller_hits: Vec<(u32, Vec<rusty_physics::ControllerHit>)>,
    pub(super) sight_rays: rusty_ai::perception::SightRays,
    pub(super) probe: Option<Box<dyn super::SystemProbe>>,
    pub(super) load_probes: super::probe::LoadProbes,
    pub(super) tap: Option<Box<dyn super::TickTap>>,
    pub(super) loading: super::scene_load::Loading,
    pub(super) model_writes: rusty_edit::model_writes::ModelWrites,
}

fn audio_cell(
    scene: Rc<RefCell<Scene>>,
    physics: &Rc<RefCell<Option<PhysicsWorld>>>,
    script_manager: &mut ScriptManager,
) -> Rc<RefCell<AudioMaestro>> {
    let audio = Rc::new(RefCell::new(AudioMaestro::default()));
    let occluder = super::audio::occluder(scene, Rc::clone(physics));
    audio.borrow_mut().set_occluder(occluder);
    script_manager.set_audio_cell(Rc::clone(&audio));
    audio
}

impl Resources {
    pub fn new(
        scene: Rc<RefCell<Scene>>,
        input: Rc<RefCell<InputState>>,
        nav: Rc<RefCell<NavigationGraph>>,
        console: Rc<RefCell<ConsoleLogs>>,
        camera: Rc<RefCell<Camera>>,
        time: Rc<RefCell<Time>>,
    ) -> Self {
        let mut script_manager = ScriptManager::new(
            Rc::clone(&scene),
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
            Rc::clone(&camera),
            Rc::clone(&time),
        );
        let physics = script_manager.physics_cell();
        let storage = Rc::new(RefCell::new(Storage::new()));
        script_manager.set_storage(Rc::clone(&storage));
        let audio = audio_cell(scene, &physics, &mut script_manager);
        let screen = Rc::new(RefCell::new(ScreenSize::default()));
        script_manager.set_screen_cell(Rc::clone(&screen));
        let event_system = script_manager.event_system_cell();
        let application = Rc::new(RefCell::new(Application::new()));
        script_manager.set_application_cell(Rc::clone(&application));
        Self {
            input,
            nav,
            console,
            camera,
            time,
            audio,
            script_manager,
            storage,
            screen,
            ui_textures: RefCell::default(),
            ui_layout: UiLayout::default(),
            event_system,
            application,
            physics,
            play_frame: 0,
            edit_snapshot: None,
            play_origin: None,
            edit_runtime: false,
            play_hold: None,
            sources_root: rusty_core::project::ASSETS_DIR.into(),
            frame_dt: 0.0,
            schedule: super::build().into_schedule(),
            animation_graphs: GraphCache::default(),
            animation_events: AnimationQueue::default(),
            controller_hits: Vec::new(),
            sight_rays: Default::default(),
            probe: None,
            load_probes: Default::default(),
            tap: None,
            loading: Default::default(),
            model_writes: Default::default(),
        }
    }

    pub fn store_graphics(&self) {
        let graphics = *self.script_manager.graphics_cell().borrow();
        let mut storage = self.storage.borrow_mut();
        graphics.store(&mut storage);
        let app = self.application.borrow();
        rusty_core::settings::store_quality(&mut storage, app.quality_choice());
    }

    /// A no-op when the store is pathless.
    pub fn flush_storage(&self) {
        if let Err(err) = self.storage.borrow().flush() {
            self.console
                .borrow_mut()
                .error(format!("Failed to flush storage: {}", err));
        }
    }

    /// Through `scene`'s camera entity, so a camera moved this frame is seen where it is.
    pub fn ui_view(&self, scene: &rusty_scene::Scene) -> rusty_ui::UiView {
        let video = *self.script_manager.video_cell().borrow();
        let camera = rusty_scene::game_camera_from_scene(&self.camera.borrow(), scene);
        self.screen.borrow().view(&video, Some(camera))
    }

    pub fn play_frame(&self) -> u64 {
        self.play_frame
    }

    /// A test hook: `rusty-dev`'s bench rows test adds a fixture system beside the engine's.
    #[doc(hidden)]
    pub fn schedule_mut(&mut self) -> &mut super::Schedule {
        &mut self.schedule
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod resources_tests;

#[derive(Default)]
pub(super) struct AnimationQueue {
    /// `(entity, name, whose montage)` per crossed event.
    pub(super) events: Vec<(u32, String, Option<MontageEvent>)>,
    /// `(entity, callback)` per graph-state callback, dispatched first.
    pub(super) states: Vec<(u32, rusty_components::StateCallback)>,
}
