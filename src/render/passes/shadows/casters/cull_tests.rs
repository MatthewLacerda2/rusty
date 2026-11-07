//! The light volume's PVS verdict (#1714, part 4), adapter-free: a baked caster its
//! light's row hides is dropped and counted once per entity, a two-sided one and a
//! volume with no row keep it, the frustum and size verdicts come first, and a
//! non-resident occluder opens the volume.

use glam::camera::rh::proj::directx;
use glam::{Mat4, Vec3};

use super::buffer::CasterData;
use super::cull::{cull, Candidate, LightVolume, ShadowCulls};
use super::CasterKey;
use crate::render::draw::batch::IndexRun;
use crate::render::gpu::pipelines::winding::Facing;
use crate::render::scene::occlusion_fixture::{baked, nearest, ROOM_1};
use crate::render::scene::{MeshHandle, OcclusionBinding};

/// Two runs of entity `id` at `at`, small or two-sided as said.
fn runs(id: u32, at: Vec3, small: bool, two_sided: bool) -> [Candidate; 2] {
    [0, 36].map(|start| Candidate {
        id,
        key: CasterKey {
            facing: Facing::Ccw,
            clip: None,
            mesh: MeshHandle(0),
            indices: IndexRun {
                start,
                end: start + 36,
            },
        },
        caster: CasterData::new(Mat4::from_translation(at), 0),
        bounds: Some((at - Vec3::splat(0.5), at + Vec3::splat(0.5))),
        small,
        two_sided,
    })
}

#[test]
fn a_hidden_caster_is_dropped_once_per_entity_unless_two_sided_or_unrowed() {
    let scene = baked();
    let pvs = OcclusionBinding::bound(&scene);
    let (near_at, far_at) = (Vec3::new(2.0, 0.5, 3.5), Vec3::new(12.0, 0.5, 3.5));
    let (near, far) = (
        nearest(&scene, "Prop0", near_at),
        nearest(&scene, "Prop0", far_at),
    );
    let everything = directx::orthographic(-100.0, 100.0, -100.0, 100.0, -100.0, 100.0);
    let row = pvs.shadow_row(ROOM_1);
    let lit = LightVolume {
        view_proj: everything,
        row,
    };
    let take = |c: &[Candidate], v: LightVolume| {
        let mut culls = ShadowCulls::default();
        (cull(c, v, &pvs, &mut culls).len(), culls)
    };
    let both: Vec<Candidate> = [
        runs(near, near_at, false, false),
        runs(far, far_at, false, false),
    ]
    .into_iter()
    .flatten()
    .collect();
    let (kept, culls) = take(&both, lit);
    assert_eq!(kept, 2, "only the near prop's two runs");
    assert_eq!((culls.tested, culls.occlusion), (2, 1), "{culls:?}");
    // No row (a cascade, a shadowmasked tile): nothing culled by occlusion.
    let (kept, culls) = take(&both, LightVolume::open(everything));
    assert_eq!((kept, culls.occlusion), (4, 0));
    // Two-sided: its back faces cast, which the bake never aims at.
    let (kept, culls) = take(&runs(far, far_at, false, true), lit);
    assert_eq!((kept, culls.occlusion), (2, 0));
    // Already culled as small: counted there, not twice.
    let (kept, culls) = take(&runs(far, far_at, true, false), lit);
    assert_eq!((kept, culls.size, culls.occlusion), (0, 1, 0));
}

#[test]
fn a_non_resident_occluder_opens_every_volume() {
    let scene = baked();
    let pvs = OcclusionBinding::bound(&scene);
    let far_at = Vec3::new(12.0, 0.5, 3.5);
    let far = nearest(&scene, "Prop0", far_at);
    let everything = directx::orthographic(-100.0, 100.0, -100.0, 100.0, -100.0, 100.0);
    let lit = LightVolume {
        view_proj: everything,
        row: pvs.shadow_row(ROOM_1),
    };
    assert!(lit.row.is_some());
    assert_eq!(lit.resident(true), lit, "every wall drawn: the row stands");
    let open = lit.resident(false);
    assert_eq!(
        open,
        LightVolume::open(everything),
        "a wall missing from the map"
    );
    let mut culls = ShadowCulls::default();
    let kept = cull(&runs(far, far_at, false, false), open, &pvs, &mut culls);
    assert_eq!((kept.len(), culls.occlusion), (2, 0), "the far prop casts");
}
