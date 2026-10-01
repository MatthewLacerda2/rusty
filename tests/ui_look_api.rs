//! The UI look surface (#425): `docs/ui.md`'s "Cyberpunk panel" recipe runs as
//! written through `Scene`, `RectTransform` and `Shape`, the setters write through
//! the shared ops (validation included), `Image` / `Text` take a blend and the
//! gradient table round-trips, and the shapes survive a scene save → load.

use std::cell::RefCell;

use glam::Vec4;
use mlua::Lua;
use rusty::components::{
    CanvasComponent, ImageComponent, ShapeCorner, ShapeKind, TextComponent, UiBlend,
};
use rusty::scene::Scene;

/// The recipe from `docs/ui.md` (kept verbatim; `hud` is a canvas).
const RECIPE: &str = include_str!("ui_look_recipe.lua");

/// Run `f` with the namespaces the recipe uses registered over `scene`.
fn with_api(scene: &RefCell<Scene>, f: impl FnOnce(&Lua)) {
    let (path, playing) = (RefCell::new(None), RefCell::new(false));
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::scene::register(&lua, scope, scene, &path, &playing).unwrap();
        rusty::api::rect_transform::register(&lua, scope, scene).unwrap();
        rusty::api::shape::register(&lua, scope, scene).unwrap();
        rusty::api::image::register(&lua, scope, scene).unwrap();
        f(&lua);
        Ok(())
    })
    .unwrap();
}

#[test]
fn the_cyberpunk_recipe_builds_its_panel() {
    let mut scene = Scene::new();
    let hud = scene.add_entity("Hud".to_string());
    scene
        .world
        .set_canvas(hud, Some(CanvasComponent::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.globals().set("hud", hud).unwrap();
        lua.load(RECIPE).exec().expect("the recipe runs");
        let (w, r, g, b, a): (f32, f32, f32, f32, f32) = lua
            .load("return Shape.GetBorder(Scene.FindEntityByName('Panel'))")
            .eval()
            .unwrap();
        assert_eq!((w, r, g, b, a), (2.0, 0.0, 0.95, 1.0, 1.0));
        let stops: usize = lua
            .load("return #Shape.GetGradient(Scene.FindEntityByName('Header')).stops")
            .eval()
            .unwrap();
        assert_eq!(stops, 2, "the gradient table round-trips");
    });
    let scene = scene.borrow();
    let find = |name: &str| scene.world.find_by_name(name).expect(name);
    let panel = scene.world.shape(find("Panel")).unwrap().clone();
    assert_eq!(panel.corner, ShapeCorner::Chamfer);
    assert_eq!(panel.radius, Vec4::new(24.0, 0.0, 24.0, 0.0));
    assert!(panel.glow.is_on() && panel.shadow.is_on());
    let scan = scene.world.shape(find("ScanLine")).unwrap().clone();
    assert_eq!(
        (scan.kind, scan.blend),
        (ShapeKind::Line, UiBlend::Additive)
    );
    assert_eq!((scan.dash, scan.gap), (12.0, 6.0));
}

#[test]
fn setters_validate_and_blend_and_gradient_reach_image_and_text() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Bar".to_string());
    scene.world.set_image(id, Some(ImageComponent::default()));
    scene.world.set_text(id, Some(TextComponent::default()));
    scene.world.set_shape(id, Some(Default::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "Shape.SetRadius({id}, -4, 2, 3, 4)
             Shape.SetKind({id}, 'Star')
             Shape.SetBlend({id}, 'screen')
             Image.SetBlend({id}, 'Multiply')
             Image.SetGradient({id}, {{ kind = 'Radial', stops = {{
                 {{ t = 1, color = {{ 0, 0, 1, 1 }} }}, {{ t = 0, color = {{ 2, 0, 0, 1 }} }} }} }})"
        ))
        .exec()
        .unwrap();
        let err = lua.load(format!("Shape.SetGlow({id}, 4)")).exec();
        assert!(err.is_err(), "a short number list is an error");
        let none: String = lua.load("return Shape.GetBlend(999)").eval().unwrap();
        assert_eq!(none, "None");
    });
    let s = scene.borrow();
    let shape = s.world.shape(id).unwrap().clone();
    assert_eq!(shape.radius, Vec4::new(0.0, 2.0, 3.0, 4.0));
    assert_eq!(
        (shape.kind, shape.blend),
        (ShapeKind::Rect, UiBlend::Screen)
    );
    let image = s.world.image(id).unwrap().clone();
    assert_eq!(image.blend, UiBlend::Multiply);
    let g = image.gradient.expect("gradient");
    assert_eq!(g.stops[0].t, 0.0, "stops are sorted");
    assert_eq!(
        g.stops[0].color,
        Vec4::new(1.0, 0.0, 0.0, 1.0),
        "and clamped"
    );
}

#[test]
fn shapes_and_looks_survive_save_and_load() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Ring".to_string());
    let mut shape = rusty::components::ShapeComponent {
        kind: ShapeKind::Ring,
        inner_radius: 12.0,
        blend: UiBlend::Additive,
        ..Default::default()
    };
    shape.glow.size = 6.0;
    scene.world.set_shape(id, Some(shape.clone()));
    let image = ImageComponent {
        blend: UiBlend::Screen,
        gradient: Some(Default::default()),
        ..Default::default()
    };
    scene.world.set_image(id, Some(image.clone()));
    let path = crate::temp::dir().join("rusty_425_look.scene.json");
    scene.save_to_file(path.to_str().unwrap()).expect("save");
    let mut loaded = Scene::new();
    loaded.load_from_file(path.to_str().unwrap()).expect("load");
    assert_eq!(loaded.world.shape(id).map(|s| s.clone()), Some(shape));
    assert_eq!(loaded.world.image(id).map(|i| i.clone()), Some(image));
}
