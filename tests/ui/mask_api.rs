//! Integration coverage for the soft/shaped mask and backdrop surface (#426, #428):
//! `RectMask.SetFeather`, the `Mask` and `BackdropFilter` namespaces write through
//! the shared ops (validation included), `Scene.AddComponent` names both new kinds,
//! and every field survives a scene save → load.

use std::cell::RefCell;

use glam::Vec4;
use mlua::Lua;
use rusty::components::{BackdropFilterComponent, MaskComponent, RectMaskComponent};
use rusty::scene::authoring::{add_component, ComponentKind};
use rusty::scene::Scene;

/// Run `f` with the three namespaces registered over `scene`.
fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::rect_mask::register(&lua, scope, scene).unwrap();
        rusty::api::mask::register(&lua, scope, scene).unwrap();
        rusty::api::backdrop_filter::register(&lua, scope, scene).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

#[test]
fn setters_validate_and_getters_read_back() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Panel".to_string());
    for kind in ["RectMask", "Mask", "Backdrop"] {
        let kind = ComponentKind::parse(kind).expect("a component kind");
        assert!(add_component(&mut scene, id, kind));
    }
    assert!(scene.world.has_rect_transform(id), "each requires a rect");
    let bare = scene.add_entity("Bare".to_string());
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "RectMask.SetFeather({id}, -4)
             Mask.SetShowMaskGraphic({id}, false)
             BackdropFilter.SetBlurRadius({id}, 24)
             BackdropFilter.SetTint({id}, 2, 0.5, 0, 0.25)
             BackdropFilter.SetSaturation({id}, -1)
             BackdropFilter.SetBrightness({id}, 0.5)
             Mask.SetShowMaskGraphic({bare}, true)"
        ))
        .exec()
        .unwrap();
        let read = format!(
            "local r, g, b, a = BackdropFilter.GetTint({id})
             return RectMask.GetFeather({id}), Mask.GetShowMaskGraphic({id}),
                    BackdropFilter.GetBlurRadius({id}), r, a,
                    BackdropFilter.GetSaturation({id}), BackdropFilter.GetBrightness({id})"
        );
        let got: (f32, bool, f32, f32, f32, f32, f32) = lua.load(read).eval().unwrap();
        assert_eq!(got, (0.0, false, 24.0, 1.0, 0.25, 0.0, 0.5));
        // Without the component: defaults, and the setter was a no-op.
        let none =
            format!("return Mask.GetShowMaskGraphic({bare}), BackdropFilter.GetBlurRadius({bare})");
        let (shown, radius): (bool, f32) = lua.load(none).eval().unwrap();
        assert_eq!((shown, radius), (false, 16.0));
    });
    assert!(!scene.borrow().world.has_mask(bare));
}

#[test]
fn masks_and_backdrops_survive_save_and_load() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Panel".to_string());
    let rect = RectMaskComponent {
        padding: Vec4::ZERO,
        feather: 6.0,
    };
    scene.world.set_rect_mask(id, Some(rect.clone()));
    let mask = MaskComponent {
        show_mask_graphic: false,
    };
    scene.world.set_mask(id, Some(mask.clone()));
    let glass = BackdropFilterComponent {
        blur_radius: 30.0,
        tint: Vec4::new(0.1, 0.2, 0.3, 0.4),
        saturation: 0.5,
        brightness: 0.8,
    };
    scene.world.set_backdrop_filter(id, Some(glass.clone()));
    let path = crate::temp::dir()
        .join("rusty_ui_mask_api.json")
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(loaded.world.rect_mask(id).map(|m| m.clone()), Some(rect));
    assert_eq!(loaded.world.mask(id).map(|m| m.clone()), Some(mask));
    let back = loaded.world.backdrop_filter(id).map(|b| b.clone());
    assert_eq!(back, Some(glass));
}
