//! Scene builders for the world-space UI screenshots (#429): unlit emissive boxes
//! (zero ambient, no lights, so a surface's colour is its emissive), canvases and
//! solid UI elements, and a colour-centroid probe for locating what drew where.

use glam::{Vec2, Vec3, Vec4};
use rusty::components::{
    CanvasComponent, ImageComponent, MaterialAsset, MaterialComponent, RectTransformComponent,
};
use rusty::scene::{DirtyFlag, MeshComponent, Scene};

/// An empty scene with no ambient light.
pub fn dark_scene() -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    scene
}

/// An opaque box of `size` at `pos` whose colour is exactly `emissive`.
pub fn emissive_box(scene: &mut Scene, name: &str, pos: Vec3, size: Vec3, emissive: [f32; 3]) {
    let id = scene.add_entity(name.to_string());
    let (vertices, indices) =
        rusty::components::mesh::primitives::generate_box(size.x, size.y, size.z);
    scene.world.transform_mut(id).expect("transform").position = pos;
    let mesh = MeshComponent {
        primitive_type: "Box".to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        skeleton: Default::default(),
        is_dirty: DirtyFlag::new(true),
    };
    scene.world.set_mesh(id, Some(mesh));
    let material = Some(MaterialComponent {
        material: name.to_string(),
    });
    scene.world.set_material(id, material);
    let asset = MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        emissive,
        ..MaterialAsset::default()
    };
    scene.materials.insert(name.to_string(), asset);
}

/// A root canvas entity carrying `canvas`.
pub fn canvas(scene: &mut Scene, canvas: CanvasComponent) -> u32 {
    let id = scene.add_entity("Canvas".to_string());
    scene.world.set_canvas(id, Some(canvas));
    id
}

/// A solid `color` image under `parent`: `rt` places it.
pub fn image(scene: &mut Scene, parent: u32, rt: RectTransformComponent, color: Vec4) -> u32 {
    let id = scene.add_entity("Image".to_string());
    scene.world.set_rect_transform(id, Some(rt));
    let image = ImageComponent {
        color,
        ..Default::default()
    };
    scene.world.set_image(id, Some(image));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

/// Stretch anchors filling the parent.
pub fn fill() -> RectTransformComponent {
    RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ONE,
        size_delta: Vec2::ZERO,
        ..Default::default()
    }
}

/// The centroid (UI pixels: bottom-left origin, y-up) of the pixels `is` accepts,
/// and how many there were.
pub fn centroid(path: &std::path::Path, is: impl Fn([u8; 3]) -> bool) -> (Vec2, usize) {
    let img = image::open(path).expect("png").to_rgb8();
    let (mut sum, mut n) = (Vec2::ZERO, 0);
    for (x, y, p) in img.enumerate_pixels() {
        if is(p.0) {
            let up = (img.height() - 1 - y) as f32;
            sum += Vec2::new(x as f32 + 0.5, up + 0.5);
            n += 1;
        }
    }
    (sum / n.max(1) as f32, n)
}

/// Whether channel `c` of `p` dominates the other two by a clear margin.
pub fn dominant(p: [u8; 3], c: usize) -> bool {
    (0..3).all(|o| o == c || i32::from(p[c]) > i32::from(p[o]) + 40)
}
