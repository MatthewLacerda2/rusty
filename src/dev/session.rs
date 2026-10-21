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
//! the loop only blocks on input. It does NOT force play mode; the agent steps the
//! sim explicitly (e.g. via the `Time`/play surface) if and when it wants to.

use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::rc::Rc;

use serde_json::json;

use super::console;
use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// A live edit-mode engine session: owns the world and the live evaluator.
pub struct Session {
    world: GameWorld,
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

        if !boot_scene.is_empty() {
            let mut s = scene.borrow_mut();
            if let Err(err) = s.load_from_file(boot_scene) {
                console
                    .borrow_mut()
                    .error(format!("Failed to load scene {}: {}", boot_scene, err));
            }
        }
        // Baked over the scene's own bounds (#452), as the windowed boot does.
        let nav = Rc::new(RefCell::new(NavigationGraph::from_scene(&scene.borrow())));

        super::lua_surface::install_api();
        let mut world = GameWorld::new(scene, input, nav, console);
        // The boot scene is the `Scene.Save()` write-back target, so an
        // argument-less save from the agent persists back to the loaded file —
        // just as the editor's Save writes to `current_scene_path`.
        if !boot_scene.is_empty() {
            *world
                .resources
                .script_manager
                .scene_path_cell()
                .borrow_mut() = Some(boot_scene.to_string());
        }
        world.init_edit_runtime()?;
        // Frame stats for `Debug.Stats()` once the agent plays the world (#433).
        super::stats::install(&mut world);
        Ok(Self { world })
    }

    /// Evaluate one command line against the live world, returning the rendered
    /// result or the error message. Drives the full `api::` surface through the one
    /// evaluator; never panics on a bad line.
    pub fn eval(&self, line: &str) -> Result<String, String> {
        // Pass the shared console *cell*, not a held borrow: `evaluate_line` borrows
        // it narrowly only at the log sites, leaving it free during eval so a script
        // that calls `print` / `Debug.*` doesn't double-borrow it and panic (#208).
        console::evaluate_line(self.world.script_manager(), self.world.console(), line)
    }

    /// The engine warnings and errors logged since the last call (#1139), moved into
    /// the console as they are taken. The first call also returns what the boot
    /// raised (a scene that failed to load an asset, say).
    pub fn take_warnings(&self) -> Vec<String> {
        crate::logging::forward(self.world.console())
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
///   - none              — boot (and seed) the bundled default scene.
///
/// `--project` may sit anywhere; it is taken out first and the rest is read as the
/// scene argument.
pub fn boot_scene_from_args(args: &[String]) -> String {
    let mut rest = args.to_vec();
    crate::core::project::open_from_args(&mut rest, crate::core::project::Access::Edit);
    match rest.first().map(String::as_str) {
        Some("--empty") => String::new(),
        Some(path) => path.to_string(),
        None => crate::scene::seed_default_scene(),
    }
}

/// One framed response line for an evaluated command.
///
/// Each command yields exactly one JSON object on its own line, so an agent can
/// read responses one-per-line in lockstep with the commands it sent:
/// `{"ok":true,"result":"…"}` or `{"ok":false,"error":"…"}`. When the engine
/// logged warnings or errors meanwhile, they ride along as `"warnings":[…]` (#1139);
/// the key is absent otherwise.
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

/// Run the command channel: read commands line-by-line from `input`, evaluate each
/// against `session`, and write one framed JSON response line per command to
/// `output`. Blank lines are skipped silently. Loop ends at EOF (channel closed).
///
/// Errors in a command are reported in the response and do **not** end the loop —
/// the session stays up across bad commands. An I/O error on the channel itself is
/// fatal and propagates.
pub fn run<R: BufRead, W: Write>(
    session: &Session,
    input: R,
    mut output: W,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let outcome = session.eval(&line);
        let warnings = session.take_warnings();
        writeln!(output, "{}", response_line(&outcome, &warnings))?;
        output.flush()?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
