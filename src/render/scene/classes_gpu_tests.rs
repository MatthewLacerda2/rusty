//! src/render/scene/classes_gpu_tests.rs — equal records under different names batch
//! (#1706): boxes each naming its own grey material (what `Material.SetColor` gives
//! each entity) draw in the calls one shared material takes; an edit splits one off
//! with no object derived, and setting it back joins it again; a class's
//! representative edited or freed leaves the rest drawing as one.

use glam::Vec3;

use super::fixture::{frame, fresh, held, table_holds, RES};
use crate::components::{MaterialAsset, MaterialComponent};
use crate::render::test_gpu::HeadlessPair;
use crate::render::{RenderCounters, Renderer};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

const BOXES: u32 = 8;
const GREY: [f32; 3] = [0.5, 0.5, 0.5];

/// [`BOXES`] boxes in a row under a sun, each naming material `p<i>` when `own`, else
/// all naming `grey`; every one of them grey.
fn boxes(own: bool) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    for i in 0..BOXES {
        let name = if own { format!("p{i}") } else { "grey".into() };
        let grey = MaterialAsset {
            base_color: GREY,
            ..MaterialAsset::default()
        };
        scene.materials.insert(name.clone(), grey);
        let id = create_entity(&mut scene, &format!("Box_{i}"), Some(Primitive::Box));
        let at = Vec3::new(i as f32 * 1.1 - 3.85, 0.0, -6.0);
        scene.world.transform_mut(id).unwrap().position = at;
        scene
            .world
            .set_material(id, Some(MaterialComponent::new(name)));
    }
    scene
}

fn colour(scene: &mut Scene, name: &str, c: [f32; 3]) {
    scene.materials.get_mut(name).expect("a box's").base_color = c;
}

fn fresh_pixels(pair: &HeadlessPair, scene: &Scene) -> Vec<u8> {
    let mut other = pair.renderer(RES, RES);
    frame(&mut other, scene).0
}

/// Render `scene` in `r`, check it against a fresh renderer on `pair` and the
/// tables, and give its counters.
fn checked(r: &mut Renderer, pair: &HeadlessPair, scene: &Scene, what: &str) -> RenderCounters {
    let (pixels, c) = frame(r, scene);
    assert_eq!(pixels, fresh_pixels(pair, scene), "{what}: the pixels");
    table_holds(r).unwrap_or_else(|e| panic!("{what}: {e}"));
    c
}

#[test]
fn gpu_equal_records_under_their_own_names_draw_as_one_material() {
    let Some((mut r, pair)) = crate::render::test_gpu::headless_pair_or_skip(RES, RES) else {
        return;
    };
    let shared = frame(&mut r, &boxes(false)).1;
    let mut scene = boxes(true);
    let base = checked(&mut r, &pair, &scene, "own names");
    let calls = |c: &RenderCounters| (c.draw_calls, c.shadow_draws);
    assert_eq!(calls(&base), calls(&shared), "as one shared material");
    // One box's colour edit: one more batch, no object derived.
    colour(&mut scene, "p5", [0.9, 0.1, 0.1]);
    let c = checked(&mut r, &pair, &scene, "p5 red");
    assert_eq!(c.render_extracted, 0, "a colour edit derives nothing");
    assert_eq!(c.draw_calls, base.draw_calls + 1, "p5 draws apart");
    // Set back, it joins the batch again.
    colour(&mut scene, "p5", GREY);
    let c = checked(&mut r, &pair, &scene, "p5 grey again");
    assert_eq!((c.render_extracted, c.draw_calls), (0, base.draw_calls));
    // The class's representative (the least slot, p0's) edited while the rest draw.
    colour(&mut scene, "p0", [0.1, 0.1, 0.9]);
    let c = checked(&mut r, &pair, &scene, "p0 blue");
    assert_eq!(c.draw_calls, base.draw_calls + 1);
    colour(&mut scene, "p0", GREY);
    assert_eq!(
        checked(&mut r, &pair, &scene, "p0 back").draw_calls,
        base.draw_calls
    );
    // Freed while the rest draw: its box falls back to the default material.
    scene.materials.remove("p0");
    let c = checked(&mut r, &pair, &scene, "p0 removed");
    assert_eq!(c.render_extracted, 1, "p0's user");
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
}
