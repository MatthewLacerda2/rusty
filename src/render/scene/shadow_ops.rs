//! src/render/scene/shadow_ops.rs — the edits the equivalence tests add for what the
//! shadow sweeps and the static bakes' key read off the retained objects (#1706):
//! a casting mode, a small-object cull, a Cutout library material, a `LODGroup`.

use proptest::prelude::*;

use crate::components::{LodGroupComponent, RenderMode, ShadowCasting};
use crate::render::passes::shadows::{statics_key, UnbakedCasters};
use crate::render::Renderer;
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
    match kept.unbaked == UnbakedCasters::of(scene) {
        true => Ok(()),
        false => Err("the unbaked renderers differ".into()),
    }
}
