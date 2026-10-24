//! How a harness is built: on the engine's default scene in a workspace of its own
//! (the tests), or on the open project's scene (`play`, #1343).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::{fixture_project, Harness};
use crate::app::GameWorld;
use crate::core::application::BuildSettings;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

impl Harness {
    /// Build a harness around the default scene — the one the editor seeds (#667) —
    /// and enter play mode. `bot_script` is the enemy brain path (empty to skip it).
    ///
    /// The run gets its own workspace, `<out_dir>/workspace`, seeded fresh from the
    /// test-fixture project's scripts ([`fixture_project`]) and the engine's bundled
    /// ones, and the scene's relative script paths resolve there (#782): a test
    /// judges the engine's scripts, never a developer's project.
    ///
    /// The workspace is seeded before the scene attaches its scripts, so the first
    /// run and every rerun see the same files — otherwise the enemy brain would be
    /// absent on the first run and present afterwards, breaking byte-determinism.
    pub fn new(out_dir: impl AsRef<Path>, bot_script: &str) -> Self {
        let workspace = Self::workspace_of(&out_dir);
        let samples = fixture_project().join(crate::core::paths::native(
            crate::scene::DEFAULT_SCRIPTS_DEST_DIR,
        ));
        crate::scene::seed_workspace(&workspace, Some(&samples));
        // The default scene's texture and shader, seeded into the working directory
        // only when missing; only screenshots read them.
        crate::scene::default_scene::seed_default_assets();
        let mut scene = Scene::new();
        crate::scene::default_scene::build(&mut scene, bot_script);
        Self::in_workspace(out_dir, scene, workspace)
    }

    /// A harness on the open project (`core::project`, the working directory),
    /// playing the scene at `scene_path` — what an agent play-testing its game wants
    /// (the `play` binary passes the project's startup scene, #1343). The bundled
    /// scripts and the default scene are seeded through the seed manifest first, as
    /// the standalone player's boot does, so a fresh project's startup scene exists.
    /// A scene document that can't be read or parsed is an error: a play-test of
    /// nothing proves nothing. A bad lighting sidecar is not: it goes to the console,
    /// as the player's boot (`shell::boot::load_game`) logs it, and probes play unbaked.
    ///
    /// Seeding before the scene loads means the first run and every rerun see the
    /// same scripts, so a scenario replays byte for byte (as for [`Harness::new`]).
    pub fn in_user_workspace(
        out_dir: impl AsRef<Path>,
        scene_path: &str,
        build: BuildSettings,
    ) -> Result<Self, String> {
        crate::scene::seed_default_scripts();
        crate::scene::seed_default_scene();
        crate::scene::io::read_scene_file(scene_path)?;
        let mut scene = Scene::new();
        // The document parsed just above, so what fails here is the sidecar.
        let sidecar = scene.load_from_file(scene_path);
        let harness = Self::in_workspace(out_dir, scene, PathBuf::new());
        if let Err(err) = sidecar {
            let line = format!("Failed to load scene {scene_path}: {err}");
            harness.console.borrow_mut().error(line);
        }
        {
            let world = harness.world.borrow();
            *world.script_manager().scene_path_cell().borrow_mut() = Some(scene_path.into());
            // Unbound, as in the player: a run reads the build settings, never writes them.
            let _ = world.resources.application.borrow_mut().set_build(build);
        }
        Ok(harness)
    }

    /// The workspace a harness built in `out_dir` runs in: see [`Harness::new`].
    pub fn workspace_of(out_dir: impl AsRef<Path>) -> PathBuf {
        out_dir.as_ref().join("workspace")
    }

    fn in_workspace(out_dir: impl AsRef<Path>, scene: Scene, workspace: PathBuf) -> Self {
        // Baked over the scene's own bounds (#452) — the same path the windowed game takes.
        let nav = NavigationGraph::from_scene(&scene);
        let scene = Rc::new(RefCell::new(scene));
        let input = Rc::new(RefCell::new(InputState::new()));
        let nav = Rc::new(RefCell::new(nav));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));

        crate::dev::lua_surface::install_api();
        let mut world = GameWorld::new(
            Rc::clone(&scene),
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
        );
        // Headless harness always runs the simulation (play mode), with no edit mode
        // to return to: standalone, so `Editor.ExitPlay()` is refused (#1360).
        world.resources.script_manager.set_workspace_root(workspace);
        world.resources.application.borrow_mut().mark_standalone();
        world.set_playing(true);
        let stats = crate::dev::stats::install(&mut world);

        Self {
            world: Rc::new(RefCell::new(world)),
            console,
            logs: Vec::new(),
            expectations: Vec::new(),
            out_dir: out_dir.as_ref().to_path_buf(),
            capture: crate::dev::capture::CaptureHost::new(),
            stats,
            _providers: crate::dev::providers::HeadlessHold::take(),
        }
    }
}
