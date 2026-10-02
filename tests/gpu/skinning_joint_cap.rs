//! Skinning past joint 63 (#455). The bone palette used to be a fixed 64-matrix
//! uniform: joint 99 of a 100-joint rig read out of bounds and skinned as identity,
//! so a Mixamo-sized character tore silently.
//!
//! A glowing red box, every vertex weighted fully to joint 99, sits in the middle of
//! the view. Joint 99's matrix moves it far off-screen, so a renderer that honours
//! joint 99 shows only sky there; the old cap left the box at rest. The control
//! capture — the same rig with joint 99 at identity — proves the box renders at all,
//! so a missing box cannot come from drawing nothing.

use glam::{Mat4, Vec3};
use rusty::components::{MaterialAsset, MaterialComponent};
use rusty::dev::screenshot::capture;
use rusty::scene::{Camera, DirtyFlag, MeshComponent, Scene};

const JOINTS: usize = 100;
const LAST: u32 = JOINTS as u32 - 1;

/// The box, skinned to joint 99 of a 100-joint palette whose joint 99 is `last`.
fn scene(last: Mat4) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    let id = scene.add_entity("Rig".to_string());
    let (mut vertices, indices) = rusty::components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
    for v in &mut vertices {
        v.joint_indices = [LAST, 0, 0, 0];
        v.joint_weights = [1.0, 0.0, 0.0, 0.0];
    }
    let mut bind_palette = vec![Mat4::IDENTITY; JOINTS];
    bind_palette[JOINTS - 1] = last;
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Box".to_string(),
            asset_ref: None,
            vertices,
            indices,
            bind_palette,
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            skeleton: Default::default(),
            is_dirty: DirtyFlag::new(true),
        }),
    );
    let material = MaterialComponent {
        material: "glow".to_string(),
    };
    scene.world.set_material(id, Some(material));
    let glow = MaterialAsset {
        base_color: [0.0, 0.0, 0.0],
        emissive: [4.0, 0.0, 0.0],
        ..MaterialAsset::default()
    };
    scene.materials.insert("glow".to_string(), glow);
    scene
}

/// How red the centre pixel is (red minus blue): high on the box, negative on sky.
fn centre_redness(path: &std::path::Path) -> i32 {
    let img = image::open(path).expect("png").to_rgb8();
    let p = img.get_pixel(img.width() / 2, img.height() / 2);
    p[0] as i32 - p[2] as i32
}

#[test]
fn gpu_joint_99_of_a_100_joint_rig_is_posed_not_identity() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 6.0), -90.0, 0.0);
    let dir = crate::temp::dir();
    let (rest, posed) = (
        dir.join("rusty_skin_rest.png"),
        dir.join("rusty_skin_j99.png"),
    );
    let away = Mat4::from_translation(Vec3::new(500.0, 0.0, 0.0));
    let c0 = capture(&scene(Mat4::IDENTITY), &cam, &rest, 64, 64).expect("capture");
    let c1 = capture(&scene(away), &cam, &posed, 64, 64).expect("capture");
    if !c0 || !c1 {
        eprintln!("[skinning] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let (at_rest, moved) = (centre_redness(&rest), centre_redness(&posed));
    eprintln!("[skinning] redness rest={at_rest} joint99-moved={moved}");
    assert!(
        at_rest > 100,
        "control: the rig must render at rest ({at_rest})"
    );
    assert!(
        moved < 0,
        "joint 99 must move the box off-screen, not skin as identity \
         (rest={at_rest}, moved={moved})"
    );
}
