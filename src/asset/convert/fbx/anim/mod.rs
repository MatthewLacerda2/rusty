//! FBX animation stacks → glTF animations (#1081).
//!
//! `ufbx::bake_anim` does the hard half: it evaluates every layer of a stack,
//! resolves pivots, pre/post-rotations and Euler orders, and resamples the curves
//! into per-node translation, rotation and scale keys in the same space as
//! `Node::local_transform`. Here each key goes through the file's [`Basis`]
//! exactly like a node's rest transform (`C·M·C⁻¹`, per component), so a Z-up
//! file's clips play upright, and each stack becomes one glTF animation.

use super::basis::Basis;
use crate::asset::convert::glb;
use crate::asset::ImportError;
use crate::core::collections::Map;

/// One glTF animation per stack in `scene`, keying the nodes `nodes` maps (FBX
/// element id → glTF node index). A stack that keys none of them is dropped.
pub fn animations(
    scene: &ufbx::Scene,
    basis: Basis,
    nodes: &Map<u32, usize>,
) -> Result<Vec<glb::Animation>, ImportError> {
    let mut out = Vec::new();
    for (i, stack) in scene.anim_stacks.iter().enumerate() {
        let opts = ufbx::BakeOpts {
            // Clips start at zero, whatever time the stack was authored at.
            trim_start_time: true,
            ..Default::default()
        };
        let baked = ufbx::bake_anim(scene, &stack.anim, opts).map_err(|e| {
            ImportError::Parse(format!("baking animation {:?}: {e:?}", stack.element.name))
        })?;
        let channels: Vec<glb::Channel> = baked
            .nodes
            .iter()
            .filter_map(|n| nodes.get(&n.element_id).map(|&node| (node, n)))
            .flat_map(|(node, n)| channels(node, n, basis))
            .collect();
        if channels.is_empty() {
            continue;
        }
        let name = if stack.element.name.is_empty() {
            format!("clip_{i}")
        } else {
            stack.element.name.to_string()
        };
        out.push(glb::Animation { name, channels });
    }
    Ok(out)
}

/// The translation, rotation and scale channels of one baked node.
fn channels(node: usize, baked: &ufbx::BakedNode, basis: Basis) -> Vec<glb::Channel> {
    let mut out = Vec::new();
    let mut push = |times: Vec<f32>, keys| {
        if !times.is_empty() {
            out.push(glb::Channel { node, times, keys });
        }
    };
    let (t, r, s) = (
        &baked.translation_keys,
        &baked.rotation_keys,
        &baked.scale_keys,
    );
    push(
        t.iter().map(|k| k.time as f32).collect(),
        glb::Keys::Translation(t.iter().map(|k| basis.vector(k.value)).collect()),
    );
    push(
        r.iter().map(|k| k.time as f32).collect(),
        glb::Keys::Rotation(r.iter().map(|k| basis.rotation(k.value)).collect()),
    );
    push(
        s.iter().map(|k| k.time as f32).collect(),
        glb::Keys::Scale(s.iter().map(|k| basis.scale(k.value)).collect()),
    );
    out
}

#[cfg(test)]
mod tests;
