use std::collections::BTreeMap;

use glam::{Mat4, Vec3};

use rusty_components::{LightProbeProxyVolumeComponent as Volume, ProxyGrid, ProxyQuality};
use rusty_scene::lighting::probe::ProbeVolume;
use rusty_scene::Scene;

/// In vec4s, as `forward/proxy_volume.wgsl`: the box matrix's 4 columns, the dims, then 4 L1 SH a
/// point.
pub(crate) const HEADER: usize = 5;
pub(crate) const COEFFS: usize = 4;

#[derive(Default)]
pub(crate) struct Frame {
    probes: Option<u64>,
    room: usize,
    pub data: Vec<[f32; 4]>,
}

#[derive(PartialEq)]
struct Key {
    model: [f32; 16],
    volume: Volume,
    bounds: [[f32; 3]; 2],
    probes: u64,
}

#[derive(Default)]
pub(crate) struct ProxyVolumes {
    cache: BTreeMap<u32, (Key, Vec<[f32; 4]>)>,
    probes: Option<u64>,
    scratch: Vec<u8>,
    warned: bool,
}

impl ProxyVolumes {
    pub(crate) fn begin_frame(&mut self, scene: &Scene) {
        let ids = scene.world.ids_with_light_probe_proxy_volume();
        self.cache.retain(|id, _| ids.contains(id));
        let scratch = &mut self.scratch;
        self.probes = (!ids.is_empty()).then(|| fingerprint(&scene.probes, scratch));
    }

    pub(crate) fn frame(&self, max_bytes: u64) -> Frame {
        Frame {
            probes: self.probes,
            room: room(max_bytes),
            data: Vec::new(),
        }
    }

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

/// Vec4s (16 bytes) in the largest power of two within `max_bytes`: the buffer grows by doubling,
/// bound whole.
pub(crate) fn room(max_bytes: u64) -> usize {
    let pow2 = if max_bytes == 0 {
        0
    } else {
        1u64 << (63 - max_bytes.leading_zeros())
    };
    usize::try_from(pow2 / 16).unwrap_or(usize::MAX)
}

pub(crate) fn fingerprint(probes: &ProbeVolume, scratch: &mut Vec<u8>) -> u64 {
    scratch.clear();
    for p in probes.probes() {
        scratch.extend_from_slice(bytemuck::bytes_of(&p.position));
        scratch.extend_from_slice(bytemuck::cast_slice(&p.sh.coeffs));
    }
    if let Some(g) = probes.grid() {
        scratch.extend_from_slice(bytemuck::bytes_of(&g.origin));
        scratch.extend_from_slice(bytemuck::bytes_of(&g.spacing));
        scratch.extend_from_slice(bytemuck::cast_slice(&g.counts));
    }
    scratch.extend_from_slice(&(probes.probes().len() as u64).to_le_bytes());
    rusty_core::hash::content(scratch)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
