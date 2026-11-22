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
    pub fn new(out_dir: impl AsRef<Path>, bot_script: &str) -> Self {
        let workspace = Self::workspace_of(&out_dir);
        let samples = fixture_project().join(rusty_edit::DEFAULT_SCRIPTS_DEST_DIR);
        rusty_edit::seed_workspace(&workspace, Some(&samples));
        rusty_edit::default_scene::seed_default_assets();
        let mut scene = Scene::new();
        rusty_edit::default_scene::build(&mut scene, bot_script);
        Self::in_workspace(out_dir, scene, workspace)
    }

    pub fn in_user_workspace(
        out_dir: impl AsRef<Path>,
        scene_path: &str,
        settings: ProjectSettings,
    ) -> Result<Self, String> {
        rusty_edit::print_refresh(&rusty_edit::seed_project());
        Self::on_project_scene(out_dir, scene_path, settings)
    }

    pub fn on_project_scene(
        out_dir: impl AsRef<Path>,
        scene_path: &str,
        settings: ProjectSettings,
    ) -> Result<Self, String> {
        Self::on_project_scene_timed(out_dir, scene_path, settings, &mut None)
    }

    pub fn on_project_scene_timed(
        out_dir: impl AsRef<Path>,
        scene_path: &str,
        settings: ProjectSettings,
        probe: &mut Option<Box<dyn LoadProbe>>,
    ) -> Result<Self, String> {
        rusty_asset::convert::mark_standalone();
        let read = timed(probe, "document", || {
            let json = rusty_scene::io::read_text(scene_path)?;
            let data = rusty_scene::io::parse_scene_text(&json, scene_path)?;
            Ok::<_, String>((json, data))
        });
        let (json, data) = read?;
        let document = timed(probe, "import", || RehydratedScene::new(data));
        let mut scene = Scene::new();
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
                world.open_project(ProjectSource::ReadOnly(settings));
            }
            harness
        });
        Ok(harness)
    }

    pub fn workspace_of(out_dir: impl AsRef<Path>) -> PathBuf {
        out_dir.as_ref().join("workspace")
    }

    fn in_workspace(out_dir: impl AsRef<Path>, scene: Scene, workspace: PathBuf) -> Self {
        let nav = NavigationGraph::from_scene(&scene);
        Self::in_workspace_with(out_dir, scene, nav, workspace)
    }

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
        console.borrow_mut().keep_warnings();

        crate::lua_surface::install_api();
        let mut world = GameWorld::new(
            Rc::clone(&scene),
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
        );
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
        }
    }
}
