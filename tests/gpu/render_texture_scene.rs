//! The scene the render-texture screenshot renders (#430): an emissive red box at
//! the origin, a second camera looking at it into `"rt:pip"`, and a full-height UI
//! Image over the screen's left half showing that texture. The screen's own camera
//! (the capture's base) looks away from the box, so red on screen can only have come
//! through the texture.

use glam::{Vec2, Vec3, Vec4};
use rusty::components::{
    CameraComponent, CanvasComponent, ImageComponent, MaterialAsset, MaterialComponent,
    RectTransformComponent, RenderTarget,
};
use rusty::scene::{Camera, MeshComponent, Scene};

pub const SIZE: u32 = 128;

/// The screen camera: beside the texture camera, looking the other way (+Z).
pub fn screen_camera() -> Camera {
    Camera::new(Vec3::new(0.0, 0.0, 6.0), 90.0, 0.0)
}

/// The scene; `target` is the texture camera's target (`None`: no such camera).
pub fn pip_scene(target: Option<RenderTarget>) -> Scene {
    let mut scene = Scene::new();
    add_red_box(&mut scene);
    if let Some(target) = target {
        // Identity rotation: looks down -Z, at the box.
        let cam = scene.add_entity("PipCamera".to_string());
        let comp = CameraComponent {
            fov: 60.0,
            target_texture: Some(target),
            ..Default::default()
        };
        scene.world.set_camera(cam, Some(comp));
        scene.world.transform_mut(cam).expect("transform").position = Vec3::new(0.0, 0.0, 6.0);
    }
    add_pip_image(&mut scene);
    scene
}

fn add_red_box(scene: &mut Scene) {
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
    let mesh = MeshComponent {
        primitive_type: "Box".to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        is_dirty: rusty::scene::DirtyFlag::new(true),
    };
    scene.world.set_mesh(id, Some(mesh));
    let material = MaterialComponent {
        material: "red".to_string(),
    };
    scene.world.set_material(id, Some(material));
    let red = MaterialAsset {
        base_color: [1.0, 0.0, 0.0],
        emissive: [1.0, 0.0, 0.0],
        ..Default::default()
    };
    scene.materials.insert("red".to_string(), red);
}

/// A canvas one reference unit per pixel, with the Image over the left half.
fn add_pip_image(scene: &mut Scene) {
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: Vec2::splat(SIZE as f32),
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    let id = scene.add_entity("Pip".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: Vec2::ZERO,
        size_delta: Vec2::new(SIZE as f32 / 2.0, SIZE as f32),
        world_anchor: None,
    };
    scene.world.set_rect_transform(id, Some(rt));
    let image = ImageComponent {
        color: Vec4::ONE,
        texture: Some("rt:pip".to_string()),
        ..Default::default()
    };
    scene.world.set_image(id, Some(image));
    scene.set_parent(id, Some(root)).expect("canvas exists");
}
