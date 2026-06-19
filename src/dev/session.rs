//! src/dev/session.rs — Long-lived headless EDIT-mode session + command channel.
//!
//! The keystone (#177). A single process that holds a live [`GameWorld`] in **edit
//! mode** and exposes a line-oriented command channel: each input line is one Lua
//! command, evaluated against the live world through the *one* existing evaluator
//! ([`ScriptManager::eval`], reached via [`console::evaluate_line`]). The same
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
        let nav = Rc::new(RefCell::new(NavigationGraph::new(
            -20.0, 20.0, -20.0, 20.0, 1.0,
        )));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));

        if !boot_scene.is_empty() {
            let mut s = scene.borrow_mut();
            if let Err(err) = s.load_from_file(boot_scene) {
                console
                    .borrow_mut()
                    .error(format!("Failed to load scene {}: {}", boot_scene, err));
            }
            nav.borrow_mut().bake(&s);
        }

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
        Ok(Self { world })
    }

    /// Evaluate one command line against the live world, returning the rendered
    /// result or the error message. Drives the full `api::` surface through the one
    /// evaluator; never panics on a bad line.
    pub fn eval(&self, line: &str) -> Result<String, String> {
        let mut c = self.world.console().borrow_mut();
        console::evaluate_line(self.world.script_manager(), &mut c, line)
    }

    /// The live world, for tests / embedders that want to inspect state directly.
    pub fn world(&self) -> &GameWorld {
        &self.world
    }
}

/// One framed response line for an evaluated command.
///
/// Each command yields exactly one JSON object on its own line, so an agent can
/// read responses one-per-line in lockstep with the commands it sent:
/// `{"ok":true,"result":"…"}` or `{"ok":false,"error":"…"}`.
fn response_line(outcome: &Result<String, String>) -> String {
    let value = match outcome {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
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
        writeln!(output, "{}", response_line(&outcome))?;
        output.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new("").expect("session boots in edit mode")
    }

    #[test]
    fn stays_in_edit_mode_with_live_evaluator() {
        let s = session();
        assert!(!s.world().is_playing(), "session must not force play mode");
        assert!(
            s.world().script_manager().is_live(),
            "edit-mode evaluator must be live"
        );
    }

    #[test]
    fn getter_setter_round_trip_through_channel() {
        let s = session();
        // Author an entity into the live edit world, then round-trip a setter and a
        // getter through the same command channel against that live state.
        s.world()
            .scene()
            .borrow_mut()
            .add_entity("Probe".to_string());
        let id = s.eval("Scene.FindEntityByName(\"Probe\")").unwrap();
        assert_ne!(id, "0", "the live scene must resolve the authored entity");

        s.eval(&format!("Transform.SetPosition({}, 1, 2, 3)", id))
            .unwrap();
        let pos = s.eval(&format!("Transform.GetPosition({})", id)).unwrap();
        assert_eq!(pos, "1, 2, 3", "the setter must persist on the live world");
    }

    #[test]
    fn state_persists_across_commands() {
        let s = session();
        s.world()
            .scene()
            .borrow_mut()
            .add_entity("Keep".to_string());
        let id = s.eval("Scene.FindEntityByName(\"Keep\")").unwrap();
        s.eval(&format!("Transform.SetPosition({}, 5, 0, 0)", id))
            .unwrap();
        // A later, independent command still sees the earlier mutation.
        let pos = s.eval(&format!("Transform.GetPosition({})", id)).unwrap();
        assert_eq!(pos, "5, 0, 0");
    }

    #[test]
    fn errors_do_not_kill_the_session() {
        let s = session();
        assert!(s.eval("this is not lua %%%").is_err());
        // The session is still usable after a bad command.
        assert_eq!(s.eval("1 + 1").unwrap(), "2");
    }

    #[test]
    fn run_frames_one_response_per_command() {
        let s = session();
        let input = b"1 + 1\n\nnope %%%\n" as &[u8];
        let mut out = Vec::new();
        run(&s, input, &mut out).expect("channel runs to EOF");
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        // Two non-blank commands -> two response lines (the blank line is skipped).
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"ok\":true") && lines[0].contains("\"result\":\"2\""));
        assert!(lines[1].contains("\"ok\":false") && lines[1].contains("\"error\""));
    }
}
