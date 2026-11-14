//! The animated bound (#833): it holds every vertex of an animated pose, skinned the
//! way the shader skins it, and refuses what it cannot bound. The poses are built
//! here, joint by joint down the chain, since the animation that samples a clip
//! sits above this crate; the bound holds for any palette of them.

use glam::{Mat4, Quat, Vec3, Vec4};

use super::SkinBounds;
use rusty_asset::{JointTransform, SkinData};
use rusty_components::mesh::Vertex;

/// A vertex at `p` weighted to `joints` by `weights`.
fn vertex(p: Vec3, joints: [u32; 4], weights: [f32; 4]) -> Vertex {
    let mut v = Vertex::new(p, Vec3::Y, [0.0, 0.0]);
    v.joint_indices = joints;
    v.joint_weights = weights;
    v
}

/// A three-joint chain up +Y (`hips` at the origin, one unit apart). Vertices ring
/// the chain from y = -0.3 to 2.6, blending each pair of neighbouring joints; one
/// carries weights that do not sum to 1 and one none at all (the shader skins it as
/// identity).
fn chain() -> (SkinData, Vec<Vertex>) {
    let local_bind: Vec<JointTransform> = (0..3)
        .map(|s| JointTransform {
            translation: if s == 0 { Vec3::ZERO } else { Vec3::Y },
            ..JointTransform::default()
        })
        .collect();
    let bind_global: Vec<Mat4> = (0..3)
        .map(|s| Mat4::from_translation(Vec3::Y * s as f32))
        .collect();
    let skin = SkinData {
        inverse_bind: bind_global.iter().map(|g| g.inverse()).collect(),
        bind_global,
        local_bind,
        parents: vec![None, Some(0), Some(1)],
        joint_nodes: vec![0, 1, 2],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["hips".into(), "spine".into(), "head".into()],
    };
    let mut vertices = Vec::new();
    for step in 0..30 {
        let y = -0.3 + step as f32 * 0.1;
        let lower = y.clamp(0.0, 1.999).floor() as u32;
        let t = (y - lower as f32).clamp(0.0, 1.0);
        for (x, z) in [(0.3, 0.0), (-0.3, 0.0), (0.0, 0.3), (0.0, -0.3)] {
            let p = Vec3::new(x, y, z);
            vertices.push(vertex(p, [lower, lower + 1, 0, 0], [1.0 - t, t, 0.0, 0.0]));
        }
    }
    vertices.push(vertex(
        Vec3::new(0.5, 1.5, 0.0),
        [1, 2, 0, 0],
        [0.6, 0.7, 0.0, 0.0],
    ));
    vertices.push(vertex(Vec3::new(0.0, 0.1, 0.4), [0, 0, 0, 0], [0.0; 4]));
    (skin, vertices)
}

/// The palette of a pose that moves every joint, `t` from 0 to 1: the hips slide
/// and squash, the spine bends up to 70° about Z, the head twists about X and grows.
/// Each joint's local transform is composed down the chain (`parents`), as a frame
/// poses its bones, then multiplied by its inverse bind.
fn bend(skin: &SkinData, t: f32) -> Vec<Mat4> {
    let wave = (t * std::f32::consts::TAU).sin();
    let locals = [
        Mat4::from_scale_rotation_translation(
            Vec3::new(1.0 + 0.4 * wave, 1.0 - 0.4 * wave, 1.0),
            Quat::IDENTITY,
            Vec3::new(0.4, -0.2, 0.0) * wave + Vec3::X * t,
        ),
        Mat4::from_rotation_translation(Quat::from_rotation_z(1.2 * wave), Vec3::Y),
        Mat4::from_scale_rotation_translation(
            Vec3::splat(1.0 + 0.5 * wave.abs()),
            Quat::from_rotation_x(0.9 * wave),
            Vec3::Y,
        ),
    ];
    let mut globals: Vec<Mat4> = Vec::new();
    for (j, local) in locals.iter().enumerate() {
        let parent = skin.parents[j].map_or(Mat4::IDENTITY, |p| globals[p]);
        globals.push(parent * *local);
    }
    let palette = globals.iter().zip(&skin.inverse_bind);
    palette.map(|(g, inverse)| *g * *inverse).collect()
}

/// Where the shader draws `v` under `palette` (`blend_joints`, then the divide).
fn skinned(v: &Vertex, palette: &[Mat4]) -> Vec3 {
    let w = v.joint_weights;
    let skin = if w.iter().sum::<f32>() < 0.01 {
        Mat4::IDENTITY
    } else {
        (0..4).fold(Mat4::ZERO, |m, k| {
            m + palette[v.joint_indices[k] as usize] * w[k]
        })
    };
    let p: Vec4 = skin * Vec3::from(v.position).extend(1.0);
    p.truncate() / p.w
}

fn inside(p: Vec3, (min, max): (Vec3, Vec3)) -> bool {
    let eps = Vec3::splat(1e-4);
    p.cmpge(min - eps).all() && p.cmple(max + eps).all()
}

#[test]
fn the_bound_holds_every_vertex_of_every_animated_pose() {
    let (skin, vertices) = chain();
    let bounds = SkinBounds::from_vertices(&vertices);
    let rest = SkinBounds::from_vertices(&vertices).posed(&skin.bind_palette());
    let mut left_the_rest_box = false;
    for k in 0..=40 {
        let palette = bend(&skin, k as f32 / 40.0);
        let posed = bounds
            .posed(&palette)
            .expect("a well-formed skin is bounded");
        for v in &vertices {
            let p = skinned(v, &palette);
            assert!(inside(p, posed), "pose {k}: {p} escapes {posed:?}");
            left_the_rest_box |= !inside(p, rest.unwrap());
        }
    }
    assert!(
        left_the_rest_box,
        "the pose must move a limb past the rest-pose box"
    );
}

#[test]
fn an_unskinned_mesh_bound_is_its_own_box() {
    let vertices = [Vec3::new(-1.0, 0.0, 2.0), Vec3::new(3.0, 5.0, -1.0)]
        .map(|p| Vertex::new(p, Vec3::Y, [0.0, 0.0]));
    let posed = SkinBounds::from_vertices(&vertices).posed(&[Mat4::IDENTITY]);
    assert_eq!(
        posed,
        Some((Vec3::new(-1.0, 0.0, -1.0), Vec3::new(3.0, 5.0, 2.0)))
    );
}

#[test]
fn what_cannot_be_bounded_is_never_culled() {
    let (skin, vertices) = chain();
    let short = &skin.bind_palette()[..2];
    let bounds = SkinBounds::from_vertices(&vertices);
    assert_eq!(
        bounds.posed(short),
        None,
        "a weighted joint past the palette"
    );
    let mut negative = vertices;
    negative[0].joint_weights = [1.2, -0.2, 0.0, 0.0];
    let bounds = SkinBounds::from_vertices(&negative);
    assert_eq!(
        bounds.posed(&skin.bind_palette()),
        None,
        "a negative weight"
    );
}
