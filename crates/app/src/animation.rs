mod blend_tree;
mod clip;
mod events;
pub(crate) mod graph;
mod layers;
mod montage;
mod motion;
pub(crate) mod root_motion;
mod states;
mod sync;

pub use events::{Crossed, END_EVENT};
pub use motion::Pose;
pub use states::state_callbacks;

use layers::{compose, mask_slots, LayerPose};
use motion::MotionContext;
use root_motion::RootBone;
use rusty_asset::animation_graph::{AnimationGraph, LayerBlending};
use rusty_asset::mesh_data::{JointTransform, SkinData};
use rusty_components::{
    AnimatorComponent, MeshComponent, Playback, RootMotion, TransformComponent,
};

pub fn bone_writes(
    anim: &AnimatorComponent,
    mesh: &MeshComponent,
    graph: Option<&AnimationGraph>,
) -> Vec<(u32, TransformComponent)> {
    let Some((skin, mut pose)) = sample_pose(anim, mesh, graph) else {
        return Vec::new();
    };
    if anim.apply_root_motion {
        if let Some(bone) = RootBone::resolve(skin, &anim.root_bone) {
            bone.strip(skin, &mut pose);
        }
    }
    let bones = &mesh.skeleton.bones;
    if bones.len() != pose.len() {
        return Vec::new();
    }
    pose.into_iter()
        .enumerate()
        .filter_map(|(slot, local)| Some((bones[slot]?, bone_local(skin, slot, local?))))
        .collect()
}

/// A root joint's bone hangs off the skinned entity, so the root offset (an `Armature` node) is
/// folded in; any other bone takes the joint's TRS as is.
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
        let weight = state.weight;
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
    montage::compose(&mut pose, skin, &mesh.clips, &anim.montages);
    Some((skin, pose))
}

/// A test hook: [`step`]'s events alone, for this crate's tests.
pub fn advance(
    anim: &mut AnimatorComponent,
    mesh: Option<&MeshComponent>,
    graph: Option<&AnimationGraph>,
    dt: f32,
) -> Vec<Crossed> {
    step(anim, mesh, graph, dt).events
}

#[derive(Debug, Default)]
pub struct Stepped {
    pub events: Vec<Crossed>,
    pub root: Option<RootMotion>,
}

pub fn step(
    anim: &mut AnimatorComponent,
    mesh: Option<&MeshComponent>,
    graph: Option<&AnimationGraph>,
    dt: f32,
) -> Stepped {
    let clips = mesh.map_or(&[][..], |m| &m.clips[..]);
    sync::place_pending(anim, clips, graph);
    if !anim.is_running() {
        return Stepped::default();
    }
    let skin = mesh.and_then(|m| m.skin.as_ref());
    let bone = skin.and_then(|s| RootBone::resolve(s, &anim.root_bone));
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
    let advance_layer = |playback: &mut Playback, layer: usize| {
        let ctx = context(layer);
        let before = ctx.before_step(playback, bone.is_some());
        let (events, step) = ctx.advance(playback, dt, *speed, layer);
        let root = before.and_then(|b| {
            let (skin, bone) = (skin?, bone.as_ref()?);
            ctx.playback_root(skin, bone, playback, step, &b)
        });
        (events, root)
    };
    let (mut events, mut root) = advance_layer(base, 0);
    let graph_layers = graph.map_or(&[][..], |g| &g.layers[..]);
    for (i, state) in layers.iter_mut().enumerate() {
        let (layer_events, layer_root) = advance_layer(&mut state.playback, i + 1);
        if state.weight > 0.0 {
            events.extend(layer_events);
        }
        let covering = graph_layers.get(i).zip(skin).zip(bone.as_ref());
        let covering = covering.filter(|((layer, skin), bone)| {
            state.weight > 0.0 && mask_slots(skin, &layer.mask).get(bone.slot()) == Some(&true)
        });
        if let (Some(delta), Some(((layer, _), _))) = (layer_root, covering) {
            let below = root.unwrap_or_default();
            root = Some(stack(below, delta, layer.blending, state.weight));
        }
    }
    // Montage layer indices start past the base layer (0) and every extra layer (1 to len).
    let (rooted, base) = (skin.zip(bone.as_ref()), graph_layers.len() + 1);
    events.extend(montage::step(anim, clips, dt, base, rooted, &mut root));
    events::sort(&mut events);
    Stepped { events, root }
}

fn stack(below: RootMotion, delta: RootMotion, blending: LayerBlending, weight: f32) -> RootMotion {
    let weight = weight.clamp(0.0, 1.0);
    match blending {
        LayerBlending::Override => below.lerp(delta, weight),
        LayerBlending::Additive => below.add_scaled(delta, weight),
    }
}

pub fn blend_poses(from: &[JointTransform], to: &[JointTransform], t: f32) -> Vec<JointTransform> {
    let t = t.clamp(0.0, 1.0);
    from.iter()
        .zip(to.iter())
        .map(|(a, b)| blend_joint(*a, *b, t))
        .collect()
}

fn blend_joint(a: JointTransform, b: JointTransform, t: f32) -> JointTransform {
    JointTransform {
        translation: a.translation.lerp(b.translation, t),
        rotation: a.rotation.slerp(b.rotation, t),
        scale: a.scale.lerp(b.scale, t),
    }
}

#[cfg(test)]
mod bounds_tests;
#[cfg(test)]
mod clamp_tests;
#[cfg(test)]
mod dominant_tests;
#[cfg(test)]
mod end_resolve_tests;
#[cfg(test)]
mod end_tests;
#[cfg(test)]
mod layer_tests;
#[cfg(test)]
mod mirrored_tests;
#[cfg(test)]
mod nested_pose_tests;
#[cfg(test)]
mod playhead_tests;
#[doc(hidden)]
pub mod posed;
#[cfg(test)]
pub(crate) mod test_rig;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tree_tests;
