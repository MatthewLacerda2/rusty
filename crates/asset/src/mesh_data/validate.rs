//! What the runtime's readers assume of an imported model, checked in one place: the
//! glTF import establishes each (a debug build asserts it after every import), and the
//! `model_import` fuzz target holds it over malformed files.

use super::{ImportedAsset, SkinData, SubMesh};
use crate::anim_data::{AnimationClip, Track};

impl ImportedAsset {
    /// The first invariant this asset breaks, named for its sub-mesh, clip or node.
    pub fn validate(&self) -> Result<(), String> {
        for sub in &self.sub_meshes {
            sub_mesh(sub, self.materials.len()).map_err(|e| format!("mesh '{}': {e}", sub.id))?;
        }
        for (i, node) in self.nodes.iter().enumerate() {
            if node.parent.is_some_and(|p| p >= i) {
                return Err(format!(
                    "node {i} ('{}') comes before its parent",
                    node.name
                ));
            }
        }
        Ok(())
    }
}

fn sub_mesh(sub: &SubMesh, materials: usize) -> Result<(), String> {
    let vertices = sub.vertices.len();
    if sub.indices.len() % 3 != 0 {
        return Err(format!(
            "{} indices are not whole triangles",
            sub.indices.len()
        ));
    }
    if let Some(i) = sub.indices.iter().find(|&&i| i as usize >= vertices) {
        return Err(format!("index {i} names one of {vertices} vertices"));
    }
    let mut covered = 0;
    for range in &sub.ranges {
        if range.indices.start != covered || range.indices.end < range.indices.start {
            return Err(format!(
                "material range {:?} does not follow {covered}",
                range.indices
            ));
        }
        if range.material.is_some_and(|m| m >= materials) {
            return Err(format!("material range names one of {materials} materials"));
        }
        covered = range.indices.end;
    }
    if covered as usize != sub.indices.len() {
        return Err(format!(
            "material ranges cover {covered} of {} indices",
            sub.indices.len()
        ));
    }
    let Some(skin) = &sub.skin else {
        return Ok(());
    };
    joints(skin)?;
    let count = skin.joint_nodes.len();
    for v in sub.vertices.iter() {
        let influences = v.joint_indices.iter().zip(&v.joint_weights);
        if let Some((j, _)) = influences
            .into_iter()
            .find(|(&j, &w)| w != 0.0 && j as usize >= count)
        {
            return Err(format!("a vertex weights joint {j} of {count}"));
        }
    }
    for clip in &sub.clips {
        clip_tracks(clip, count).map_err(|e| format!("clip '{}': {e}", clip.name))?;
    }
    Ok(())
}

fn joints(skin: &SkinData) -> Result<(), String> {
    let count = skin.joint_nodes.len();
    let lengths = [
        ("inverse-bind matrices", skin.inverse_bind.len()),
        ("bind globals", skin.bind_global.len()),
        ("local binds", skin.local_bind.len()),
        ("parents", skin.parents.len()),
        ("names", skin.names.len()),
    ];
    if let Some((what, len)) = lengths.iter().find(|(_, len)| *len != count) {
        return Err(format!("the skin has {len} {what} for {count} joints"));
    }
    let parents = skin.parents.iter().enumerate();
    if let Some((slot, p)) = parents
        .filter_map(|(s, p)| Some((s, (*p)?)))
        .find(|&(s, p)| p >= count || p == s)
    {
        return Err(format!(
            "joint {slot}'s parent {p} is not another of its {count} joints"
        ));
    }
    Ok(())
}

fn clip_tracks(clip: &AnimationClip, joints: usize) -> Result<(), String> {
    if clip.tracks.len() != joints {
        return Err(format!("{} tracks for {joints} joints", clip.tracks.len()));
    }
    for (slot, t) in clip.tracks.iter().enumerate() {
        let named = |e: String| format!("joint {slot}: {e}");
        track(&t.translation).map_err(|e| named(format!("translation {e}")))?;
        track(&t.rotation).map_err(|e| named(format!("rotation {e}")))?;
        track(&t.scale).map_err(|e| named(format!("scale {e}")))?;
    }
    Ok(())
}

fn track<T>(t: &Track<T>) -> Result<(), String> {
    if t.values.len() != t.times.len() {
        return Err(format!(
            "{} values for {} key times",
            t.values.len(),
            t.times.len()
        ));
    }
    if t.times.iter().any(|t| !t.is_finite()) || t.times.windows(2).any(|w| w[0] > w[1]) {
        return Err("key times are not finite and ascending".into());
    }
    Ok(())
}
