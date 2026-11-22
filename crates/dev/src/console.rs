use std::cell::RefCell;

use rusty_script::scripting::{ConsoleLogs, ScriptManager};

pub fn evaluate_line(
    scripts: &ScriptManager,
    console: &RefCell<ConsoleLogs>,
    line: &str,
) -> Result<String, String> {
    if line.trim().is_empty() {
        return Ok(String::new());
    }

    console.borrow_mut().info(format!("> {}", line));
    // No borrow of `console` is held across `eval`: `print` and `Debug.*` borrow it
    // mid-script, and a held one panics.
    match scripts.eval(line) {
        Ok(result) => {
            if !result.is_empty() {
                console.borrow_mut().info(result.clone());
            }
            Ok(result)
        }
        Err(err) => {
            console.borrow_mut().error(err.clone());
            Err(err)
        }
    }
}

mod repl_input;
pub use repl_input::ReplInput;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use rusty_core::input::InputState;
    use rusty_navigation::NavigationGraph;
    use rusty_scene::Camera;
    use rusty_scene::Scene;
    use rusty_time::Time;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn live_manager() -> (ScriptManager, Rc<RefCell<ConsoleLogs>>) {
        crate::lua_surface::install_api();
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
        let out = evaluate_line(&m, &console, "1 + 2").unwrap();
        assert_eq!(out, "3");
        let log = console.borrow();
        assert_eq!(log.messages.len(), 2);
        assert_eq!(log.messages[0].0, "> 1 + 2");
        assert_eq!(log.messages[1].0, "3");
    }

    #[test]
    fn live_api_call_resolves_against_scene() {
        let (m, console) = live_manager();
        let out = evaluate_line(&m, &console, "Scene.FindEntityByName(\"Player\")").unwrap();
        assert_eq!(out, "1");
    }

    #[test]
    fn statement_line_runs_without_echo() {
        let (m, console) = live_manager();
        let out = evaluate_line(&m, &console, "print(\"hi\")").unwrap();
        assert_eq!(out, "");
        assert!(console.borrow().messages.iter().any(|(m, _)| m == "hi"));
    }

    #[test]
    fn logs_during_eval_do_not_double_borrow() {
        let (m, console) = live_manager();

        let out = evaluate_line(&m, &console, "print(\"from-print\")").unwrap();
        assert_eq!(out, "", "a statement echoes no value");

        let out = evaluate_line(&m, &console, "Debug.Log(\"from-debug-log\")").unwrap();
        assert_eq!(out, "");

        let log = console.borrow();
        assert!(
            log.messages.iter().any(|(m, _)| m == "from-print"),
            "print() must reach the shared console without panicking"
        );
        assert!(
            log.messages.iter().any(|(m, _)| m == "from-debug-log"),
            "Debug.Log() must reach the shared console without panicking"
        );
    }

    #[test]
    fn errors_are_logged_and_returned() {
        let (m, console) = live_manager();
        let err = evaluate_line(&m, &console, "this is not lua %%%").unwrap_err();
        assert!(!err.is_empty());
        assert!(console
            .borrow()
            .messages
            .iter()
            .any(|(_, lvl)| *lvl == rusty_script::scripting::LogLevel::Error));
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
            Rc::clone(&console),
            Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
            Rc::new(RefCell::new(Time::new())),
        );
        assert!(!m.is_live());
        assert!(evaluate_line(&m, &console, "1 + 1").is_err());
    }
}
