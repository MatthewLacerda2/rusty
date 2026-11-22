use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::rc::Rc;

use serde_json::json;

use super::console;
use rusty_app::{GameWorld, ProjectSource};
use rusty_core::input::InputState;
use rusty_core::project::{Access, BootPlan};
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::{ConsoleLogs, LogLevel};

pub struct Session {
    world: GameWorld,
    frame_errors: RefCell<Vec<String>>,
}

impl Session {
    pub fn new(boot_scene: &str) -> Result<Self, String> {
        Self::boot(&BootPlan::of(boot_scene), ProjectSource::defaults())
    }

    pub fn boot(plan: &BootPlan, project: ProjectSource) -> Result<Self, String> {
        let scene = Rc::new(RefCell::new(Scene::new()));
        let input = Rc::new(RefCell::new(InputState::new()));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));

        let loaded = rusty_app::load_boot_scene(
            &mut scene.borrow_mut(),
            &mut console.borrow_mut(),
            plan,
            false,
        );
        let nav = Rc::new(RefCell::new(NavigationGraph::from_scene(&scene.borrow())));

        super::lua_surface::install_api();
        let mut world = GameWorld::new(scene, input, nav, console);
        world.open_project(project);
        if let Some(path) = loaded {
            *world
                .resources
                .script_manager
                .scene_path_cell()
                .borrow_mut() = Some(path);
        }
        world.init_edit_runtime()?;
        super::stats::install(&mut world);
        Ok(Self {
            world,
            frame_errors: RefCell::default(),
        })
    }

    pub fn command(&mut self, line: &str) -> Result<String, String> {
        let outcome = self.eval(line);
        let mark = self.world.console().borrow().logged();
        self.world.paused_frame();
        let console = self.world.console().borrow();
        let errors = console.since(mark).filter(|(_, l)| *l == LogLevel::Error);
        let mut kept = self.frame_errors.borrow_mut();
        kept.extend(errors.map(|(m, _)| m.clone()));
        outcome
    }

    pub fn eval(&self, line: &str) -> Result<String, String> {
        console::evaluate_line(self.world.script_manager(), self.world.console(), line)
    }

    pub fn take_warnings(&self) -> Vec<String> {
        let mut taken = rusty_logging::forward(self.world.console());
        taken.append(&mut self.frame_errors.borrow_mut());
        taken
    }

    pub fn world(&self) -> &GameWorld {
        &self.world
    }
}

pub fn boot_from_args(args: &[String]) -> (BootPlan, ProjectSource) {
    let mut rest = args.to_vec();
    let opened = rusty_core::project::open_from_args(&mut rest, Access::Edit);
    seed();
    let plan = match rest.first().map(String::as_str) {
        Some("--empty") => BootPlan::default(),
        Some(path) => BootPlan::of(path),
        None => boot_scene(&opened),
    };
    (plan, ProjectSource::bound(&opened))
}

pub(crate) fn seeded_boot_scene(opened: &rusty_core::project::Opened) -> BootPlan {
    seed();
    boot_scene(opened)
}

fn seed() {
    rusty_edit::print_refresh(&rusty_edit::seed_project());
}

fn boot_scene(opened: &rusty_core::project::Opened) -> BootPlan {
    let startup = &opened.settings.build.startup_scene;
    rusty_core::project::boot_scene(&opened.root, startup, Access::Edit).unwrap_or_default()
}

pub(crate) fn response_line(outcome: &Result<String, String>, warnings: &[String]) -> String {
    let mut value = match outcome {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    if !warnings.is_empty() {
        value["warnings"] = json!(warnings);
    }
    value.to_string()
}

pub fn run<R: BufRead, W: Write>(
    session: &mut Session,
    input: R,
    mut output: W,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let outcome = session.command(&line);
        let warnings = session.take_warnings();
        writeln!(output, "{}", response_line(&outcome, &warnings))?;
        output.flush()?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "session_boot_tests.rs"]
mod boot_tests;
#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
