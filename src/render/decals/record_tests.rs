//! The decal record the forward shader reads (#638): what it changes, by how much,
//! and the box and frame it projects through.

use glam::{Mat4, Vec3, Vec4};

use super::{bounds, maps, record, NO_LAYER};
use crate::components::{DecalBlend, MaterialAsset};
use crate::scene::decal::{Decal, DecalSpec};

const NONE: [u32; 4] = [NO_LAYER; 4];

fn wall_hit(spec: DecalSpec) -> Decal {
    Decal::from_hit(Vec3::new(1.0, 2.0, 0.0), Vec3::Z, spec)
}

#[test]
fn a_decal_without_a_material_changes_only_the_albedo_by_its_texture() {
    let decal = wall_hit(DecalSpec {
        color: [1.0, 0.0, 0.0, 0.5],
        texture: Some("hole.png".into()),
        ..Default::default()
    });
    let r = record(&decal, None, NONE);
    assert_eq!(r.color, [1.0, 0.0, 0.0, 0.5]);
    assert_eq!((r.right[3], r.up[3]), (1.0, 0.0), "albedo on, normal off");
    assert_eq!(&r.surface[2..], &[0.0, 0.0], "metallic and roughness off");
    assert_eq!(r.fade[0], 1.0, "no occlusion added");
    assert_eq!(
        maps(&decal, None),
        [Some("hole.png".into()), None, None, None]
    );
}

#[test]
fn a_material_decal_carries_its_factors_maps_and_clamped_weights() {
    let blood = MaterialAsset {
        base_color: [0.5, 0.0, 0.0],
        alpha: 0.8,
        metallic: 0.1,
        roughness: 0.2,
        normal_map: Some("n.png".into()),
        roughness_map: Some("r.png".into()),
        decal: DecalBlend {
            normal: 0.0,
            roughness: 2.0,
            occlusion: -1.0,
            ..DecalBlend::default()
        },
        ..MaterialAsset::default()
    };
    let decal = wall_hit(DecalSpec {
        color: [1.0, 1.0, 1.0, 0.5],
        texture: Some("fallback.png".into()),
        material: Some("blood".into()),
        ..Default::default()
    });
    let r = record(&decal, Some(&blood), [3, 4, NO_LAYER, 5]);
    assert_eq!(r.color, [0.5, 0.0, 0.0, 0.4], "material colour × tint");
    assert_eq!(r.surface, [0.1, 0.2, 1.0, 1.0], "weights clamp to [0, 1]");
    assert_eq!(r.up[3], 0.0, "normal weight");
    assert_eq!(r.fade[0], 0.0, "occlusion clamps to [0, 1]");
    assert_eq!(r.layers, [3, 4, NO_LAYER, 5]);
    let want = [
        Some("fallback.png".into()),
        Some("n.png".into()),
        None,
        Some("r.png".into()),
    ];
    assert_eq!(
        maps(&decal, Some(&blood)),
        want,
        "no albedo map: the decal's own"
    );
}

#[test]
fn the_box_straddles_the_hit_and_projects_into_the_surface() {
    let decal = wall_hit(DecalSpec {
        size: 2.0,
        depth: 0.5,
        rotation_deg: 90.0,
        ..Default::default()
    });
    let r = record(&decal, None, NONE);
    let to_decal = Mat4::from_cols_array(&r.world_to_decal);
    let hit = to_decal.transform_point3(Vec3::new(1.0, 2.0, 0.0));
    assert!(hit.length() < 1e-5, "the hit is the box's centre: {hit}");
    let edge = to_decal.transform_point3(Vec3::new(1.0, 2.0, 0.25));
    assert!((edge.z - 0.5).abs() < 1e-5, "depth spans the box: {edge}");
    let axis = |v: [f32; 4]| Vec4::from(v).truncate();
    assert!(
        axis(r.forward).abs_diff_eq(Vec3::Z, 1e-5),
        "out of the wall"
    );
    let (right, up) = (axis(r.right), axis(r.up));
    assert!(
        right.cross(up).abs_diff_eq(Vec3::Z, 1e-5),
        "right-handed frame"
    );
    assert!(right.dot(Vec3::Z).abs() < 1e-5, "u runs along the wall");
}

#[test]
fn the_angle_fade_runs_fifteen_degrees_past_the_threshold() {
    let r = record(&wall_hit(DecalSpec::default()), None, NONE);
    assert!((r.forward[3] - 60f32.to_radians().cos()).abs() < 1e-6);
    assert!((r.fade[1] - 75f32.to_radians().cos()).abs() < 1e-6);
    let flat = MaterialAsset {
        decal: DecalBlend {
            angle_fade: 85.0,
            ..DecalBlend::default()
        },
        ..MaterialAsset::default()
    };
    let r = record(&wall_hit(DecalSpec::default()), Some(&flat), NONE);
    assert!(r.fade[1].abs() < 1e-6, "the fade never runs past 90°");
}

#[test]
fn the_bounding_sphere_holds_the_whole_box() {
    let decal = wall_hit(DecalSpec {
        size: 2.0,
        depth: 1.0,
        ..Default::default()
    });
    let (centre, radius) = bounds(&decal);
    assert_eq!(centre, decal.position);
    assert!(
        (radius - 1.5).abs() < 1e-6,
        "half the box's diagonal: {radius}"
    );
}
