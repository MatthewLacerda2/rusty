//! The lightmap binding's state (#888, after #810's survivors): which page lists
//! rebind, when direction pages count as bound, and which instances read them. The
//! zeroed fallback makes a wrong `directional()` invisible in pixels, so these assert
//! on the state. Skips when no adapter is present.

use glam::Mat4;

use super::Lightmaps;
use crate::draw::uniforms::solid_object;
use crate::test_gpu::headless_or_skip;
use rusty_scene::lighting::lightmap::LightmapEntry;
use rusty_scene::Scene;

/// Write a black `edge`² PNG named `name` under the test's temp dir.
fn page(name: &str, edge: u32) -> String {
    let dir = rusty_test_support::temp::dir().join("rusty_888");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    image::RgbaImage::new(edge, edge).save(&path).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn gpu_lightmaps_rebind_on_colour_pages_and_match_direction_shapes() {
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let (a, b) = (vec![page("a.png", 2)], vec![page("b.png", 2)]);
    let mismatched = vec![page("wide_dir.png", 4)];
    let matching = vec![page("b_dir.png", 2)];
    let mut maps = Lightmaps::new(device, true);

    assert!(maps.bind(device, queue, &a, &[], &[]));
    assert!(maps.resident() && !maps.directional(), "no direction pages");
    assert!(
        maps.bind(device, queue, &b, &[], &[]),
        "new colour pages rebind"
    );
    assert!(
        !maps.bind(device, queue, &b, &[], &[]),
        "the same lists do not"
    );
    assert!(maps.bind(device, queue, &b, &mismatched, &[]));
    assert!(!maps.directional(), "direction pages of another size");
    assert!(maps.bind(device, queue, &b, &matching, &[]));
    assert!(maps.directional());

    let mut scene = Scene::new();
    scene.lightmaps.insert(LightmapEntry {
        entity: 1,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    let instance = |id| {
        solid_object(
            &scene,
            id,
            (Mat4::IDENTITY, glam::Vec3::ZERO),
            &Default::default(),
            &maps,
        )
    };
    assert_eq!(instance(1).lightmap_directional, 1, "lightmapped");
    assert_eq!(
        instance(2).lightmap_directional,
        0,
        "no lightmap of its own"
    );
}

/// The shared cache contract (#1207): a page list that failed before its page was
/// baked binds once it is, and a re-bake at the same path binds the new page.
#[test]
fn gpu_lightmaps_follow_bakes_at_one_path() {
    use rusty_core::fs::written;
    use rusty_test_support::rebakes::{follows_rebakes, Bake};
    let Some(renderer) = headless_or_skip(4, 4) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let mut maps = Lightmaps::new(device, true);
    let edge = |bake| if bake == Bake::A { 2 } else { 4 };
    let path = rusty_test_support::temp::dir().join("rebaked.png");
    follows_rebakes(
        &path.to_string_lossy(),
        |p, bake| {
            image::RgbaImage::new(edge(bake), edge(bake))
                .save(p)
                .unwrap();
            written::note_written(p);
        },
        |p| {
            maps.bind(device, queue, &[p.to_string()], &[], &[]);
            maps.bound_edge()
        },
        |loaded, bake| *loaded == Some(edge(bake)),
    );
}
