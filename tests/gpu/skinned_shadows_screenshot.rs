//! Skinned casters cast their pose's shadow, not their bind pose's (#599). The depth
//! pass used to transform by the world matrix alone, so an animated character's
//! shadow stayed in its T-pose.
//!
//! A skinned slab floats over the ground under a high sun; its one joint's pose moves
//! it `SHIFT` sideways. Two cameras sit between slab and ground looking straight down,
//! so neither sees the slab — only the ground and the shadow on it. Posed, the ground
//! under the rest position must be lit and the ground under the posed one dark; at
//! rest, the reverse (the control: the slab does cast).

use glam::{Mat4, Quat, Vec3};
use rusty::asset::SkinData;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{create_entity, Primitive};
use rusty::scene::{Camera, DirtyFlag, MeshComponent, Scene};

const SHIFT: f32 = 10.0;

/// Ground, sun and the slab skinned to joint 0, whose pose is `pose`.
fn yard(pose: Mat4) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.3);
    let ground = create_entity(&mut scene, "Ground", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(ground).unwrap();
    t.position = Vec3::new(SHIFT / 2.0, -0.5, 0.0);
    t.scale = Vec3::new(40.0, 1.0, 40.0);
    drop(t);

    let slab = scene.add_entity("Slab".to_string());
    scene.world.transform_mut(slab).unwrap().position = Vec3::Y * 3.0;
    let (mut vertices, indices) = rusty::components::mesh::primitives::generate_box(6.0, 0.2, 6.0);
    for v in &mut vertices {
        v.joint_indices = [0; 4];
        v.joint_weights = [1.0, 0.0, 0.0, 0.0];
    }
    let skin = SkinData {
        inverse_bind: vec![Mat4::IDENTITY],
        bind_global: vec![Mat4::IDENTITY],
        ..SkinData::default()
    };
    scene.world.set_mesh(
        slab,
        Some(MeshComponent {
            primitive_type: "Slab".to_string(),
            asset_ref: None,
            vertices,
            indices,
            bind_palette: vec![Mat4::IDENTITY],
            skin: Some(skin),
            clips: Vec::new(),
            pose_palette: vec![pose],
            skeleton: Default::default(),
            is_dirty: DirtyFlag::new(true),
        }),
    );
    scene
}

/// Mean brightness of a downward look at the ground under `x`, from beneath the slab.
fn ground_brightness(scene: &Scene, x: f32, name: &str) -> Option<f32> {
    // The sun leans the shadow ~0.8 units toward -Z; look where it lands.
    let cam = Camera::new(Vec3::new(x, 1.5, -0.8), -90.0, -89.0);
    let path = crate::temp::dir().join(format!("rusty_{name}_{}.png", std::process::id()));
    if !capture(scene, &cam, &path, 32, 32).expect("capture") {
        return None;
    }
    let img = image::open(&path).expect("png readable").to_rgb8();
    let _ = std::fs::remove_file(&path);
    let sum: u64 = img.pixels().flat_map(|p| p.0).map(u64::from).sum();
    Some(sum as f32 / (img.width() * img.height() * 3) as f32)
}

#[test]
fn gpu_a_posed_skinned_caster_shadows_its_pose_not_its_rest() {
    let rest = yard(Mat4::IDENTITY);
    let posed = yard(Mat4::from_translation(Vec3::X * SHIFT));
    let (Some(rest_here), Some(rest_there), Some(posed_here), Some(posed_there)) = (
        ground_brightness(&rest, 0.0, "skinshadow_rest_here"),
        ground_brightness(&rest, SHIFT, "skinshadow_rest_there"),
        ground_brightness(&posed, 0.0, "skinshadow_posed_here"),
        ground_brightness(&posed, SHIFT, "skinshadow_posed_there"),
    ) else {
        eprintln!("[skinned shadows] no GPU/software adapter — skipping visual assertion");
        return;
    };
    eprintln!(
        "[skinned shadows] rest: under={rest_here:.1} beside={rest_there:.1}; \
         posed: rest spot={posed_here:.1} posed spot={posed_there:.1}"
    );
    assert!(
        rest_there > rest_here + 20.0,
        "control: at rest the slab shades the ground under it"
    );
    assert!(
        posed_here > posed_there + 20.0,
        "posed, the shadow must follow the pose ({posed_there:.1} under it) and leave \
         the rest spot lit ({posed_here:.1})"
    );
}
