//! crates/render/src/scene/occlusion/ops.rs — what the equivalence tests add for the baked
//! PVS's binding (#1714): the yard made static and baked, and a check that the
//! binding an extract keeps decides what one bound from scratch does, after any
//! edit (a `LODGroup` change among them, through `shadow_ops`).

use std::sync::Arc;

use super::OcclusionBinding;
use crate::scene::fixture::yard;
use crate::Renderer;
use rusty_lighting::occlusion::bake_occlusion;
use rusty_scene::Scene;

/// [`yard`] with every mesh static and its PVS baked.
pub(in crate::scene) fn baked_yard(n: u32) -> Scene {
    let mut scene = yard(n);
    for id in scene.world.ids_with_mesh() {
        scene.world.set_static(id, true);
    }
    let data = bake_occlusion(&scene, 1).0.expect("the boxes occlude");
    *scene.occlusion = Some(Arc::new(data));
    scene
}

/// Whether the binding `r` keeps for `scene` equals one bound from scratch.
pub(in crate::scene) fn kept_equals_fresh(r: &Renderer, scene: &Scene) -> Result<(), String> {
    let kept = r.retained.scene(scene.id()).expect("extracted");
    let mut fresh = OcclusionBinding::default();
    fresh.refresh(scene);
    let (kept, fresh) = (kept.occlusion.state(), fresh.state());
    match kept == fresh {
        true => Ok(()),
        false => Err(format!("PVS binding {kept:?}, from scratch {fresh:?}")),
    }
}
