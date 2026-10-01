use super::*;
use glam::{Mat4, Vec3};

fn at(x: f32, bone_base: u32) -> CasterData {
    CasterData::new(Mat4::from_translation(Vec3::X * x), bone_base)
}

fn mesh(m: &str) -> CasterKey {
    CasterKey {
        clip: None,
        mesh: MeshId(m.to_string()),
    }
}

#[test]
fn casters_of_one_mesh_become_one_instanced_draw() {
    let casters = vec![
        (mesh("Box"), 36, at(0.0, 0)),
        (mesh("Pillar"), 96, at(1.0, 0)),
        (mesh("Box"), 36, at(2.0, 0)),
    ];
    let (packed, batches) = batch_casters(casters.clone(), true);
    let counts: Vec<_> = batches.iter().map(CasterBatch::counts).collect();
    assert_eq!(counts, [(36, 2), (96, 1)]);
    let xs: Vec<f32> = packed.iter().map(|c| c.model[12]).collect();
    assert_eq!(xs, [0.0, 2.0, 1.0], "a run keeps scene order");
    let (_, solo) = batch_casters(casters, false);
    assert_eq!(solo.len(), 3, "instancing off: one draw per caster");
}

/// The pose rides each instance (#599), so two skinned copies of one rig cost one
/// shadow draw, each still reading its own palette.
#[test]
fn skinned_copies_of_one_mesh_share_a_draw_with_their_own_palettes() {
    let casters = vec![
        (mesh("Rig"), 900, at(0.0, 1)),
        (mesh("Rig"), 900, at(4.0, 31)),
    ];
    let (packed, batches) = batch_casters(casters, true);
    let counts: Vec<_> = batches.iter().map(CasterBatch::counts).collect();
    assert_eq!(counts, [(900, 2)]);
    let bases: Vec<u32> = packed.iter().map(|c| c.bone_base).collect();
    assert_eq!(bases, [1, 31]);
}

/// A clipped caster (#648) binds its material, so it never shares a draw with a plain
/// copy of its mesh; plain runs sort first, so the clip pipelines bind last.
#[test]
fn a_clipped_caster_draws_apart_from_plain_copies_of_its_mesh() {
    let clip = Clip {
        pipeline: 1,
        material: 3,
    };
    let clipped = CasterKey {
        clip: Some(clip),
        ..mesh("Box")
    };
    let casters = vec![
        (clipped.clone(), 36, at(0.0, 0).cutout(0.5, true)),
        (mesh("Box"), 36, at(1.0, 0)),
        (clipped, 36, at(2.0, 0)),
    ];
    let (packed, batches) = batch_casters(casters, true);
    let keys: Vec<_> = batches.iter().map(|b| b.key.clip).collect();
    assert_eq!(keys, [None, Some(clip)]);
    assert_eq!(packed[1].alpha_cutoff, 0.5, "the cutout rides its instance");
}

#[test]
fn the_caster_layout_matches_the_shadow_stage() {
    // `struct Caster { model: mat4x4<f32>, bone_base, alpha_cutoff, use_texture, _pad0 }`.
    assert_eq!(std::mem::size_of::<CasterData>(), 80);
}
