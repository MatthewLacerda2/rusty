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
        // Pass the shared console *cell*, not a held borrow: `evaluate_line` borrows
        // it narrowly only at the log sites, leaving it free during eval so a script
        // that calls `print` / `Debug.*` doesn't double-borrow it and panic (#208).
        console::evaluate_line(self.world.script_manager(), self.world.console(), line)
    }

    /// The live world, for tests / embedders that want to inspect state directly.
    pub fn world(&self) -> &GameWorld {
        &self.world
    }
}

/// Resolve the boot scene from a session bin's CLI args, applying `--project` first.
///
/// Shared by the `session` and `session-mcp` bins so they parse arguments
/// identically. The accepted forms are:
///   - `--project <dir>` — `chdir` into a project directory **before** booting, so
///     the relative asset/scene paths the engine reads resolve against it. On a bad
///     directory this prints to stderr and exits 2.
///   - `--empty`         — start from a blank scene (returns an empty boot path).
///   - `<scene-path>`    — boot that specific scene.
///   - none              — boot (and seed) the bundled default scene.
///
/// `--project` may precede any of the scene forms; it is consumed first and the rest
/// is interpreted as the scene argument.
pub fn boot_scene_from_args(args: &[String]) -> String {
    let mut rest = args;
    if let [first, dir, tail @ ..] = rest {
        if first == "--project" {
            if let Err(e) = std::env::set_current_dir(dir) {
                eprintln!("session: failed to enter project dir {dir}: {e}");
                std::process::exit(2);
            }
            rest = tail;
        }
    }

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
/// `{"ok":true,"result":"…"}` or `{"ok":false,"error":"…"}`.
///
/// Shared with the windowed command channel ([`super::command_channel`]) so the
/// headless and windowed responses are byte-identical — the framing lives here once.
pub(crate) fn response_line(outcome: &Result<String, String>) -> String {
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
    fn logging_through_the_real_shared_cell_does_not_panic() {
        // Regression for #208: the session's single console cell is the very one
        // `print` / `Debug.*` write into during eval. The fix evaluates with that
        // cell free, so logging from a command no longer double-borrows and panics.
        let s = session();
        assert_eq!(s.eval("print(\"hello-headless\")").unwrap(), "");
        assert_eq!(s.eval("Debug.Log(\"debug-headless\")").unwrap(), "");
        assert!(s.eval("Debug.Warn(\"warn-headless\")").is_ok());
        assert!(s.eval("Debug.Error(\"err-headless\")").is_ok());

        let log = s.world().console().borrow();
        for expected in [
            "hello-headless",
            "debug-headless",
            "warn-headless",
            "err-headless",
        ] {
            assert!(
                log.messages.iter().any(|(m, _)| m == expected),
                "{expected} must reach the shared console"
            );
        }
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

    #[test]
    fn debug_snapshot_round_trips_authored_state() {
        let s = session();
        // Author + configure an entity purely through the command channel, then read
        // it back via the rich snapshot on the same channel (#180 acceptance).
        let id = s.eval("Scene.CreateEntity(\"Widget\", \"Box\")").unwrap();
        s.eval(&format!("Transform.SetPosition({}, 4, 5, 6)", id))
            .unwrap();

        let snap = s.eval("Debug.Snapshot()").unwrap();
        let world: serde_json::Value = serde_json::from_str(&snap).unwrap();
        assert_eq!(world["play_state"], "editor", "session is edit-mode");

        let entities = world["entities"].as_array().unwrap();
        let widget = entities
            .iter()
            .find(|e| e["name"] == "Widget")
            .expect("authored entity appears in the snapshot");
        assert_eq!(widget["transform"]["pos"][0].as_f64(), Some(4.0));
        assert!(
            widget["components"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c == "Mesh"),
            "the Box primitive's mesh shows in the inventory"
        );
    }
}
