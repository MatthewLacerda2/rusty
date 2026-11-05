//! src/render/scene/records_gpu_tests.rs — the material table on the GPU (#1706): a
//! colour edit re-derives no object and uploads its one record, and draws what a
//! fresh renderer draws; a key edit re-derives the material's users; two kept scenes
//! holding a same-named material each draw their own record.

use super::fixture::{frame, fresh, held, yard, RES};
use crate::components::RenderMode;
use crate::render::{MaterialData, Renderer};
use crate::scene::Scene;

fn fresh_pixels(scene: &Scene) -> Vec<u8> {
    let mut other = crate::render::test_gpu::headless_or_skip(RES, RES).expect("one ran");
    frame(&mut other, scene).0
}

fn red(scene: &mut Scene) -> &mut crate::components::MaterialAsset {
    scene.materials.get_mut("red").expect("yard's")
}

#[test]
fn gpu_a_colour_edit_rewrites_one_record_and_derives_no_object() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(12);
    frame(&mut r, &scene);
    let idle = frame(&mut r, &scene).1.render_upload_bytes;
    for step in 1..=3 {
        red(&mut scene).base_color = [0.2 * step as f32, 0.9, 0.1];
        let (pixels, c) = frame(&mut r, &scene);
        assert_eq!((c.render_extracted, c.render_posed), (0, 0), "step {step}");
        let record = std::mem::size_of::<MaterialData>() as u64;
        assert_eq!(
            c.render_upload_bytes - idle,
            record,
            "one record, step {step}"
        );
        assert_eq!(
            pixels,
            fresh_pixels(&scene),
            "step {step}: the colour drawn"
        );
    }
    // (A fresh build in `r` writes object slots of its own: checked after.)
    assert_eq!(held(&r, &scene), fresh(&mut r, &scene));
    // A glass's alpha is its record's too.
    scene.materials.get_mut("glass").unwrap().alpha = 0.8;
    let (pixels, c) = frame(&mut r, &scene);
    assert_eq!(c.render_extracted, 0, "an alpha edit");
    assert_eq!(pixels, fresh_pixels(&scene));
    // Turning it Cutout changes its key: its users (props 1, 7 and 10; prop 4
    // is glass) are re-derived.
    red(&mut scene).render_mode = RenderMode::Cutout;
    let (pixels, c) = frame(&mut r, &scene);
    assert_eq!(c.render_extracted, 3, "red's users");
    assert_eq!(pixels, fresh_pixels(&scene));
    // A cutoff edit is a static bake's term: it re-keys too.
    red(&mut scene).alpha_cutoff = 0.9;
    assert_eq!(frame(&mut r, &scene).1.render_extracted, 3);
}

/// The red of the frame's brightest red pixel.
fn reddest(px: &[u8]) -> u8 {
    px.chunks(4)
        .map(|p| p[0].saturating_sub(p[1]))
        .max()
        .unwrap_or(0)
}

#[test]
fn gpu_two_kept_scenes_draw_their_own_record_of_one_name() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let (mut main, mut preview) = (yard(6), yard(6));
    red(&mut preview).base_color = [0.1, 0.9, 0.1]; // "red" is green there
    let draw = |r: &mut Renderer, s: &Scene| frame(r, s).0;
    let (m, p) = (draw(&mut r, &main), draw(&mut r, &preview));
    assert!(
        reddest(&m) > 100 && reddest(&p) < 40,
        "{} {}",
        reddest(&m),
        reddest(&p)
    );
    for _ in 0..2 {
        assert_eq!(draw(&mut r, &main), m, "nothing changed: the main scene");
        assert_eq!(draw(&mut r, &preview), p, "nor the preview");
    }
    // A colour edit in one rewrites that scene's record alone.
    red(&mut main).base_color = [0.9, 0.1, 0.9];
    let edited = draw(&mut r, &main);
    assert_eq!(edited, fresh_pixels(&main));
    assert_eq!(draw(&mut r, &preview), p, "the preview keeps its own");
    let names = |s: &Scene| r.retained.records.variants_of(s.id(), "red");
    assert_ne!(names(&main), names(&preview), "a record per scene");
    // The library dropping the name frees its records.
    preview.materials.remove("red");
    draw(&mut r, &preview);
    assert!(r
        .retained
        .records
        .variants_of(preview.id(), "red")
        .is_empty());
    assert_eq!(draw(&mut r, &preview), fresh_pixels(&preview));
    assert_eq!(draw(&mut r, &main), edited);
}
