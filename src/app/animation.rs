//! src/app/animation.rs — the deterministic keyframe sampler (#80).
//!
//! Poses an imported skeleton ([`SkinData`]) from an [`AnimationClip`] at a given
//! clip time. Since #453 the pose lands on the **bone GameObjects**: the sampler
//! produces each joint's local TRS, a crossfade blends two poses in TRS space
//! (lerp translation and scale, slerp rotation — blending finished matrices
//! shrinks limbs), and the `animate` system writes the result onto the bones. The
//! skin palette is then built from the bones by `Scene::build_skin_palettes`, after
//! every later writer (scripts, physics) had its say.
//!
//! Since #457 a node's motion may be a **blend tree** (`blend_tree`, `motion`) and
//! the graph may stack **layers** over the base layer, each masked to a set of
//! bone subtrees and blended by override or addition (`layers`). Every layer is
//! the same kind of pose — per-joint local TRS, `None` where it is not driven —
//! so they compose joint by joint, and only joints something keys are written.
//!
//! Purity / determinism: every function here is a pure function of (skin, clips,
//! graph, parameters, time) — no wall clock, no RNG — so a headless replay at the fixed timestep is
//! byte-reproducible. `glam` math only.

mod blend_tree;
mod clip;
pub(crate) mod graph;
mod layers;
mod motion;

pub use blend_tree::tree_weights;
pub use clip::sample_palette;
pub use motion::Pose;

use crate::asset::animation_graph::{AnimationGraph, LayerBlending};
use crate::asset::mesh_data::{JointTransform, SkinData};
use crate::components::{AnimatorComponent, MeshComponent, TransformComponent};
use layers::{compose, mask_slots, LayerPose};
use motion::MotionContext;

/// The local transform `anim` gives each of `mesh`'s bones this tick, as
/// `(bone entity, transform)` writes. A joint no layer's playing motion keys (or
/// whose layers' masks leave it out) is not written, so a script or an override
/// posing it keeps it (Unity leaves unanimated properties alone). Empty without a
/// bound skeleton: a stopped animator leaves the bones where they are. `graph` is
/// the animator's graph asset, when it has one — blend-tree nodes and layers
/// resolve against it.
pub fn bone_writes(
    anim: &AnimatorComponent,
    mesh: &MeshComponent,
    graph: Option<&AnimationGraph>,
) -> Vec<(u32, TransformComponent)> {
    let Some((skin, pose)) = sample_pose(anim, mesh, graph) else {
        return Vec::new();
    };
    let bones = &mesh.skeleton.bones;
    if bones.len() != pose.len() {
        return Vec::new();
    }
    pose.into_iter()
        .enumerate()
        .filter_map(|(slot, local)| Some((bones[slot], bone_local(skin, slot, local?))))
        .collect()
}

/// A joint's sampled local as its bone's Transform. A skeleton root's bone hangs
/// off the skinned entity, so the fixed root offset (an `Armature` node) is folded
/// in; every other bone's parent is its parent joint's bone, so TRS maps 1:1.
fn bone_local(skin: &SkinData, slot: usize, local: JointTransform) -> TransformComponent {
    if skin.parents.get(slot).copied().flatten().is_some() {
        return TransformComponent {
            position: local.translation,
            rotation: local.rotation,
            scale: local.scale,
        };
    }
    TransformComponent::from_matrix(skin.root_offset(slot) * local.matrix())
}

/// The pose `anim` puts `mesh`'s skeleton in: the base layer's playback, then
/// each extra layer of `graph` composed over it in order. `None` for a skinless or
/// clipless mesh.
pub fn sample_pose<'m>(
    anim: &AnimatorComponent,
    mesh: &'m MeshComponent,
    graph: Option<&AnimationGraph>,
) -> Option<(&'m SkinData, Pose)> {
    let skin = mesh.skin.as_ref().filter(|_| !mesh.clips.is_empty())?;
    let context = |layer: usize| MotionContext {
        clips: &mesh.clips,
        machine: graph.and_then(|g| g.machine(layer)),
        parameters: &anim.parameters,
    };
    let mut pose = context(0)
        .sample(skin, &anim.base, false)
        .unwrap_or_else(|| vec![None; skin.local_bind.len()]);
    let layers = graph.map_or(&[][..], |g| &g.layers[..]);
    for (i, (layer, state)) in layers.iter().zip(&anim.layers).enumerate() {
        let weight = state.weight.unwrap_or(layer.weight);
        let ctx = context(i + 1);
        let Some(top) = ctx
            .sample(skin, &state.playback, false)
            .filter(|_| weight > 0.0)
        else {
            continue;
        };
        let reference = match layer.blending {
            LayerBlending::Additive => ctx.sample(skin, &state.playback, true).unwrap_or_default(),
            LayerBlending::Override => Vec::new(),
        };
        let mask = mask_slots(skin, &layer.mask);
        let layer_pose = LayerPose {
            blending: layer.blending,
            weight,
            mask: &mask,
            pose: &top,
            reference: &reference,
        };
        compose(&mut pose, &layer_pose, &skin.local_bind);
    }
    Some((skin, pose))
}

/// Advance every layer's playheads by `dt` (scaled by the animator's `speed` and
/// each node's rate), each at its own motion's rate: a clip in seconds, a blend
/// tree by its weighted cycle length. Nothing moves while stopped or paused.
pub fn advance(
    anim: &mut AnimatorComponent,
    mesh: Option<&MeshComponent>,
    graph: Option<&AnimationGraph>,
    dt: f32,
) {
    if !anim.is_running() {
        return;
    }
    let clips = mesh.map_or(&[][..], |m| &m.clips[..]);
    let AnimatorComponent {
        base,
        layers,
        parameters,
        speed,
        ..
    } = anim;
    let context = |layer: usize| MotionContext {
        clips,
        machine: graph.and_then(|g| g.machine(layer)),
        parameters,
    };
    context(0).advance(base, dt, *speed);
    for (i, state) in layers.iter_mut().enumerate() {
        context(i + 1).advance(&mut state.playback, dt, *speed);
    }
}

/// Blend two poses of equal length in TRS space, `t` in `[0, 1]` (0 → `from`,
/// 1 → `to`): translation and scale lerp, rotation slerps along the shortest arc,
/// so a limb keeps its length however far apart the two poses are. A length
/// mismatch is truncated to the shorter (poses from one skin always match).
pub fn blend_poses(from: &[JointTransform], to: &[JointTransform], t: f32) -> Vec<JointTransform> {
    let t = t.clamp(0.0, 1.0);
    from.iter()
        .zip(to.iter())
        .map(|(a, b)| JointTransform {
            translation: a.translation.lerp(b.translation, t),
            rotation: a.rotation.slerp(b.rotation, t),
            scale: a.scale.lerp(b.scale, t),
        })
        .collect()
}

#[cfg(test)]
mod tests;
