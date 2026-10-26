//! The player graphics options (#1438): `Graphics.Get/SetOption` write the live
//! cell the renderer reads, `ResetOptions` returns to the project's defaults
//! (`Get/SetDefaultOption`, on `Application`), `GetOptionNames` lists every
//! switch, and an unknown name is an error naming the valid ones.

use std::cell::RefCell;

use mlua::Lua;
use rusty::core::application::Application;
use rusty::core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty::core::settings::GraphicsSettings;
use rusty::scene::Scene;

/// Run `f` with a `Graphics` namespace over fresh cells.
fn with_graphics(f: impl FnOnce(&Lua, &RefCell<GraphicsSettings>, &RefCell<Application>)) {
    let (lua, scene) = (Lua::new(), RefCell::new(Scene::new()));
    let (graphics, app) = (RefCell::default(), RefCell::new(Application::new()));
    lua.scope(|scope| {
        rusty::api::graphics::register(&lua, scope, &scene, &graphics, &app).unwrap();
        f(&lua, &graphics, &app);
        Ok(())
    })
    .unwrap();
}

#[test]
fn set_option_writes_the_live_cell_and_reads_back() {
    with_graphics(|lua, graphics, _| {
        let on: bool = lua
            .load("return Graphics.GetOption('motion_blur')")
            .eval()
            .unwrap();
        assert!(on, "every option defaults to allowed");
        lua.load("Graphics.SetOption('motion_blur', false)")
            .exec()
            .unwrap();
        assert!(!graphics.borrow().options.allows(GraphicsOption::MotionBlur));
        let on: bool = lua
            .load("return Graphics.GetOption('motion_blur')")
            .eval()
            .unwrap();
        assert!(!on);
    });
}

#[test]
fn reset_options_returns_to_the_project_defaults() {
    with_graphics(|lua, graphics, app| {
        lua.load("Graphics.SetDefaultOption('bloom', false)")
            .exec()
            .unwrap();
        assert!(!app
            .borrow()
            .graphics_defaults()
            .allows(GraphicsOption::Bloom));
        let live: bool = lua
            .load("return Graphics.GetOption('bloom')")
            .eval()
            .unwrap();
        assert!(!live, "an option the player never set follows its default");
        lua.load("Graphics.SetOption('fxaa', false); Graphics.SetDefaultOption('fxaa', true)")
            .exec()
            .unwrap();
        let fxaa: bool = lua
            .load("return Graphics.GetOption('fxaa')")
            .eval()
            .unwrap();
        assert!(!fxaa, "a default never overrides the player's choice");
        lua.load("Graphics.ResetOptions()").exec().unwrap();
        let mut expected = GraphicsOptions::default();
        expected.set(GraphicsOption::Bloom, false);
        assert_eq!(graphics.borrow().options, expected);
        let default: bool = lua
            .load("return Graphics.GetDefaultOption('bloom')")
            .eval()
            .unwrap();
        assert!(!default);
    });
}

#[test]
fn option_names_list_every_switch() {
    with_graphics(|lua, _, _| {
        let names: Vec<String> = lua.load("return Graphics.GetOptionNames()").eval().unwrap();
        let all: Vec<_> = GraphicsOption::ALL.iter().map(|o| o.name()).collect();
        assert_eq!(names, all);
    });
}

#[test]
fn an_unknown_name_is_an_error_listing_the_valid_ones() {
    with_graphics(|lua, _, _| {
        for call in [
            "Graphics.GetOption('warp')",
            "Graphics.SetOption('warp', true)",
            "Graphics.SetDefaultOption('warp', true)",
        ] {
            let err = lua.load(call).exec().unwrap_err().to_string();
            assert!(err.contains("unknown option 'warp'"), "{call}: {err}");
            assert!(err.contains("fxaa, motion_blur"), "{call}: {err}");
        }
    });
}

/// Every option is documented in `docs/api/Graphics.md`'s *Player options* table.
#[test]
fn every_option_is_documented() {
    let doc = std::fs::read_to_string("docs/api/Graphics.md").unwrap();
    for option in GraphicsOption::ALL {
        let row = format!("| `{}` |", option.name());
        assert!(doc.contains(&row), "docs/api/Graphics.md lacks a {row} row");
    }
}
