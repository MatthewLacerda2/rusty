//! src/app/animation/layers.rs — composing layers and avatar masks (#457).
//!
//! Each extra layer is blended over the pose below it, in order, only on the
//! joints its mask covers and its motion keys — #453's "write only keyed joints"
//! rule, per layer:
//! - **Override** blends toward the layer's pose by the layer weight.
//! - **Additive** adds the layer's change from its reference pose (its motion's
//!   first frame): translation offset, rotation delta in the joint's local space
//!   (`base × reference⁻¹ × pose`), scale ratio, each scaled by the weight.
//!
//! A joint the layer drives that nothing below drove starts from its bind pose.

use glam::{Quat, Vec3};

use crate::asset::animation_graph::LayerBlending;
use crate::asset::mesh_data::{JointTransform, SkinData};

use super::motion::Pose;

/// One layer's contribution, ready to compose.
pub struct LayerPose<'a> {
    pub blending: LayerBlending,
    pub weight: f32,
    pub mask: &'a [bool],
    pub pose: &'a Pose,
    /// The additive reference pose (unused by `Override`).
    pub reference: &'a Pose,
}

/// Which joint slots a mask covers: each named bone and its whole subtree. An
/// empty mask covers the whole body; a name the skeleton lacks covers nothing.
pub fn mask_slots(skin: &SkinData, mask: &[String]) -> Vec<bool> {
    let joints = skin.local_bind.len();
    if mask.is_empty() {
        return vec![true; joints];
    }
    let named = |slot: usize| skin.names.get(slot).is_some_and(|n| mask.contains(n));
    (0..joints)
        .map(|slot| {
            // Walk up the parent chain, bounded by the joint count so a malformed
            // cyclic `parents` can't loop forever.
            let mut at = Some(slot);
            for _ in 0..joints {
                let Some(s) = at else { break };
                if named(s) {
                    return true;
                }
                at = skin.parents.get(s).copied().flatten();
            }
            false
        })
        .collect()
}

/// Blend `layer` onto `pose` in place.
pub fn compose(pose: &mut Pose, layer: &LayerPose<'_>, bind: &[JointTransform]) {
    let weight = layer.weight.clamp(0.0, 1.0);
    for (slot, out) in pose.iter_mut().enumerate() {
        let covered = layer.mask.get(slot).copied().unwrap_or(false);
        let Some(Some(top)) = layer.pose.get(slot).filter(|_| covered) else {
            continue;
        };
        let Some(below) = out.or_else(|| bind.get(slot).copied()) else {
            continue;
        };
        *out = Some(match layer.blending {
            LayerBlending::Override => lerp(below, *top, weight),
            LayerBlending::Additive => {
                let reference = layer.reference.get(slot).copied().flatten();
                add(below, *top, reference.unwrap_or(*top), weight)
            }
        });
    }
}

fn lerp(a: JointTransform, b: JointTransform, t: f32) -> JointTransform {
    JointTransform {
        translation: a.translation.lerp(b.translation, t),
        rotation: a.rotation.slerp(b.rotation, t),
        scale: a.scale.lerp(b.scale, t),
    }
}

/// `base` plus `weight` of the change from `reference` to `pose`.
fn add(
    base: JointTransform,
    pose: JointTransform,
    reference: JointTransform,
    weight: f32,
) -> JointTransform {
    let delta_rotation = Quat::IDENTITY.slerp(reference.rotation.inverse() * pose.rotation, weight);
    let ratio = Vec3::select(
        reference.scale.cmpeq(Vec3::ZERO),
        Vec3::ONE,
        pose.scale / reference.scale,
    );
    JointTransform {
        translation: base.translation + (pose.translation - reference.translation) * weight,
        rotation: (base.rotation * delta_rotation).normalize(),
        scale: base.scale * Vec3::ONE.lerp(ratio, weight),
    }
}
