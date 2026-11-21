use glam::{Mat4, Quat, Vec3};

use crate::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_asset::{JointTransform, SkinData};
use rusty_components::mesh::primitives::generate_sphere;
use rusty_components::MeshComponent;
use rusty_scene::create::{create_live_light, Primitive};
use rusty_scene::{Camera, Scene};

const W: u32 = 64;
const H: u32 = 64;

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
        bind_palette: skin.bind_palette(),
        skin: Some(skin),
        pose_palette: vec![Mat4::from_translation(pose)],
        ..MeshComponent::new("SkinnedCullSphere", vertices, indices)
    }
}

fn scene(at: Option<Vec3>, pose: Vec3) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    if let Some(at) = at {
        let id = scene.add_entity("Soldier".to_string());
        scene.world.transform_mut(id).unwrap().position = at;
        scene.world.set_mesh(id, Some(skinned_sphere(pose)));
    }
    scene
}

fn render(renderer: &mut Renderer, scene: &Scene) -> RenderCounters {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.5, 1.0), -90.0, -10.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    renderer.frame_counters
}

#[test]
fn gpu_skinned_meshes_are_culled_by_their_pose() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let empty = render(&mut renderer, &scene(None, Vec3::ZERO));
    let far = Vec3::new(500.0, 0.0, 0.0);

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
