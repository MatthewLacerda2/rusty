//! Integration coverage for the UI graphics surface (#418): the `Image`,
//! `CanvasGroup` and `RectMask` namespaces write through the shared ops (validation
//! included), and all three components survive a scene save → load.

use std::cell::RefCell;

use glam::Vec4;
use mlua::Lua;
use rusty::components::{
    CanvasGroupComponent, FillMethod, FillOrigin, ImageComponent, ImageType, RectMaskComponent,
};
use rusty::scene::Scene;

/// Run `f` with the three graphics namespaces registered over `scene`.
fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::image::register(&lua, scope, scene).unwrap();
        rusty::api::canvas_group::register(&lua, scope, scene).unwrap();
        rusty::api::rect_mask::register(&lua, scope, scene).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

#[test]
fn image_setters_validate_and_round_trip() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Bar".to_string());
    scene.world.set_image(id, Some(ImageComponent::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "Image.SetColor({id}, 2, 0.5, 0, 0.25)
             Image.SetType({id}, 'filled')
             Image.SetFillMethod({id}, 'Vertical')
             Image.SetFillOrigin({id}, 'Right')
             Image.SetFillAmount({id}, 1.5)
             Image.SetBorder({id}, -1, 2, 3, 4)
             Image.SetTexture({id}, 'ui/bar.png')
             Image.SetType({id}, 'Mesh')"
        ))
        .exec()
        .unwrap();
        let get = format!("local r, g, b, a = Image.GetColor({id}) return r, a");
        let (r, a): (f32, f32) = lua.load(get).eval().unwrap();
        assert_eq!((r, a), (1.0, 0.25));
        let names: (String, String, String) = lua
            .load(format!(
                "return Image.GetType({id}), Image.GetFillMethod({id}), Image.GetFillOrigin({id})"
            ))
            .eval()
            .unwrap();
        assert_eq!(names, ("Filled".into(), "Vertical".into(), "Top".into()));
        let tex: Option<String> = lua
            .load(format!("return Image.GetTexture({id})"))
            .eval()
            .unwrap();
        assert_eq!(tex.as_deref(), Some("ui/bar.png"));
        lua.load(format!("Image.SetTexture({id}, nil)"))
            .exec()
            .unwrap();
        let none: String = lua.load("return Image.GetType(9)").eval().unwrap();
        assert_eq!(none, "None");
    });
    let img = scene.borrow().world.image(id).map(|i| i.clone()).unwrap();
    assert_eq!(img.image_type, ImageType::Filled);
    assert_eq!(img.fill_amount, 1.0);
    assert_eq!(img.border, Vec4::new(0.0, 2.0, 3.0, 4.0));
    assert_eq!(img.texture, None);
}

#[test]
fn group_and_mask_setters_write_through() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Panel".to_string());
    scene
        .world
        .set_canvas_group(id, Some(CanvasGroupComponent::default()));
    scene
        .world
        .set_rect_mask(id, Some(RectMaskComponent::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "CanvasGroup.SetAlpha({id}, 1.5)
             CanvasGroup.SetAlpha({id}, 0.4)
             CanvasGroup.SetInteractable({id}, false)
             RectMask.SetPadding({id}, 1, 2, 3, 4)"
        ))
        .exec()
        .unwrap();
        let (alpha, blocks): (f32, bool) = lua
            .load(format!(
                "return CanvasGroup.GetAlpha({id}), CanvasGroup.GetBlocksRaycasts({id})"
            ))
            .eval()
            .unwrap();
        assert_eq!((alpha, blocks), (0.4, true));
        let t: f32 = lua
            .load(format!(
                "local l, b, r, t = RectMask.GetPadding({id}) return t"
            ))
            .eval()
            .unwrap();
        assert_eq!(t, 4.0);
    });
    assert!(!scene.borrow().world.canvas_group(id).unwrap().interactable);
}

#[test]
fn graphics_components_survive_save_and_load() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Ring".to_string());
    let ring = ImageComponent {
        image_type: ImageType::Filled,
        fill_method: FillMethod::Radial360,
        fill_origin: FillOrigin::Top,
        fill_amount: 0.3,
        ..Default::default()
    };
    scene.world.set_image(id, Some(ring.clone()));
    let group = CanvasGroupComponent {
        alpha: 0.5,
        ..Default::default()
    };
    scene.world.set_canvas_group(id, Some(group.clone()));
    let mask = RectMaskComponent {
        padding: Vec4::splat(2.0),
    };
    scene.world.set_rect_mask(id, Some(mask.clone()));
    let path = crate::temp::dir()
        .join(format!("rusty_ui_graphics_{}.json", std::process::id()))
        .to_string_lossy()
        .into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(loaded.world.image(id).map(|i| i.clone()), Some(ring));
    assert_eq!(
        loaded.world.canvas_group(id).map(|g| g.clone()),
        Some(group)
    );
    assert_eq!(loaded.world.rect_mask(id).map(|m| m.clone()), Some(mask));
}
