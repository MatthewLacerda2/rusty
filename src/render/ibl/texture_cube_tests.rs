//! A baked reflection cube read by a GPU `texture_cube` (#1615): each face lies as
//! the hardware samples it, so a direction off a face's axis reads what the capture
//! saw along it. A face mirrored in-plane or turned reads the dark wall beside the
//! patch instead. Skips with no adapter.

use glam::{Quat, Vec3};

use crate::components::{MaterialAsset, MaterialComponent};
use crate::render::ibl::cubemap::read_cube_shape;
use crate::scene::{MeshComponent, ReflectionProbe, Scene};

const RES: u32 = 64;

#[path = "tap_tests.rs"]
mod tap;
use tap::sample;

/// Off-axis directions, each with the colour of the patch placed along it: on a face
/// edge, and inside each of the six faces off both of its in-plane axes.
const PATCHES: [([f32; 3], [f32; 3]); 7] = [
    ([1.0, 0.0, 1.0], [9.0, 0.0, 0.0]),
    ([0.87, 0.22, 0.44], [0.0, 9.0, 0.0]),
    ([0.3, 0.9, -0.3], [0.0, 0.0, 9.0]),
    ([-0.4, -0.85, 0.35], [9.0, 9.0, 0.0]),
    ([-0.8, 0.3, -0.5], [9.0, 0.0, 9.0]),
    ([0.2, -0.3, -0.93], [0.0, 9.0, 9.0]),
    ([-0.35, 0.3, 0.88], [9.0, 0.0, 0.0]),
];

/// A static box at `pos`, scaled `scale`, glowing `emissive` (black when zero).
fn add_box(scene: &mut Scene, name: &str, pos: Vec3, scale: Vec3, emissive: [f32; 3]) {
    let (vertices, indices) = crate::components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
    let material = MaterialAsset {
        base_color: [0.0; 3],
        emissive,
        ..MaterialAsset::default()
    };
    scene.materials.insert(format!("{name}_mat"), material);
    let id = scene.add_entity(name.to_string());
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene.world.set_static(id, true);
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale, t.rotation) = (pos, scale, Quat::IDENTITY);
    }
    let slot = MaterialComponent::new(format!("{name}_mat"));
    scene.world.set_material(id, Some(slot));
}

/// A closed black room, 16 units across, with a glowing patch along each of
/// [`PATCHES`] five units out.
pub(super) fn patched_room() -> Scene {
    let mut scene = Scene::new();
    for (i, axis) in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ]
    .into_iter()
    .enumerate()
    {
        let thin = Vec3::splat(20.0) - axis.abs() * 19.5;
        add_box(&mut scene, &format!("Wall{i}"), axis * 8.0, thin, [0.0; 3]);
    }
    for (i, (dir, glow)) in PATCHES.iter().enumerate() {
        let at = Vec3::from(*dir).normalize() * 5.0;
        add_box(
            &mut scene,
            &format!("Patch{i}"),
            at,
            Vec3::splat(1.2),
            *glow,
        );
    }
    scene
}

/// Baked, written and loaded as a frame loads it, the cube returns each patch's
/// colour along its direction, at the sharp base level and the first rough mip
/// (which the GPU prefilter writes through the same face matrices).
#[test]
fn gpu_a_baked_cube_reads_each_direction_as_the_capture_saw_it() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(5.0));
    let bytes = r.bake_reflection(&patched_room(), &probe, RES);
    let path = crate::test_temp::dir().join(format!("cube_layout_{}.ktx2", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let path = path.to_string_lossy().into_owned();
    let cube = r
        .load_cubemap_mips(&path, read_cube_shape(&path).unwrap())
        .unwrap();
    let taps: Vec<(Vec3, f32)> = [0.0, 1.0]
        .iter()
        .flat_map(|&lod| {
            PATCHES
                .iter()
                .map(move |(d, _)| (Vec3::from(*d).normalize(), lod))
        })
        .collect();
    let got = sample(&r, &cube.texture, &taps);
    for ((dir, lod), rgba) in taps.iter().zip(&got) {
        let glow = PATCHES
            .iter()
            .find(|(d, _)| Vec3::from(*d).normalize() == *dir)
            .unwrap()
            .1;
        let lit = |c: usize| glow[c] > 0.0;
        let ok = (0..3).all(|c| if lit(c) { rgba[c] > 120 } else { rgba[c] < 60 });
        assert!(
            ok,
            "{dir} at lod {lod}: got {rgba:?}, want the patch glowing {glow:?}"
        );
    }
}
