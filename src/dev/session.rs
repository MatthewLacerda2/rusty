//! src/dev/session.rs — Long-lived headless EDIT-mode session + command channel.
//!
//! The keystone (#177). A single process that holds a live [`GameWorld`] in **edit
//! mode** and exposes a line-oriented command channel: each input line is one Lua
//! command, evaluated against the live world through the *one* existing evaluator
//! ([`ScriptManager::eval`](crate::scripting::ScriptManager::eval), reached via [`console::evaluate_line`]). The same
//! evaluator backs the in-editor console — so the headless agent and the editor can
//! never drift apart (the two-VM split the harness used is gone here).
//!
//! State persists across commands because the process stays up: one `GameWorld`
//! lives for the whole session, so a loaded scene, baked nav and imported meshes
//! all survive from one command to the next. Errors are returned to the caller and
//! never tear down the session.
//!
//! Determinism: the session builds the world exactly like the windowed boot (same
//! empty scene + boot-scene load + nav bake) but takes no wall-clock reads itself —
//! the loop only blocks on input. It does NOT force play mode: the agent enters it with
//! `Editor.EnterPlay()` (#1360) and steps the sim explicitly with `Time.Pause()` +
//! `Time.Step(n)`. Nothing advances on its own: after each command the channel runs
//! one paused frame ([`Session::command`]), which takes a Play/Stop request and pumps
//! the queued fixed-dt steps, exactly as the windowed loop does while paused.

use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::rc::Rc;

use serde_json::json;

use super::console;
use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::core::project::Access;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

/// A live edit-mode engine session: owns the world and the live evaluator.
pub struct Session {
    world: GameWorld,
    /// Errors the console took during a command's frame (a script's `Start`, a Lua
    /// init failure), until [`take_warnings`](Session::take_warnings) hands them on.
    frame_errors: RefCell<Vec<String>>,
}

impl Session {
    /// Build a session around a fresh world and bring the edit-mode runtime live.
    ///
    /// Mirrors the windowed boot: seed the bundled scripts, create the shared
    /// engine cells, optionally load `boot_scene` and bake nav, then construct the
    /// world and initialise the evaluator **without entering play**. `boot_scene`
    /// empty means start from an empty scene (the agent authors into it live).
    pub fn new(boot_scene: &str) -> Result<Self, String> {
        crate::scene::seed_default_scripts();

        let scene = Rc::new(RefCell::new(Scene::new()));
        let input = Rc::new(RefCell::new(InputState::new()));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));

        let loaded = !boot_scene.is_empty() && {
            let mut c = console.borrow_mut();
            match scene.borrow_mut().load_from_file(boot_scene) {
                Ok(()) => true,
                Err(err) if err.replaced_world() => {
                    c.warn(format!("Scene {boot_scene}: {err}"));
                    true
                }
                Err(err) => {
                    c.error(format!("Failed to load scene {boot_scene}: {err}"));
                    false
                }
            }
        };
        // Baked over the scene's own bounds (#452), as the windowed boot does.
        let nav = Rc::new(RefCell::new(NavigationGraph::from_scene(&scene.borrow())));

        super::lua_surface::install_api();
        let mut world = GameWorld::new(scene, input, nav, console);
        // The boot scene is the `Scene.Save()` write-back target, so an
        // argument-less save from the agent persists back to the loaded file —
        // just as the editor's Save writes to its current scene file. Only when it
        // loaded: an unparseable one stays untouched by an argument-less save (#1374).
        if loaded {
            *world
                .resources
                .script_manager
                .scene_path_cell()
                .borrow_mut() = Some(boot_scene.to_string());
        }
        world.init_edit_runtime()?;
        // Frame stats for `Debug.Stats()` once the agent plays the world (#433).
        super::stats::install(&mut world);
        Ok(Self {
            world,
            frame_errors: RefCell::default(),
        })
    }

    /// One command from the channel: [`eval`](Self::eval) the line, then run one
    /// paused frame ([`GameWorld::paused_frame`]) so a Play/Stop it asked for takes
    /// effect and the `Time.Step` steps it queued run before the next command (#1360).
    /// An error that frame logs (a script's `Awake`/`Start`/`Update`, a Lua init
    /// failure) rides along with the reply, through [`take_warnings`](Self::take_warnings).
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

    /// Evaluate one command line against the live world, returning the rendered
    /// result or the error message. Drives the full `api::` surface through the one
    /// evaluator; never panics on a bad line. Runs no frame: see
    /// [`command`](Self::command).
    pub fn eval(&self, line: &str) -> Result<String, String> {
        // Pass the shared console *cell*, not a held borrow: `evaluate_line` borrows
        // it narrowly only at the log sites, leaving it free during eval so a script
        // that calls `print` / `Debug.*` doesn't double-borrow it and panic (#208).
        console::evaluate_line(self.world.script_manager(), self.world.console(), line)
    }

    /// The engine warnings and errors logged since the last call (#1139), moved into
    /// the console as they are taken, then the errors a command's frame wrote to the
    /// console (#1360). The first call also returns what the boot
    /// raised (a scene that failed to load an asset, say).
    pub fn take_warnings(&self) -> Vec<String> {
        let mut taken = crate::logging::forward(self.world.console());
        taken.append(&mut self.frame_errors.borrow_mut());
        taken
    }

    /// The live world, for tests / embedders that want to inspect state directly.
    pub fn world(&self) -> &GameWorld {
        &self.world
    }
}

/// Open the project and resolve the boot scene from a session bin's CLI args.
///
/// Shared by the `session` and `session-mcp` bins so they parse arguments
/// identically. The accepted forms are:
///   - `--project <dir>` — the game project to open (`core::project::open`, #829):
///     its root becomes the working directory, so the scene and asset paths the
///     engine reads resolve against it. Without it, `./project`. On a bad
///     directory this prints to stderr and exits 2.
///   - `--empty`         — start from a blank scene (returns an empty boot path).
///   - `<scene-path>`    — boot that specific scene (relative to the project root).
///   - none              — seed the bundled default scene (a new project gets it
///     once, #1618) and boot the project's boot scene
///     (`core::project::boot_scene`: the editor's last scene, then the startup
///     scene, then an empty one).
///
/// `--project` may sit anywhere; it is taken out first and the rest is read as the
/// scene argument.
pub fn boot_scene_from_args(args: &[String]) -> String {
    let mut rest = args.to_vec();
    let opened = crate::core::project::open_from_args(&mut rest, Access::Edit);
    match rest.first().map(String::as_str) {
        Some("--empty") => String::new(),
        Some(path) => path.to_string(),
        None => seeded_boot_scene(&opened),
    }
}

/// Seed the default scene into the open project, then pick the scene it boots on,
/// as the editor does (`shell::editor::start`): `session`, `session-mcp` and a
/// `bench_open` switch all open a project here.
pub(crate) fn seeded_boot_scene(opened: &crate::core::project::Opened) -> String {
    crate::scene::seed_default_scene();
    let startup = &opened.settings.build.startup_scene;
    crate::core::project::boot_scene(&opened.root, startup, Access::Edit)
}

/// One framed response line for an evaluated command.
///
/// Each command yields exactly one JSON object on its own line, so an agent can
/// read responses one-per-line in lockstep with the commands it sent:
/// `{"ok":true,"result":"…"}` or `{"ok":false,"error":"…"}`. `warnings` are the
/// engine warnings and errors the caller drained for this reply (#1139): the
/// headless session passes everything raised since its previous reply, the windowed
/// channel what was raised while *this line* was evaluated (the rest go to the
/// window's console). They ride along as `"warnings":[…]`; the key is absent when
/// there are none.
///
/// Shared with the windowed command channel ([`super::command_channel`]) so the
/// headless and windowed responses are byte-identical — the framing lives here once.
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

/// Run the command channel: read commands line-by-line from `input`, run each as a
/// [`Session::command`], and write one framed JSON response line per command to
/// `output`. Blank lines are skipped silently. Loop ends at EOF (channel closed).
///
/// Errors in a command are reported in the response and do **not** end the loop —
/// the session stays up across bad commands. An I/O error on the channel itself is
/// fatal and propagates.
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
