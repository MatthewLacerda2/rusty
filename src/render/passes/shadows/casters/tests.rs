use super::*;
use crate::render::gpu::pipelines::winding::is_mirrored;
use glam::{Mat4, Vec3};

fn at(x: f32, bone_base: u32) -> CasterData {
    CasterData::new(Mat4::from_translation(Vec3::X * x), bone_base)
}

/// A plain caster key of mesh `m`, drawing indices `0..n`.
fn mesh(m: &str, n: u32) -> CasterKey {
    CasterKey {
        facing: Facing::Front,
        clip: None,
        mesh: MeshId(m.to_string()),
        indices: (0..n).into(),
    }
}

#[test]
fn casters_of_one_mesh_become_one_instanced_draw() {
    let casters = vec![
        (mesh("Box", 36), at(0.0, 0)),
        (mesh("Pillar", 96), at(1.0, 0)),
        (mesh("Box", 36), at(2.0, 0)),
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
        (mesh("Rig", 900), at(0.0, 1)),
        (mesh("Rig", 900), at(4.0, 31)),
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
        ..mesh("Box", 36)
    };
    let casters = vec![
        (clipped.clone(), at(0.0, 0).cutout(0.5, true)),
        (mesh("Box", 36), at(1.0, 0)),
        (clipped, at(2.0, 0)),
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

/// A mirrored caster (#1134) draws with the winding flipped, so it never shares a
/// draw with an unmirrored copy of its mesh.
#[test]
fn a_mirrored_caster_draws_apart_from_unmirrored_copies() {
    let flip = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    assert!(is_mirrored(flip) && !is_mirrored(Mat4::IDENTITY));
    let twice = Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
    assert!(!is_mirrored(twice), "two flips are a turn");
    let mirrored = CasterKey {
        facing: Facing::Mirrored,
        ..mesh("Box", 36)
    };
    let casters = vec![
        (mirrored.clone(), at(0.0, 0)),
        (mesh("Box", 36), at(1.0, 0)),
        (mirrored, at(2.0, 0)),
    ];
    let (_, batches) = batch_casters(casters, true);
    let runs: Vec<_> = batches.iter().map(|b| (b.key.facing, b.counts())).collect();
    assert_eq!(
        runs,
        [(Facing::Front, (36, 1)), (Facing::Mirrored, (36, 2))]
    );
}

/// Mirroring sorts before the clip's material, so one clip pipeline's plain and
/// mirrored versions each bind once, not once per material.
#[test]
fn mirrored_runs_sort_before_the_clip_material() {
    let key = |mirrored: bool, material| CasterKey {
        facing: [Facing::Front, Facing::Mirrored][usize::from(mirrored)],
        clip: Some(Clip {
            pipeline: 1,
            material,
        }),
        ..mesh("Box", 36)
    };
    let casters = [(true, 3), (false, 4), (true, 4), (false, 3)]
        .map(|(m, mat)| (key(m, mat), at(0.0, 0)))
        .to_vec();
    let (_, batches) = batch_casters(casters, true);
    let order: Vec<_> = batches
        .iter()
        .map(|b| (b.key.facing, b.key.clip.unwrap().material))
        .collect();
    let (f, m) = (Facing::Front, Facing::Mirrored);
    assert_eq!(order, [(f, 3), (f, 4), (m, 3), (m, 4)]);
}

/// A two-sided caster (#1571) culls nothing, so it draws apart from its mesh's
/// one-sided copies, mirrored or not: winding no longer matters to it.
#[test]
fn a_two_sided_caster_draws_apart_whatever_its_winding() {
    let flip = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
    assert_eq!(Facing::of(flip, true), Facing::Both);
    assert_eq!(Facing::of(Mat4::IDENTITY, true), Facing::Both);
    assert_eq!(Facing::of(flip, false), Facing::Mirrored);
    assert_eq!(Facing::Both.cull_mode(), None);
    let both = CasterKey {
        facing: Facing::Both,
        ..mesh("Card", 6)
    };
    let casters = vec![(both, at(0.0, 0)), (mesh("Card", 6), at(1.0, 0))];
    let (_, batches) = batch_casters(casters, true);
    let facings: Vec<_> = batches.iter().map(|b| b.key.facing).collect();
    assert_eq!(facings, [Facing::Front, Facing::Both]);
}
