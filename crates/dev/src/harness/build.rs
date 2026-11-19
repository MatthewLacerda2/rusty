//! How a harness is built: on the engine's default scene in a workspace of its own
//! (the tests), or on the open project's scene (`play`, #1343).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::{fixture_project, Harness};
use rusty_app::{timed, GameWorld, LoadProbe, ProjectSource};
use rusty_core::input::InputState;
use rusty_core::project::ProjectSettings;
use rusty_navigation::NavigationGraph;
use rusty_scene::serialize::RehydratedScene;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

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
        let samples = fixture_project().join(rusty_edit::DEFAULT_SCRIPTS_DEST_DIR);
        rusty_edit::seed_workspace(&workspace, Some(&samples));
        // The default scene's texture and shader, seeded into the working directory
        // when missing (the shader also re-baked when stale, #1638); only
        // screenshots read them.
        rusty_edit::default_scene::seed_default_assets();
        let mut scene = Scene::new();
        rusty_edit::default_scene::build(&mut scene, bot_script);
        Self::in_workspace(out_dir, scene, workspace)
    }

    /// A harness on the open project (`core::project`, the working directory),
    /// playing the scene at `scene_path` — what an agent play-testing its game wants
    /// (the `play` binary passes the project's startup scene, #1343). The project is
    /// seeded first (`edit::seed_project`: the bundled scripts, the default scene
    /// through the seed manifest, the audio refresh), as the editor's open and
    /// `make export` do (the standalone player, which runs an export, boots
    /// read-only, #1653), so a fresh project's startup scene exists.
    /// A scene document that can't be read or parsed is an error: a play-test of
    /// nothing proves nothing. A bad lighting sidecar is not: it goes to the console,
    /// as the player's boot (`shell::boot::load_game`) logs it, and probes play unbaked.
    ///
    /// Seeding before the scene loads means the first run and every rerun see the
    /// same scripts, so a scenario replays byte for byte (as for [`Harness::new`]).
    pub fn in_user_workspace(
        out_dir: impl AsRef<Path>,
        scene_path: &str,
        settings: ProjectSettings,
    ) -> Result<Self, String> {
        rusty_edit::print_refresh(&rusty_edit::seed_project());
        Self::on_project_scene(out_dir, scene_path, settings)
    }

    /// [`Self::in_user_workspace`] without seeding the project first: the run
    /// writes nothing into it (`make bench-scenes` benches a project anywhere on
    /// disk, read-only). The scene must already exist.
    pub fn on_project_scene(
        out_dir: impl AsRef<Path>,
        scene_path: &str,
        settings: ProjectSettings,
    ) -> Result<Self, String> {
        Self::on_project_scene_timed(out_dir, scene_path, settings, &mut None)
    }

    /// [`Self::on_project_scene`], each step of the boot load run under `probe`
    /// (`rusty_app::timed`, as a play-mode `Scene.Load` runs its own): `document`
    /// (the file read and parsed, once), `import` (its meshes built, its models
    /// through the import cache), `apply` (the World replaced, the navmesh and
    /// lighting sidecars read), `navmesh` (the graph built or restored from the
    /// scene) and `world` (the game world, its Lua API and stats, the project's
    /// settings). The steps tile the call: nothing between them is left untimed.
    pub fn on_project_scene_timed(
        out_dir: impl AsRef<Path>,
        scene_path: &str,
        settings: ProjectSettings,
        probe: &mut Option<Box<dyn LoadProbe>>,
    ) -> Result<Self, String> {
        // Standalone before the import: a run reads the project's cooked models
        // and writes none (`asset::cook`), as it compresses no texture.
        rusty_asset::convert::mark_standalone();
        let read = timed(probe, "document", || {
            let json = rusty_scene::io::read_text(scene_path)?;
            let data = rusty_scene::io::parse_scene_text(&json, scene_path)?;
            Ok::<_, String>((json, data))
        });
        let (json, data) = read?;
        let document = timed(probe, "import", || RehydratedScene::new(data));
        let mut scene = Scene::new();
        // The document parsed just above, so what fails here is the sidecar.
        let sidecar = timed(probe, "apply", || {
            rusty_edit::load_rehydrated(&mut scene, scene_path, json, document)
        });
        let nav = timed(probe, "navmesh", || NavigationGraph::from_scene(&scene));
        let harness = timed(probe, "world", || {
            let harness = Self::in_workspace_with(out_dir, scene, nav, PathBuf::new());
            if let Err(err) = sidecar {
                let line = format!("Failed to load scene {scene_path}: {err}");
                harness.console.borrow_mut().error(line);
            }
            {
                let world = harness.world.borrow();
                *world.script_manager().scene_path_cell().borrow_mut() = Some(scene_path.into());
                // Unbound, as in the player: a run reads the settings, never writes
                // them. No saved player choices in a harness (its store is pathless):
                // the project's default options (#1438) and initial quality preset
                // (#1522) are what it plays with.
                world.open_project(ProjectSource::ReadOnly(settings));
            }
            harness
        });
        Ok(harness)
    }

    /// The workspace a harness built in `out_dir` runs in: see [`Harness::new`].
    pub fn workspace_of(out_dir: impl AsRef<Path>) -> PathBuf {
        out_dir.as_ref().join("workspace")
    }

    fn in_workspace(out_dir: impl AsRef<Path>, scene: Scene, workspace: PathBuf) -> Self {
        // Baked over the scene's own bounds (#452) — the same path the windowed game takes.
        let nav = NavigationGraph::from_scene(&scene);
        Self::in_workspace_with(out_dir, scene, nav, workspace)
    }

    /// [`Self::in_workspace`] on the navmesh `nav` built from `scene`.
    fn in_workspace_with(
        out_dir: impl AsRef<Path>,
        scene: Scene,
        nav: NavigationGraph,
        workspace: PathBuf,
    ) -> Self {
        let scene = Rc::new(RefCell::new(scene));
        let input = Rc::new(RefCell::new(InputState::new()));
        let nav = Rc::new(RefCell::new(nav));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));
        // A scenario reads every warning of its run (`Harness.Warnings`, #2054).
        console.borrow_mut().keep_warnings();

        crate::lua_surface::install_api();
        let mut world = GameWorld::new(
            Rc::clone(&scene),
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
        );
        // Headless harness always runs the simulation (play mode), with no edit mode
        // to return to: standalone, so `Editor.ExitPlay()` is refused (#1360), and
        // a player's imports: it compresses no texture, playing from the caches the
        // editor or an export made (#1574).
        world.resources.script_manager.set_workspace_root(workspace);
        world.resources.application.borrow_mut().mark_standalone();
        rusty_asset::convert::mark_standalone();
        world.set_playing(true);
        let stats = crate::stats::install(&mut world);

        Self {
            world: Rc::new(RefCell::new(world)),
            console,
            logs: Vec::new(),
            expectations: Vec::new(),
            out_dir: out_dir.as_ref().to_path_buf(),
            capture: crate::capture::CaptureHost::new(),
            stats,
            _providers: crate::providers::HeadlessHold::take(),
        }
    }
}
