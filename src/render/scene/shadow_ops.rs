//! src/render/scene/shadow_ops.rs — the edits the equivalence tests add for what the
//! shadow sweeps and the static bakes' key read off the retained objects (#1706):
//! a casting mode, a small-object cull, a Cutout library material, a `LODGroup`.

use glam::camera::rh::proj::directx;
use glam::camera::rh::view::look_to_mat4;
use glam::{Mat4, Vec3};
use proptest::prelude::*;

use super::RenderScene;
use crate::components::{LodGroupComponent, RenderMode, ShadowCasting};
use crate::render::passes::shadows::{statics_key, UnbakedCasters};
use crate::render::{Frustum, Renderer};
use crate::scene::Scene;

#[derive(Clone, Debug)]
pub(super) enum S {
    Cast(usize, u8),
    Small(usize, bool),
    Cutout(bool),
    Group(usize, usize, bool),
}

pub(super) fn s_op() -> impl Strategy<Value = S> {
    let at = || 0usize..12;
    prop_oneof![
        (at(), 0u8..4).prop_map(|(i, m)| S::Cast(i, m)),
        (at(), any::<bool>()).prop_map(|(i, s)| S::Small(i, s)),
        any::<bool>().prop_map(S::Cutout),
        (at(), at(), any::<bool>()).prop_map(|(i, j, g)| S::Group(i, j, g)),
    ]
}

/// Apply `op` to `scene`.
pub(super) fn edit(scene: &mut Scene, op: &S) {
    let ids = scene.world.ids();
    let pick = |i: usize| ids.get(i).copied();
    let modes = [
        ShadowCasting::On,
        ShadowCasting::Off,
        ShadowCasting::TwoSided,
        ShadowCasting::ShadowsOnly,
    ];
    match *op {
        S::Cast(i, m) => {
            if let Some(mut mesh) = pick(i).and_then(|id| scene.world.mesh_mut(id)) {
                mesh.cast_shadows = modes[m as usize];
            }
        }
        S::Small(i, on) => {
            if let Some(mut mesh) = pick(i).and_then(|id| scene.world.mesh_mut(id)) {
                mesh.max_draw_distance = if on { 30.0 } else { 0.0 };
            }
        }
        S::Cutout(on) => {
            let mode = if on {
                RenderMode::Cutout
            } else {
                RenderMode::Opaque
            };
            scene.materials.get_mut("red").unwrap().render_mode = mode;
        }
        S::Group(i, j, on) => {
            let (Some(group), Some(renderer)) = (pick(i), pick(j)) else {
                return;
            };
            let mut lod = LodGroupComponent::default();
            lod.levels[0].renderers.push(renderer);
            scene.world.set_lod_group(group, on.then_some(lod));
        }
    }
}

/// Whether the static bakes' key and the unbaked renderers `r` keeps for `scene`
/// equal a read of the ECS from scratch; what differs, when not.
pub(super) fn kept_equals_fresh(r: &Renderer, scene: &Scene) -> Result<(), String> {
    let kept = r.retained.scene(scene.id()).expect("extracted");
    let (key, fresh) = (kept.statics_key(scene), statics_key(scene));
    if key != fresh {
        return Err(format!("statics key {key:?}, from scratch {fresh:?}"));
    }
    if kept.unbaked != UnbakedCasters::of(scene) {
        return Err("the unbaked renderers differ".into());
    }
    gathers_what_a_walk_keeps(kept)
}

/// The shadow sweeps' gather through the caster BVH (#1874), for short point-light
/// faces every half metre along the yard and a degenerate volume, in walk order:
/// a static bake's holds every static object whose bounds the frustum keeps (or
/// that has none), a per-frame sweep's every dynamic object and every static one
/// that casts per frame.
fn gathers_what_a_walk_keeps(kept: &RenderScene) -> Result<(), String> {
    let face = directx::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.05, 0.8);
    let mut views = vec![Mat4::ZERO];
    for x in (-8..20).map(|x| x as f32 * 0.5) {
        let at = Vec3::new(x, 0.0, -4.0);
        views.extend([Vec3::X, -Vec3::Y].map(|d| face * look_to_mat4(at, d, Vec3::Z)));
    }
    let walk: Vec<u32> = kept.objects().map(|(id, _)| id).collect();
    for (view, baked) in views.into_iter().flat_map(|v| [(v, true), (v, false)]) {
        let near = kept.casters_near(&[view], baked);
        let near: Vec<u32> = near.iter().map(|&(id, _)| id).collect();
        let ordered: Vec<u32> = walk
            .iter()
            .copied()
            .filter(|id| near.contains(id))
            .collect();
        if near != ordered {
            return Err(format!("gathered {near:?}, out of walk order {walk:?}"));
        }
        let frustum = Frustum::from_view_proj(view);
        let mut taken = kept.objects().filter(|&(id, o)| match baked {
            true => {
                o.is_static
                    && o.bounds
                        .is_none_or(|(lo, hi)| frustum.intersects_aabb(lo, hi))
            }
            false => !o.is_static || o.moving || kept.unbaked.holds(id, o.casting.culls_small()),
        });
        if let Some((id, _)) = taken.find(|(id, _)| !near.contains(id)) {
            return Err(format!(
                "object {id} is taken (baked: {baked}) but not gathered"
            ));
        }
    }
    Ok(())
}
