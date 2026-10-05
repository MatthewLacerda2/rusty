//! Skinned meshes are frustum-culled by their pose (#833), on the GPU: a character
//! off-screen and out of every shadow volume submits nothing, and one whose rest
//! pose is off-screen but whose posed limb is in view still draws. Skips when no
//! adapter is present.

use glam::{Mat4, Quat, Vec3};

use crate::asset::{JointTransform, SkinData};
use crate::components::mesh::primitives::generate_sphere;
use crate::components::MeshComponent;
use crate::render::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

const W: u32 = 64;
const H: u32 = 64;

/// A one-joint skinned unit sphere whose pose moves it by `pose` (mesh space).
fn skinned_sphere(pose: Vec3) -> MeshComponent {
    let (vertices, indices) = generate_sphere(1.0, 12, 16);
    let skin = SkinData {
        inverse_bind: vec![Mat4::IDENTITY],
        bind_global: vec![Mat4::IDENTITY],
        local_bind: vec![JointTransform::default()],
        parents: vec![None],
        joint_nodes: vec![0],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["root".into()],
    };
    MeshComponent {
        primitive_type: "SkinnedCullSphere".to_string(),
        asset_ref: None,
        vertices,
        indices,
        bind_palette: skin.bind_palette(),
        skin: Some(skin),
        clips: Vec::new(),
        pose_palette: vec![Mat4::from_translation(pose)],
        skeleton: Default::default(),
        is_dirty: Default::default(),
    }
}

/// A sun, and (when `at` is given) the skinned sphere placed there, posed by `pose`.
fn scene(at: Option<Vec3>, pose: Vec3) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    if let Some(at) = at {
        let id = scene.add_entity("Soldier".to_string());
        scene.world.transform_mut(id).unwrap().position = at;
        scene.world.set_mesh(id, Some(skinned_sphere(pose)));
    }
    scene
}

/// Render `scene` once from a camera at `(0, 1.5, 1)` looking down -Z.
fn render(renderer: &mut Renderer, scene: &Scene) -> RenderCounters {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H, 2);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.5, 1.0), -90.0, -10.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    renderer.frame_counters
}

#[test]
fn gpu_skinned_meshes_are_culled_by_their_pose() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let empty = render(&mut renderer, &scene(None, Vec3::ZERO));
    let far = Vec3::new(500.0, 0.0, 0.0);

    // Far off to the side: outside the camera and every shadow volume.
    let away = render(&mut renderer, &scene(Some(far), Vec3::ZERO));
    assert_eq!(
        (away.visible_entities, away.culled_entities),
        (0, 1),
        "{away:?}"
    );
    assert_eq!(
        away.triangles, empty.triangles,
        "no pass draws it: {away:?}"
    );

    // The same entity, its pose carrying the mesh back in front of the camera: the
    // rest-pose box is still 500 m away, the posed bound is not.
    let posed = render(
        &mut renderer,
        &scene(Some(far), Vec3::new(-500.0, 0.5, -5.0)),
    );
    assert_eq!(
        (posed.visible_entities, posed.culled_entities),
        (1, 0),
        "{posed:?}"
    );
    assert!(posed.triangles > empty.triangles, "{posed:?}");
}
