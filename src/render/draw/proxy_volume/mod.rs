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
//!
//! One camera's array is bound whole, so it never outgrows the device's storage
//! binding: a volume that would not fit is left out of that frame (its object takes
//! the single sample) and the console is told once ([`Frame::room`]).

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};

use crate::components::{LightProbeProxyVolumeComponent as Volume, ProxyGrid, ProxyQuality};
use crate::scene::lighting::probe::ProbeVolume;
use crate::scene::Scene;

/// vec4s before a volume's points: the world-to-box matrix's columns, then the
/// grid's dimensions as ordinary floats (1.0–32.0).
pub(crate) const HEADER: usize = 5;
/// vec4s per point: the L1 SH, one RGB coefficient each.
pub(crate) const COEFFS: usize = 4;

/// One camera's volumes as they upload: back to back, each [`pack`]ed.
#[derive(Default)]
pub(crate) struct Frame {
    /// The probe field's [`fingerprint`]; `None` when no entity carries a volume,
    /// and the frame packs nothing.
    probes: Option<u64>,
    /// The most vec4s `data` may hold: what one storage binding takes.
    room: usize,
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

/// Every volume's last samples, by entity, so a still object is not re-sampled,
/// and the probe field's fingerprint for this render.
#[derive(Default)]
pub(crate) struct ProxyVolumes {
    cache: BTreeMap<u32, (Key, Vec<[f32; 4]>)>,
    probes: Option<u64>,
    /// The bytes [`fingerprint`] hashes, kept so a render does not reallocate them.
    scratch: Vec<u8>,
    /// Whether the console was told a frame's volumes outgrew the binding.
    warned: bool,
}

impl ProxyVolumes {
    /// Start a render: forget the volumes no entity carries any more, and
    /// fingerprint the probe field once when any remain.
    pub(crate) fn begin_frame(&mut self, scene: &Scene) {
        let ids = scene.world.ids_with_light_probe_proxy_volume();
        self.cache.retain(|id, _| ids.contains(id));
        let scratch = &mut self.scratch;
        self.probes = (!ids.is_empty()).then(|| fingerprint(&scene.probes, scratch));
    }

    /// A camera's empty frame under this render's fingerprint, holding at most
    /// `max_bytes` (the device's storage-binding limit); it packs nothing before
    /// [`Self::begin_frame`] saw a volume.
    pub(crate) fn frame(&self, max_bytes: u64) -> Frame {
        Frame {
            probes: self.probes,
            room: room(max_bytes),
            data: Vec::new(),
        }
    }

    /// Append entity `id`'s volume to `frame` and return where it starts plus one,
    /// the instance's `proxy_volume`; `0` when it carries none, no probe covers the
    /// scene, its pose cannot be inverted, or the frame has no room left for it. `model` is its world matrix and
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
        if frame.data.len() + data.len() > frame.room {
            if !self.warned {
                self.warned = true;
                log::warn!(
                    "[Render] Light Probe Proxy Volumes outgrow one storage binding \
                     ({} bytes); the rest take one probe sample",
                    frame.room * 16
                );
            }
            return 0;
        }
        let start = frame.data.len() as u32 + 1;
        frame.data.extend_from_slice(data);
        start
    }
}

/// `volume`'s `grid` under world matrix `model`, sampled from `probes`, in the
/// layout the shader reads: the world-to-box matrix (the box `[0, 1]^3`), the
/// dimensions (as floats), then each point's L1 SH (bands 1–3 zeroed at `Low`).
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
    let [x, y, z] = grid.dims.map(|d| d as f32);
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

/// How many vec4s fit in a binding of `max_bytes`: the largest power of two
/// within it, since the buffer grows by powers of two and is bound whole.
pub(crate) fn room(max_bytes: u64) -> usize {
    let pow2 = if max_bytes == 0 {
        0
    } else {
        1u64 << (63 - max_bytes.leading_zeros())
    };
    usize::try_from(pow2 / 16).unwrap_or(usize::MAX)
}

/// A hash of everything a probe sample reads: each probe's position and SH, and
/// the grid. A bake, a moved probe or a new grid changes it, so every volume is
/// sampled again. The field's raw bytes go into `scratch` and through
/// `core::hash::content` (xxh3), once per render.
pub(crate) fn fingerprint(probes: &ProbeVolume, scratch: &mut Vec<u8>) -> u64 {
    scratch.clear();
    for p in &probes.probes {
        scratch.extend_from_slice(bytemuck::bytes_of(&p.position));
        scratch.extend_from_slice(bytemuck::cast_slice(&p.sh.coeffs));
    }
    if let Some(g) = probes.grid {
        scratch.extend_from_slice(bytemuck::bytes_of(&g.origin));
        scratch.extend_from_slice(bytemuck::bytes_of(&g.spacing));
        scratch.extend_from_slice(bytemuck::cast_slice(&g.counts));
    }
    scratch.extend_from_slice(&(probes.probes.len() as u64).to_le_bytes());
    crate::core::hash::content(scratch)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
