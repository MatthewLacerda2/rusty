//! The lightmap binding's state (#888, after #810's survivors): which page lists
//! rebind, when direction pages count as bound, and which instances read them. The
//! zeroed fallback makes a wrong `directional()` invisible in pixels, so these assert
//! on the state. Skips when no adapter is present.

use glam::Mat4;

use super::lightmaps::Lightmaps;
use crate::render::draw::uniforms::solid_instance;
use crate::render::test_gpu::headless_or_skip;
use crate::scene::lighting::lightmap::LightmapEntry;
use crate::scene::Scene;

/// Write a black `edge`² PNG named `name` under a per-process temp dir.
fn page(name: &str, edge: u32) -> String {
    let dir = std::env::temp_dir().join(format!("rusty_888_{}", std::process::id()));
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
    let mut maps = Lightmaps::new(device);

    assert!(maps.bind(device, queue, &a, &[]));
    assert!(maps.resident() && !maps.directional(), "no direction pages");
    assert!(maps.bind(device, queue, &b, &[]), "new colour pages rebind");
    assert!(!maps.bind(device, queue, &b, &[]), "the same lists do not");
    assert!(maps.bind(device, queue, &b, &mismatched));
    assert!(!maps.directional(), "direction pages of another size");
    assert!(maps.bind(device, queue, &b, &matching));
    assert!(maps.directional());

    let mut scene = Scene::new();
    scene.lightmaps.entries.push(LightmapEntry {
        entity: 1,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    let instance = |id| solid_instance(&scene, id, Mat4::IDENTITY, false, &maps);
    assert_eq!(instance(1).lightmap_directional, 1, "lightmapped");
    assert_eq!(
        instance(2).lightmap_directional,
        0,
        "no lightmap of its own"
    );
}
