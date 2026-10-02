//! src/shell/boot.rs — the boot steps both frontends share: the project workspace,
//! the window, the renderer, the simulation, audio, and the assembled [`Shell`].

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

use super::frame::FrameClock;
use super::{settings, Shell};
use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::core::keymap::{Keymap, KEYBINDINGS_KEY, KEYBINDINGS_NAMESPACE};
use crate::core::video::VideoSettings;
use crate::navigation::NavigationGraph;
use crate::render::Renderer;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Set up the git-ignored project folder structure and seed the bundled scripts and
/// the default scene. Idempotent: existing files are left untouched.
pub fn seed_project_workspace() {
    for dir in [
        "project/assets/textures",
        "project/assets/models",
        "project/assets/audio",
        "project/scenes",
        // Authored shaders (#272) bake here; a `ShaderRegistry` loads them by name.
        crate::shadergen::DEFAULT_OUT_DIR,
    ] {
        std::fs::create_dir_all(dir).ok();
    }
    // The bundled GAME scripts (player_controller.lua, bot.lua) and the demo scene
    // that uses them; the play loop itself runs no gameplay.
    crate::scene::seed_default_scripts();
    crate::scene::seed_default_scene();
}

/// Open the window at `size` with `title`.
pub fn create_window(
    event_loop: &EventLoop<()>,
    title: &str,
    (width, height): (u32, u32),
) -> Arc<winit::window::Window> {
    Arc::new(
        WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .build(event_loop)
            .expect("the OS refused to create a window"),
    )
}

/// Build the renderer on `window`. Exits the process if no compatible GPU exists.
pub fn init_renderer(window: &Arc<winit::window::Window>) -> Renderer {
    match pollster::block_on(Renderer::new(Arc::clone(window))) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("[Engine] Could not initialize the renderer: {err}");
            eprintln!("[Engine] No compatible GPU is available — exiting.");
            std::process::exit(1);
        }
    }
}

/// Build the simulation around the scene at `scene_path`: shared engine state, baked
/// nav, and the persistent store bound to its file. A scene that fails to load is
/// logged to the console and leaves an empty scene.
pub fn load_game(scene_path: &str) -> GameWorld {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    {
        let mut c = console.borrow_mut();
        c.info(format!("Loading scene from {scene_path}..."));
        if let Err(err) = scene.borrow_mut().load_from_file(scene_path) {
            c.error(format!("Failed to load scene {scene_path}: {err}"));
        }
    }
    // Baked over the scene's own bounds (#452) — the same path the headless harness takes.
    let nav = Rc::new(RefCell::new(NavigationGraph::from_scene(&scene.borrow())));
    let input = Rc::new(RefCell::new(InputState::new()));
    #[cfg_attr(not(feature = "dev"), allow(unused_mut))] // the dev probe install mutates it
    let mut game = GameWorld::new(scene, input, nav, console);
    // Dev builds time the schedule so `Debug.Stats()` answers in a playtest (#433).
    #[cfg(feature = "dev")]
    crate::dev::stats::install(&mut game);
    *game.script_manager().scene_path_cell().borrow_mut() = Some(scene_path.to_string());

    // Bind the persistent store to its file and load it once — this boundary read is
    // a sim input. Writes flush at Stop and on loop exit. The harness leaves the store
    // pathless so headless runs never read this file.
    let opened = game
        .resources
        .storage
        .borrow_mut()
        .open(crate::core::storage::DEFAULT_STORAGE_PATH);
    if let Err(err) = opened {
        game.console()
            .borrow_mut()
            .error(format!("Failed to load storage: {err}"));
    }
    game
}

/// Open the real audio device and inject it into the `AudioMaestro` (#212). A box
/// with no audio device keeps the no-op backend, logging instead of failing.
pub fn init_audio(game: &GameWorld) {
    match crate::audio::KiraBackend::open() {
        Some(backend) => game
            .resources
            .audio
            .borrow_mut()
            .set_backend(Box::new(backend)),
        None => game
            .console()
            .borrow_mut()
            .info("No audio device available — running silent.".to_string()),
    }
}

/// Load the physical→logical key remap (#88) from the persistent store. A missing
/// binding blob means the identity mapping.
pub fn load_keymap(game: &GameWorld) -> Keymap {
    let storage = game.resources.storage.borrow();
    match storage.get(KEYBINDINGS_NAMESPACE, KEYBINDINGS_KEY) {
        Some(blob) => Keymap::from_json(&blob),
        None => Keymap::new(),
    }
}

impl Shell {
    /// Assemble the shell around a booted `game`: the keymap, the frame clock, the
    /// dev command channel, and the persisted video/quality settings applied to the
    /// surface and window. `video_defaults` is what a first launch (no saved video
    /// settings) gets.
    pub fn new(
        window: Arc<winit::window::Window>,
        renderer: Renderer,
        game: &GameWorld,
        video_defaults: VideoSettings,
    ) -> Self {
        init_audio(game);
        let mut shell = Self {
            clipboard: super::input::clipboard_source::OsClipboard::open(&window),
            window,
            renderer,
            clock: FrameClock::new(Instant::now()),
            keymap: load_keymap(game),
            applied_video: VideoSettings::default(),
            cursor: super::input::CursorPolicy::default(),
            pad_source: super::input::pad_source::GilrsSource::open(),
            pads: super::input::pads::PadPump::default(),
            modifiers: winit::keyboard::ModifiersState::empty(),
            window_focused: true,
            // A bind failure logs to the game's console and leaves the channel `None`.
            #[cfg(feature = "dev")]
            cmd_channel: crate::dev::command_channel::CommandChannel::start(game.console()),
        };
        // Boundary read: apply the persisted settings before the first frame.
        settings::load(&mut shell, game, video_defaults);
        shell
    }
}
