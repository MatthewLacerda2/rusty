//! FBX animation stacks → glTF animations (#1081).
//!
//! `ufbx::bake_anim` does the hard half: it evaluates every layer of a stack,
//! resolves pivots, pre/post-rotations and Euler orders, and resamples the curves
//! into per-node translation, rotation and scale keys in the same space as
//! `Node::local_transform`. Here each key goes through the file's [`Basis`]
//! part by part, the same calls [`Basis::local`] makes for a node's rest
//! transform, so a key and the rest pose it equals split alike (#1110); each channel
//! is cut to its take's range (`range`), and each stack becomes one glTF
//! animation.

mod range;

use super::basis::Basis;
use crate::asset::convert::{glb, Skip};
use crate::core::collections::Map;
use glam::{Quat, Vec3};

/// One glTF animation per stack in `scene`, keying the nodes `nodes` maps (FBX
/// element id → glTF node index). A stack that keys none of them is dropped; one
/// that fails to bake is added to `skipped` (#1102), as Unity imports the model
/// and its other takes.
pub fn animations(
    scene: &ufbx::Scene,
    basis: Basis,
    nodes: &Map<u32, usize>,
    skipped: &mut Vec<Skip>,
) -> Vec<glb::Animation> {
    let mut out = Vec::new();
    for (i, stack) in scene.anim_stacks.iter().enumerate() {
        let name = if stack.element.name.is_empty() {
            format!("clip_{i}")
        } else {
            stack.element.name.to_string()
        };
        // Untrimmed: `range::clamp` shifts by the take's start, whatever its sign.
        let baked = match ufbx::bake_anim(scene, &stack.anim, ufbx::BakeOpts::default()) {
            Ok(baked) => baked,
            Err(e) => {
                skipped.push(Skip {
                    what: format!("FBX animation {name:?}"),
                    why: format!("baking failed ({e:?})"),
                });
                continue;
            }
        };
        let take = Take {
            basis,
            begin: stack.time_begin,
            end: stack.time_end,
        };
        let channels: Vec<glb::Channel> = baked
            .nodes
            .iter()
            .filter_map(|n| nodes.get(&n.element_id).map(|&node| (node, n)))
            .flat_map(|(node, n)| take.node_channels(node, n))
            .collect();
        if !channels.is_empty() {
            out.push(glb::Animation { name, channels });
        }
    }
    out
}

/// What every channel of one stack is converted with: the file's axes and the
/// take's range, in seconds.
struct Take {
    basis: Basis,
    begin: f64,
    end: f64,
}

impl Take {
    /// The translation, rotation and scale channels of one baked node.
    fn node_channels(&self, node: usize, baked: &ufbx::BakedNode) -> Vec<glb::Channel> {
        let b = self.basis;
        let t: Vec<_> = (baked.translation_keys.iter())
            .map(|k| (k.time, Vec3::from(b.vector(k.value))))
            .collect();
        let r: Vec<_> = (baked.rotation_keys.iter())
            .map(|k| (k.time, Quat::from_array(b.rotation(k.value))))
            .collect();
        let s: Vec<_> = (baked.scale_keys.iter())
            .map(|k| (k.time, Vec3::from(b.scale(k.value))))
            .collect();
        let lerp = |a: Vec3, b: Vec3, t: f64| a.lerp(b, t as f32);
        let slerp = |a: Quat, b: Quat, t: f64| a.slerp(b, t as f32);
        [
            self.channel(node, &t, lerp, |v| glb::Keys::Translation(arrays(v))),
            self.channel(node, &r, slerp, |v| glb::Keys::Rotation(arrays(v))),
            self.channel(node, &s, lerp, |v| glb::Keys::Scale(arrays(v))),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// One channel cut to the take, or `None` when ufbx baked no keys for it.
    fn channel<T: Copy>(
        &self,
        node: usize,
        keys: &[(f64, T)],
        lerp: impl Fn(T, T, f64) -> T,
        wrap: impl Fn(Vec<T>) -> glb::Keys,
    ) -> Option<glb::Channel> {
        let cut = range::clamp(keys, self.begin, self.end, lerp);
        if cut.is_empty() {
            return None;
        }
        let (times, values) = cut.into_iter().unzip();
        Some(glb::Channel {
            node,
            times,
            keys: wrap(values),
        })
    }
}

/// glam values as the arrays `glb` stores.
fn arrays<T: Into<[f32; N]>, const N: usize>(values: Vec<T>) -> Vec<[f32; N]> {
    values.into_iter().map(Into::into).collect()
}

#[cfg(test)]
mod mirrored;
#[cfg(test)]
mod tests;
