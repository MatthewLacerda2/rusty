//! The quality presets (#1522): `Graphics.SetQuality` selects one of the project's
//! presets by name (an unknown one refused with the list) and puts its values in
//! the graphics cell the renderer reads; `QualityPresets` lists them; and
//! `SetQualityPreset` / `RemoveQualityPreset` author them.

use std::cell::RefCell;

use mlua::Lua;
use rusty::core::application::Application;
use rusty::core::settings::quality::QualityPresets;
use rusty::core::settings::GraphicsSettings;
use rusty::scene::Scene;

/// Run `f` with the `Graphics` namespace over a fresh cell and a seeded project.
fn with_graphics(f: impl FnOnce(&Lua, &RefCell<GraphicsSettings>, &RefCell<Application>)) {
    let (lua, scene) = (Lua::new(), RefCell::new(Scene::new()));
    let (graphics, app) = (RefCell::default(), RefCell::default());
    lua.scope(|scope| {
        rusty::api::graphics::register(&lua, scope, &scene, &graphics, &app).unwrap();
        f(&lua, &graphics, &app);
        Ok(())
    })
    .unwrap();
}

fn seeded(name: &str) -> rusty::core::settings::quality::QualityValues {
    QualityPresets::default().get(name).unwrap().values
}

#[test]
fn set_quality_selects_a_preset_and_refuses_an_unknown_one() {
    with_graphics(|lua, graphics, _| {
        let name = || lua.load("return Graphics.GetQuality()").eval::<String>();
        assert_eq!(name().unwrap(), "Medium", "the seeded default");
        lua.load(r#"Graphics.SetQuality("high")"#).exec().unwrap();
        assert_eq!(name().unwrap(), "High");
        assert_eq!(graphics.borrow().quality, seeded("High"));
        let err = lua.load(r#"Graphics.SetQuality("Ultra")"#).exec();
        let err = err.unwrap_err().to_string();
        assert!(
            err.contains("'Ultra'") && err.contains("Low, Medium, High"),
            "{err}"
        );
        assert_eq!(name().unwrap(), "High", "a refusal keeps the selection");
    });
}

#[test]
fn presets_are_listed_and_authored() {
    with_graphics(|lua, graphics, app| {
        let count = || lua.load("return #Graphics.QualityPresets()").eval::<u32>();
        assert_eq!(count().unwrap(), 3);
        let low: u32 = lua
            .load("return Graphics.QualityPresets()[1].ssao.medium.samples")
            .eval()
            .unwrap();
        assert_eq!(low, seeded("Low").ssao.medium.samples);
        lua.load(r#"Graphics.SetQualityPreset("Ultra", { ssao = { medium = { samples = 48 } } })"#)
            .exec()
            .unwrap();
        lua.load(r#"Graphics.SetQuality("Ultra")"#).exec().unwrap();
        assert_eq!(graphics.borrow().quality.ssao.medium.samples, 48);
        let typo = lua
            .load(r#"Graphics.SetQualityPreset("Ultra", { sao = {} })"#)
            .exec();
        assert!(typo.unwrap_err().to_string().contains("'sao'"));
        lua.load(r#"Graphics.RemoveQualityPreset("Ultra")"#)
            .exec()
            .unwrap();
        assert_eq!(
            app.borrow().quality(),
            Some("Medium"),
            "back to the initial one"
        );
        assert_eq!(graphics.borrow().quality, seeded("Medium"));
        let gone = lua.load(r#"Graphics.RemoveQualityPreset("Ultra")"#).exec();
        assert!(gone.is_err(), "an unknown preset is refused");
    });
}

/// A listed preset edits back in whole (its `name` key included), but not under
/// another preset's name.
#[test]
fn a_listed_preset_round_trips_into_set_quality_preset() {
    with_graphics(|lua, _, app| {
        lua.load(
            "local p = Graphics.QualityPresets()[1]
             p.ssao.low.samples = 5
             Graphics.SetQualityPreset(p.name, p)",
        )
        .exec()
        .unwrap();
        let low = app.borrow().quality_presets().get("Low").unwrap().values;
        assert_eq!(low.ssao.low.samples, 5);
        assert_eq!(low.ssr, seeded("Low").ssr, "the rest is unchanged");
        let renamed = lua
            .load("Graphics.SetQualityPreset('High', Graphics.QualityPresets()[1])")
            .exec();
        assert!(renamed.unwrap_err().to_string().contains("'High'"));
    });
}

/// `SetDefaultQuality` names the preset a first launch gets; `nil` clears it (the
/// middle one), and an unknown name is refused.
#[test]
fn the_default_preset_is_read_and_written() {
    with_graphics(|lua, _, app| {
        let get = || {
            lua.load("return Graphics.GetDefaultQuality()")
                .eval::<Option<String>>()
        };
        assert_eq!(get().unwrap().as_deref(), Some("Medium"));
        lua.load("Graphics.SetDefaultQuality('high')")
            .exec()
            .unwrap();
        assert_eq!(get().unwrap().as_deref(), Some("High"));
        assert_eq!(
            app.borrow().quality(),
            Some("High"),
            "no choice: the default runs"
        );
        lua.load("Graphics.SetDefaultQuality(nil)").exec().unwrap();
        assert_eq!(get().unwrap(), None);
        let err = lua.load("Graphics.SetDefaultQuality('Ultra')").exec();
        assert!(err.unwrap_err().to_string().contains("Low, Medium, High"));
    });
}
