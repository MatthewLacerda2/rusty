//! src/dev/console.rs — Console: log output + live Lua REPL
//!
//! Two responsibilities behind one type:
//! 1. Log sink — buffers info/warn/error lines (exists today as ConsoleLogs).
//! 2. REPL — evaluates a line of Lua against the LIVE runtime and returns the
//!    result. The SAME evaluator backs both the in-editor terminal panel
//!    (type while the game plays) and the headless harness (scenario lines).
//!    There is exactly one evaluator, so windowed and headless can never drift.
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

use crate::scripting::{ScriptCtx, ScriptManager};

/// Evaluate one REPL line against the live runtime and log the outcome.
///
/// Echoes the line as `> <line>`, then logs either the result value (info) or the
/// error (error level). Returns the raw `Ok(result)` / `Err(message)` so callers
/// (the harness) can assert on it without scraping the log buffer. Blank lines are
/// ignored and produce no log noise.
///
/// The console it logs to is the SAME `ctx.console` the evaluator binds inside its
/// scope (#57), so the prompt echo and result land in the one live console buffer.
pub fn evaluate_line(
    scripts: &ScriptManager,
    ctx: &mut ScriptCtx,
    line: &str,
) -> Result<String, String> {
    if line.trim().is_empty() {
        return Ok(String::new());
    }

    ctx.console.info(format!("> {}", line));
    match scripts.eval(ctx, line) {
        Ok(result) => {
            if !result.is_empty() {
                ctx.console.info(result.clone());
            }
            Ok(result)
        }
        Err(err) => {
            ctx.console.error(err.clone());
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
    use crate::navigation::NavigationGraph;
    use crate::render::Camera;
    use crate::scene::Scene;
    use crate::scripting::ConsoleLogs;
    use crate::time::Time;
    use glam::Vec3;

    /// The owned engine state a REPL test borrows into a `ScriptCtx`.
    struct Env {
        scene: Scene,
        input: InputState,
        nav: NavigationGraph,
        console: ConsoleLogs,
        camera: Camera,
        time: Time,
        physics: Option<crate::physics::PhysicsWorld>,
    }

    impl Env {
        fn new() -> Self {
            Self {
                scene: Scene::new(),
                input: InputState::new(),
                nav: NavigationGraph::new(-5.0, 5.0, -5.0, 5.0, 1.0),
                console: ConsoleLogs::new(),
                camera: Camera::new(Vec3::ZERO, 0.0, 0.0),
                time: Time::new(),
                physics: None,
            }
        }

        fn ctx(&mut self) -> ScriptCtx<'_> {
            ScriptCtx {
                scene: &mut self.scene,
                input: &mut self.input,
                nav: &mut self.nav,
                console: &mut self.console,
                camera: &mut self.camera,
                time: &mut self.time,
                physics: &mut self.physics,
            }
        }
    }

    fn live_manager() -> (ScriptManager, Env) {
        let mut env = Env::new();
        env.scene.add_entity("Player".to_string());
        let mut m = ScriptManager::new();
        m.init_runtime().expect("runtime inits");
        (m, env)
    }

    #[test]
    fn expression_line_echoes_value() {
        let (m, mut env) = live_manager();
        let out = evaluate_line(&m, &mut env.ctx(), "1 + 2").unwrap();
        assert_eq!(out, "3");
        // prompt echo + result line
        assert_eq!(env.console.messages.len(), 2);
        assert_eq!(env.console.messages[0].0, "> 1 + 2");
        assert_eq!(env.console.messages[1].0, "3");
    }

    #[test]
    fn live_api_call_resolves_against_scene() {
        let (m, mut env) = live_manager();
        // Player is entity 1; FindEntityByName must hit the live scene.
        let out = evaluate_line(&m, &mut env.ctx(), "Scene.FindEntityByName(\"Player\")").unwrap();
        assert_eq!(out, "1");
    }

    #[test]
    fn statement_line_runs_without_echo() {
        let (m, mut env) = live_manager();
        // A statement has no return value, so the REPL echoes nothing. print()
        // routes through the borrowed console sink (the same buffer the prompt
        // echo lands in), not the evaluator's return value.
        let out = evaluate_line(&m, &mut env.ctx(), "print(\"hi\")").unwrap();
        assert_eq!(out, "");
        assert!(env.console.messages.iter().any(|(m, _)| m == "hi"));
    }

    #[test]
    fn errors_are_logged_and_returned() {
        let (m, mut env) = live_manager();
        let err = evaluate_line(&m, &mut env.ctx(), "this is not lua %%%").unwrap_err();
        assert!(!err.is_empty());
        assert!(env
            .console
            .messages
            .iter()
            .any(|(_, lvl)| *lvl == crate::scripting::LogLevel::Error));
    }

    #[test]
    fn eval_without_runtime_reports_not_live() {
        let m = ScriptManager::new();
        let mut env = Env::new();
        assert!(!m.is_live());
        assert!(evaluate_line(&m, &mut env.ctx(), "1 + 1").is_err());
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
