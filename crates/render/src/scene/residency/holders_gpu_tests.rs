//! crates/render/src/scene/residency/holders_gpu_tests.rs — each holder of a cached
//! upload keeps its map through a release that drops another scene's (#2458): a UI
//! image, a sprite particle's texture, the sky's panorama and a decal's map of the
//! edit scene a Play holds, and the hidden UI of the scene a load drew.

use glam::{Vec2, Vec3};

use super::super::fixture::frame;
use super::maps_gpu_tests::{maps, png, textured};
use rusty_asset::MapRef;
use rusty_components::particle::{Particle, ParticleEmitterComponent};
use rusty_components::{CanvasComponent, ImageComponent, RectTransformComponent};
use rusty_scene::decal::DecalSpec;
use rusty_scene::Scene;
use rusty_test_support::panorama::fixture as panorama;

/// An overlay canvas with an Image of `map`; the image's id.
pub(super) fn image(scene: &mut Scene, map: &MapRef) -> u32 {
    let root = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(root, Some(CanvasComponent::default()));
    let id = scene.add_entity("Image".to_string());
    let rt = RectTransformComponent {
        size_delta: Vec2::splat(8.0),
        ..RectTransformComponent::default()
    };
    scene.world.set_rect_transform(id, Some(rt));
    let image = ImageComponent {
        texture: Some(map.clone()),
        ..ImageComponent::default()
    };
    scene.world.set_image(id, Some(image));
    scene.set_parent(id, Some(root)).expect("the canvas exists");
    id
}

/// A sprite emitter of `map` holding one live particle in view.
fn sprites(scene: &mut Scene, map: &MapRef) {
    let id = scene.add_entity("Smoke".to_string());
    let mut emitter = ParticleEmitterComponent {
        texture: Some(map.clone()),
        ..Default::default()
    };
    let at = Vec3::new(0.0, 0.5, -4.0);
    emitter.runtime.particles = vec![Particle::at(at, 0.5, [1.0; 4], 99.0)];
    scene.world.set_particles(id, Some(emitter));
}

#[test]
fn gpu_the_held_edit_scenes_ui_particles_sky_and_decal_keep_their_maps() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (box_map, ui, smoke) = (
        png("box", [90; 3]),
        png("ui", [200, 0, 0]),
        png("smoke", [0; 3]),
    );
    let decal = png("decal", [0, 200, 0]);
    let mut edit = textured(&[&box_map]);
    image(&mut edit, &ui);
    sprites(&mut edit, &smoke);
    edit.skybox_path = panorama("sky.png", [60, 100, 220], [160, 110, 60], 40);
    let spec = DecalSpec {
        texture: Some(decal.clone()),
        size: 4.0,
        depth: 4.0,
        ..DecalSpec::default()
    };
    edit.spawn_decal(Vec3::new(0.0, 0.0, -4.0), Vec3::Z, spec);
    frame(&mut r, &edit);
    let drawn = maps(&r);
    assert_eq!(drawn, ["box.png", "sky.png", "smoke.png", "ui.png"]);
    edit.set_playing(true);
    frame(&mut r, &edit);
    let mut level = textured(&[&png("level", [0, 0, 200])]);
    level.set_playing(true);
    frame(&mut r, &level);
    let mut held = maps(&r);
    held.retain(|m| m != "level.png");
    assert_eq!(held, drawn, "the held scene's holders keep their maps");
    edit.set_playing(false);
    let (_, stop) = frame(&mut r, &edit);
    assert_eq!(stop.uploads.count, 0, "Stop reads nothing again");
    assert_eq!(maps(&r), drawn, "the level's map is released");
    let layer = r.decals.atlas.layer(decal.as_str());
    assert_ne!(
        layer,
        u32::MAX,
        "the decal's map still has its layer (not `NO_LAYER`)"
    );
}

#[test]
fn gpu_a_hidden_image_of_the_scene_a_load_drew_keeps_its_map() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let (menu, a) = (png("menu", [200, 0, 0]), png("a", [0, 0, 200]));
    let mut first = textured(&[&a]);
    image(&mut first, &menu);
    frame(&mut r, &first);
    // The next scene's menu is hidden: its image is laid out by no frame yet.
    let mut next = textured(&[]);
    let hidden = image(&mut next, &menu);
    next.world.set_active(hidden, false);
    frame(&mut r, &next);
    assert_eq!(maps(&r), ["menu.png"], "a's map goes; the menu's stays");
    next.world.set_active(hidden, true);
    let (_, shown) = frame(&mut r, &next);
    assert_eq!(shown.uploads.count, 0, "showing it reads nothing again");
}

#[test]
fn gpu_a_load_drops_the_lightmap_lists_no_kept_scene_binds() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    // Pages that fail to load are cached all the same: one list each.
    let lit = |page: &str| {
        let mut scene = textured(&[]);
        scene.lightmaps.pages = vec![format!("{page}.png")];
        scene
    };
    let names = |r: &crate::Renderer| r.lightmaps.cached_lists().concat();
    let mut edit = lit("edit");
    frame(&mut r, &edit);
    edit.set_playing(true);
    frame(&mut r, &edit);
    let mut level = lit("level");
    level.set_playing(true);
    frame(&mut r, &level);
    assert_eq!(
        names(&r),
        ["edit.png", "level.png"],
        "the held scene's stays"
    );
    edit.set_playing(false);
    frame(&mut r, &edit);
    assert_eq!(names(&r), ["edit.png"], "the level's list is dropped");
}
