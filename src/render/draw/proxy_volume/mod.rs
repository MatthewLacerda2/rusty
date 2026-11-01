//! src/render/draw/proxy_volume/ — Light Probe Proxy Volumes on the GPU (#1672).
//!
//! A mesh entity with a `LightProbeProxyVolumeComponent` gets its probe light from
//! a grid of probe-field samples over its box, interpolated per pixel by
//! `engine/shaders/forward/proxy_volume.wgsl`, instead of the one sample at its
//! origin (`uniforms::entity_probe_sh`). Each camera's solids append their volumes
//! to one storage array (group 1, binding 5) and point their instance at theirs.
//!
//! [`pack`] samples the grid exactly as the single sample is taken
//! (`ProbeVolume::sample`, trilinear over the probe grid), so a volume holds what
//! the object would read standing at each of its points. A volume is sampled again
//! only when its entity's pose, its settings, its mesh's bounds or the probe field
//! changed ([`ProxyVolumes::push`]): Unity's *Automatic* refresh, every frame for a
//! moving object, never for one standing still. Nothing here is saved: the scene
//! keeps the component's values, never its samples.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};

use crate::components::{LightProbeProxyVolumeComponent as Volume, ProxyGrid, ProxyQuality};
use crate::scene::lighting::probe::ProbeVolume;
use crate::scene::Scene;

/// vec4s before a volume's points: the world-to-box matrix's columns, then the
/// grid's dimensions.
pub(crate) const HEADER: usize = 5;
/// vec4s per point: the L1 SH, one RGB coefficient each.
pub(crate) const COEFFS: usize = 4;

/// One camera's volumes as they upload: back to back, each [`pack`]ed.
#[derive(Default)]
pub(crate) struct Frame {
    /// The probe field's [`fingerprint`]; `None` when no entity carries a volume,
    /// and the frame packs nothing.
    probes: Option<u64>,
    pub data: Vec<[f32; 4]>,
}

/// What a cached volume was sampled under.
#[derive(PartialEq)]
struct Key {
    model: [f32; 16],
    volume: Volume,
    bounds: [[f32; 3]; 2],
    probes: u64,
}

/// Every volume's last samples, by entity, so a still object is not re-sampled.
#[derive(Default)]
pub(crate) struct ProxyVolumes {
    cache: BTreeMap<u32, (Key, Vec<[f32; 4]>)>,
}

impl ProxyVolumes {
    /// Start a camera's frame: forget the volumes no entity carries any more, and
    /// fingerprint the probe field once when any remain.
    pub(crate) fn begin(&mut self, scene: &Scene) -> Frame {
        let ids = scene.world.ids_with_light_probe_proxy_volume();
        self.cache.retain(|id, _| ids.contains(id));
        Frame {
            probes: (!ids.is_empty()).then(|| fingerprint(&scene.probes)),
            data: Vec::new(),
        }
    }

    /// Append entity `id`'s volume to `frame` and return where it starts plus one,
    /// the instance's `proxy_volume`; `0` when it carries none, no probe covers the
    /// scene, or its pose cannot be inverted. `model` is its world matrix and
    /// `bounds` its mesh's local bounds.
    pub(crate) fn push(
        &mut self,
        scene: &Scene,
        id: u32,
        (model, bounds): (Mat4, (Vec3, Vec3)),
        frame: &mut Frame,
    ) -> u32 {
        let Some(probes) = frame.probes else {
            return 0;
        };
        let Some(volume) = scene.world.light_probe_proxy_volume(id).map(|v| v.clone()) else {
            return 0;
        };
        let key = Key {
            model: model.to_cols_array(),
            bounds: [bounds.0.to_array(), bounds.1.to_array()],
            volume,
            probes,
        };
        let fresh = self.cache.get(&id).is_some_and(|(k, _)| *k == key);
        if !fresh {
            let grid = key.volume.grid(bounds);
            match pack(&key.volume, &grid, model, &scene.probes) {
                Some(data) => {
                    self.cache.insert(id, (key, data));
                }
                None => {
                    self.cache.remove(&id);
                    return 0;
                }
            }
        }
        let Some((_, data)) = self.cache.get(&id) else {
            return 0;
        };
        let start = frame.data.len() as u32 + 1;
        frame.data.extend_from_slice(data);
        start
    }
}

/// `volume`'s `grid` under world matrix `model`, sampled from `probes`, in the
/// layout the shader reads: the world-to-box matrix (the box `[0, 1]^3`), the
/// dimensions (u32 bits), then each point's L1 SH (bands 1–3 zeroed at `Low`).
/// `None` when no probe exists or `model` cannot be inverted (a zero scale).
pub(crate) fn pack(
    volume: &Volume,
    grid: &ProxyGrid,
    model: Mat4,
    probes: &ProbeVolume,
) -> Option<Vec<[f32; 4]>> {
    if probes.is_empty() || model.determinant().abs() < 1e-12 {
        return None;
    }
    let to_box =
        Mat4::from_scale(grid.size.recip()) * Mat4::from_translation(-grid.min) * model.inverse();
    if !to_box.is_finite() {
        return None;
    }
    let bands = match volume.quality {
        ProxyQuality::Low => 1,
        ProxyQuality::Normal => COEFFS,
    };
    let mut out = Vec::with_capacity(HEADER + grid.len() * COEFFS);
    out.extend(to_box.to_cols_array_2d());
    let [x, y, z] = grid.dims.map(f32::from_bits);
    out.push([x, y, z, 0.0]);
    for point in grid.points() {
        let sh = probes.sample(model.transform_point3(point))?;
        for (band, c) in sh.coeffs.iter().take(COEFFS).enumerate() {
            let c = if band < bands { *c } else { [0.0; 3] };
            out.push([c[0], c[1], c[2], 0.0]);
        }
    }
    Some(out)
}

/// A hash of everything a probe sample reads: each probe's position and SH, and
/// the grid. A bake, a moved probe or a new grid changes it, so every volume is
/// sampled again (FNV-1a over the bits: deterministic, and cheap next to sampling).
pub(crate) fn fingerprint(probes: &ProbeVolume) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: f32| {
        h ^= u64::from(v.to_bits());
        h = h.wrapping_mul(0x0100_0000_01b3);
    };
    for p in &probes.probes {
        p.position.to_array().into_iter().for_each(&mut eat);
        p.sh.coeffs.iter().flatten().copied().for_each(&mut eat);
    }
    if let Some(g) = &probes.grid {
        g.origin.to_array().into_iter().for_each(&mut eat);
        g.spacing.to_array().into_iter().for_each(&mut eat);
        g.counts.iter().for_each(|&n| eat(f32::from_bits(n)));
    }
    h
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
