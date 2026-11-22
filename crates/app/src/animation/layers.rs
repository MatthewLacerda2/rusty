use glam::{Quat, Vec3};

use rusty_asset::animation_graph::LayerBlending;
use rusty_asset::mesh_data::{JointTransform, SkinData};

use super::blend_joint;
use super::motion::Pose;

pub struct LayerPose<'a> {
    pub blending: LayerBlending,
    pub weight: f32,
    pub mask: &'a [bool],
    pub pose: &'a Pose,
    pub reference: &'a Pose,
}

pub fn mask_slots(skin: &SkinData, mask: &[String]) -> Vec<bool> {
    let joints = skin.local_bind.len();
    if mask.is_empty() {
        return vec![true; joints];
    }
    let named = |slot: usize| skin.names.get(slot).is_some_and(|n| mask.contains(n));
    (0..joints)
        .map(|slot| {
            let mut at = Some(slot);
            // Bounded by the joint count, so a cyclic `parents` (a malformed skin) can't loop
            // forever.
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
            LayerBlending::Override => blend_joint(below, *top, weight),
            LayerBlending::Additive => {
                let reference = layer.reference.get(slot).copied().flatten();
                add(below, *top, reference.unwrap_or(*top), weight)
            }
        });
    }
}

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
