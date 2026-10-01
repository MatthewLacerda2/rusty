//! `RectTransform.SetAnchorPreset` (#423): the inspector's anchor-preset grid from
//! Lua — the rect stays put on a plain preset, Alt-style `setPosition` snaps it,
//! and an unknown name is an error.

use std::cell::RefCell;

use mlua::Table;

use super::ui_api::{ui_scene, with_ui_api};

/// `UI.GetRect(id)`'s `x, y, width, height`.
fn rect(lua: &mlua::Lua, id: u32) -> [f32; 4] {
    let r: Table = lua.load(format!("return UI.GetRect({id})")).eval().unwrap();
    ["x", "y", "width", "height"].map(|k| r.get::<_, f32>(k).unwrap())
}

#[test]
fn a_preset_reanchors_in_place_and_set_position_snaps() {
    let (scene, _, child) = ui_scene();
    let scene = RefCell::new(scene);
    with_ui_api(&scene, |lua| {
        let before = rect(lua, child);
        lua.load(format!(
            "RectTransform.SetAnchorPreset({child}, 'right', 'top', true)"
        ))
        .exec()
        .unwrap();
        assert_eq!(rect(lua, child), before, "a plain preset keeps the rect");
        let pivot: (f32, f32) = lua
            .load(format!("return RectTransform.GetPivot({child})"))
            .eval()
            .unwrap();
        assert_eq!(pivot, (1.0, 1.0));
        lua.load(format!(
            "RectTransform.SetAnchorPreset({child}, 'stretch', 'bottom', false, true)"
        ))
        .exec()
        .unwrap();
        let [x, y, w, h] = rect(lua, child);
        assert_eq!((x, w), (0.0, 1920.0), "stretched across the parent");
        assert_eq!(
            (y + h, h),
            (0.0, before[3]),
            "hung below the bottom by its top pivot"
        );
        let bad = lua
            .load(format!(
                "RectTransform.SetAnchorPreset({child}, 'top', 'left')"
            ))
            .exec();
        assert!(bad.is_err(), "x takes left/center/right/stretch");
    });
}
