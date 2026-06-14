//! src/dev/console.rs — Console: log output + live Lua REPL
//!
//! Two responsibilities behind one type:
//!   1. Log sink — buffers info/warn/error lines (exists today as ConsoleLogs).
//!   2. REPL     — evaluates a line of Lua against the LIVE runtime and returns the
//!                 result. The SAME evaluator backs both the in-editor terminal panel
//!                 (type while the game plays) and the headless harness (scenario
//!                 lines). There is exactly one evaluator, so windowed and headless
//!                 can never drift.
//!
//! "Call the API live" == feed a string to this evaluator.
//!
//! The single evaluator lives on `ScriptManager::eval` (it owns the live mlua
//! state). This module is the thin, UI-agnostic glue: it takes one input line,
//! runs it through that evaluator, and writes the echoed prompt plus the result
//! (or error) into the shared `ConsoleLogs` buffer. Both the editor input line
//! and the harness call `evaluate_line`, so they share byte-for-byte behaviour.
//!
//! Allowed deps: scripting (mlua), api.

use crate::scripting::{ConsoleLogs, ScriptManager};

/// Evaluate one REPL line against the live runtime and log the outcome.
///
/// Echoes the line as `> <line>`, then logs either the result value (info) or the
/// error (error level). Returns the raw `Ok(result)` / `Err(message)` so callers
/// (the harness) can assert on it without scraping the log buffer. Blank lines are
/// ignored and produce no log noise.
pub fn evaluate_line(
    scripts: &ScriptManager,
    console: &mut ConsoleLogs,
    line: &str,
) -> Result<String, String> {
    if line.trim().is_empty() {
        return Ok(String::new());
    }

    console.info(format!("> {}", line));
    match scripts.eval(line) {
        Ok(result) => {
            if !result.is_empty() {
                console.info(result.clone());
            }
            Ok(result)
        }
        Err(err) => {
            console.error(err.clone());
            Err(err)
        }
    }
}

/// A single line of REPL input plus a small history, owned by whatever UI hosts
/// the console (the editor's bottom panel). Kept separate from `ConsoleLogs` so
/// the log buffer stays a pure sink with no UI state.
#[derive(Default)]
pub struct ReplInput {
    /// The text currently being edited in the input line.
    pub buffer: String,
    /// Previously submitted lines, newest last, for up/down recall.
    history: Vec<String>,
}

impl ReplInput {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the current buffer, push it onto history, and clear the buffer.
    /// Returns `None` if the buffer is blank.
    pub fn take_submit(&mut self) -> Option<String> {
        let line = std::mem::take(&mut self.buffer);
        if line.trim().is_empty() {
            return None;
        }
        self.history.push(line.clone());
        Some(line)
    }

    pub fn history(&self) -> &[String] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::input::InputState;
    use crate::core::scene::Scene;
    use crate::navigation::NavigationGraph;
    use crate::render::Camera;
    use crate::time::Time;
    use glam::Vec3;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn live_manager() -> (ScriptManager, Rc<RefCell<ConsoleLogs>>) {
        let mut raw = Scene::new();
        raw.add_entity("Player".to_string());
        let scene = Rc::new(RefCell::new(raw));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));
        let mut m = ScriptManager::new(
            Rc::clone(&scene),
            Rc::new(RefCell::new(InputState::new())),
            Rc::new(RefCell::new(NavigationGraph::new(
                -5.0, 5.0, -5.0, 5.0, 1.0,
            ))),
            Rc::clone(&console),
            Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
            Rc::new(RefCell::new(Time::new())),
        );
        m.init_runtime().expect("runtime inits");
        (m, console)
    }

    #[test]
    fn expression_line_echoes_value() {
        let (m, console) = live_manager();
        let mut logs = ConsoleLogs::new();
        let out = evaluate_line(&m, &mut logs, "1 + 2").unwrap();
        assert_eq!(out, "3");
        // prompt echo + result line
        assert_eq!(logs.messages.len(), 2);
        assert_eq!(logs.messages[0].0, "> 1 + 2");
        assert_eq!(logs.messages[1].0, "3");
        let _ = console;
    }

    #[test]
    fn live_api_call_resolves_against_scene() {
        let (m, _console) = live_manager();
        let mut logs = ConsoleLogs::new();
        // Player is entity 1; FindEntityByName must hit the live scene.
        let out = evaluate_line(&m, &mut logs, "Scene.FindEntityByName(\"Player\")").unwrap();
        assert_eq!(out, "1");
    }

    #[test]
    fn statement_line_runs_without_echo() {
        let (m, console) = live_manager();
        let mut logs = ConsoleLogs::new();
        // A statement has no return value, so the REPL echoes nothing. print()
        // routes through the ScriptManager's own console sink (the same shared
        // buffer in production), not the evaluator's return value.
        let out = evaluate_line(&m, &mut logs, "print(\"hi\")").unwrap();
        assert_eq!(out, "");
        assert!(console.borrow().messages.iter().any(|(m, _)| m == "hi"));
    }

    #[test]
    fn errors_are_logged_and_returned() {
        let (m, _console) = live_manager();
        let mut logs = ConsoleLogs::new();
        let err = evaluate_line(&m, &mut logs, "this is not lua %%%").unwrap_err();
        assert!(!err.is_empty());
        assert!(logs
            .messages
            .iter()
            .any(|(_, lvl)| *lvl == crate::scripting::LogLevel::Error));
    }

    #[test]
    fn eval_without_runtime_reports_not_live() {
        let scene = Rc::new(RefCell::new(Scene::new()));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));
        let m = ScriptManager::new(
            Rc::clone(&scene),
            Rc::new(RefCell::new(InputState::new())),
            Rc::new(RefCell::new(NavigationGraph::new(
                -5.0, 5.0, -5.0, 5.0, 1.0,
            ))),
            console,
            Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
            Rc::new(RefCell::new(Time::new())),
        );
        assert!(!m.is_live());
        let mut logs = ConsoleLogs::new();
        assert!(evaluate_line(&m, &mut logs, "1 + 1").is_err());
    }

    #[test]
    fn repl_input_take_submit_records_history() {
        let mut input = ReplInput::new();
        assert!(input.take_submit().is_none());
        input.buffer = "  ".to_string();
        assert!(input.take_submit().is_none());
        input.buffer = "print(1)".to_string();
        assert_eq!(input.take_submit().as_deref(), Some("print(1)"));
        assert!(input.buffer.is_empty());
        assert_eq!(input.history(), &["print(1)".to_string()]);
    }
}
