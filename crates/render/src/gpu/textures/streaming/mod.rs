//! Unity's mip streaming: a material's texture keeps the levels its use on screen needs.

mod plan;
mod reads;
mod shrink;

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::compressed::level_bytes;
use super::TextureKind;
use crate::render_texture::is_render_texture;
use crate::scene::residency::scene_maps;
use crate::{GpuTexture, Renderer};
use plan::Candidate;
use rusty_asset::MapRef;
use rusty_scene::Scene;

/// So a camera cut never re-makes every texture in one frame.
const UPGRADES_PER_FRAME: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mips {
    pub full_size: (u32, u32),
    pub full: u32,
    pub dropped: u32,
    pub most: u32,
    pub kind: TextureKind,
}

impl Mips {
    pub(crate) fn of(texture: &wgpu::Texture, kind: TextureKind) -> Option<Self> {
        let (size, full) = (texture.size(), texture.mip_level_count());
        let most = match texture.format().block_dimensions() {
            (1, 1) => full - 1,
            _ => rusty_asset::texture_import::max_skip(size.width, size.height, full),
        };
        (most > 0).then_some(Self {
            full_size: (size.width, size.height),
            full,
            dropped: 0,
            most,
            kind,
        })
    }

    /// Each level's bytes in the chain the file holds, mip 0 first, whatever is resident.
    pub(crate) fn level_bytes(&self, format: wgpu::TextureFormat) -> Vec<u64> {
        let size = super::extent(self.full_size.0, self.full_size.1);
        let at = |mip| size.mip_level_size(mip, wgpu::TextureDimension::D2);
        (0..self.full)
            .map(|mip| level_bytes(format, at(mip)))
            .collect()
    }

    fn dropping(self, by: u32) -> Self {
        Self {
            dropped: self.dropped + by,
            ..self
        }
    }
}

pub(crate) struct MipStreaming {
    /// Headless only when asked: a golden never depends on read timing.
    pub(crate) allowed: bool,
    /// Per material group: the largest share of the view's height it covered.
    seen: Vec<f32>,
    reads: Option<reads::Reads>,
    warned: bool,
}

impl MipStreaming {
    pub(crate) fn new(allowed: bool) -> Self {
        Self {
            allowed,
            seen: Vec::new(),
            reads: None,
            warned: false,
        }
    }

    pub(crate) fn see(&mut self, group: usize, share: f32) {
        if self.seen.len() <= group {
            self.seen.resize(group + 1, 0.0);
        }
        self.seen[group] = self.seen[group].max(share);
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.reads.as_ref().map_or(0, |r| r.in_flight)
    }
}

impl Renderer {
    pub fn stream_textures(&mut self, allowed: bool) {
        self.streaming.allowed = allowed;
    }

    pub(crate) fn streams_textures(&self) -> bool {
        self.streaming.allowed && self.quality.texture_streaming.active
    }

    pub(crate) fn stream_mips(&mut self, scene: &Scene, height: u32) {
        if self.is_aside() {
            return;
        }
        let seen = std::mem::take(&mut self.streaming.seen);
        let active = self.streams_textures();
        self.install_reads();
        let pixels = self.seen_pixels(&seen, height as f32);
        let (maps, mut candidates) = self.candidates(scene);
        if maps.is_empty() {
            return;
        }
        for (candidate, map) in candidates.iter_mut().zip(&maps) {
            candidate.pixels = pixels.get(map.as_str()).copied();
        }
        let plan = match active {
            true => {
                let streamed: u64 = candidates.iter().map(|c| c.bytes(c.dropped)).sum();
                let all: u64 = self
                    .resident_textures()
                    .map(super::compressed::gpu_bytes)
                    .sum();
                let budget = u64::from(self.quality.texture_streaming.budget_mb) << 20;
                plan::plan(&candidates, all - streamed, Some(budget))
            }
            false => plan::Plan {
                targets: vec![0; candidates.len()],
                over: false,
            },
        };
        if plan.over && !std::mem::replace(&mut self.streaming.warned, true) {
            log::warn!(
                "[Render] the textures pass the quality preset's streaming budget ({} MB) with \
                 every streamed one at its smallest: raise texture_streaming.budget_mb",
                self.quality.texture_streaming.budget_mb
            );
        }
        let mut upgrades = Vec::new();
        for ((map, candidate), target) in maps.iter().zip(&candidates).zip(plan.targets) {
            match target.cmp(&candidate.dropped) {
                std::cmp::Ordering::Greater => self.drop_levels(map, target - candidate.dropped),
                std::cmp::Ordering::Less => upgrades.push((candidate.pixels, map, target)),
                std::cmp::Ordering::Equal => {}
            }
        }
        upgrades.sort_by(|a, b| b.0.unwrap_or(0.0).total_cmp(&a.0.unwrap_or(0.0)));
        for (_, map, target) in upgrades {
            self.ask_read(map, target);
        }
    }

    fn seen_pixels(&self, seen: &[f32], height: f32) -> BTreeMap<String, f32> {
        let mut pixels: BTreeMap<String, f32> = BTreeMap::new();
        let worn = seen.iter().enumerate().filter(|(_, &s)| s > 0.0);
        for (group, &share) in worn {
            let Some(paths) = self.materials.paths_of(group) else {
                continue;
            };
            for map in paths.iter().flatten() {
                let at = pixels.entry(map.as_str().to_owned()).or_default();
                *at = at.max(share * height);
            }
        }
        pixels
    }

    /// A material's, never one a UI image, particle, the sky or a second upload binds.
    fn candidates(&self, scene: &Scene) -> (Vec<MapRef>, Vec<Candidate>) {
        let mut others: BTreeSet<MapRef> = scene_maps(scene).collect();
        others.insert(MapRef::from(self.skybox_path.as_str()));
        others.extend(self.texture_fallbacks.keys().cloned());
        let streamed = self.gpu_textures.iter().filter_map(|(map, tex)| {
            let mips = tex.mips?;
            let other = others.contains(map) || is_render_texture(map.as_str());
            (!other).then(|| (map.clone(), candidate(tex, mips)))
        });
        let mut both: Vec<_> = streamed.collect();
        both.sort_by(|a, b| a.0.cmp(&b.0));
        both.into_iter().unzip()
    }

    fn drop_levels(&mut self, map: &MapRef, by: u32) {
        let Some(tex) = self.gpu_textures.get(map) else {
            return;
        };
        let made = self.shrunk(tex, by);
        self.replace_texture(map, made);
    }

    fn ask_read(&mut self, map: &MapRef, skip: u32) {
        let Some(tex) = self.gpu_textures.get(map) else {
            return;
        };
        let Some(kind) = tex.mips.map(|m| m.kind) else {
            return;
        };
        let of = Rc::downgrade(tex);
        let features = self.device.features();
        let reads = self
            .streaming
            .reads
            .get_or_insert_with(|| reads::Reads::new(features));
        reads.ask(map, kind, (skip, of));
    }

    fn install_reads(&mut self) {
        for _ in 0..UPGRADES_PER_FRAME {
            let Some((done, of)) = self.streaming.reads.as_mut().and_then(|r| r.take()) else {
                return;
            };
            let current = self.gpu_textures.get(&done.0);
            if current
                .zip(of)
                .is_some_and(|(now, of)| of.ptr_eq(&Rc::downgrade(now)))
            {
                self.install_read(done);
            }
        }
    }

    fn install_read(&mut self, (map, skip, decoded): reads::Done) {
        let Some(mips) = self.gpu_textures.get(&map).and_then(|t| t.mips) else {
            return;
        };
        let Some(decoded) = decoded.filter(|_| skip < mips.dropped) else {
            return;
        };
        let read = self.upload_texture(&map, mips.kind, decoded);
        let levels = read.texture.mip_level_count();
        if levels > mips.full || read.texture.format() != self.gpu_textures[&map].texture.format() {
            return; // the file changed: its freshness poll re-loads it
        }
        let kept = Mips {
            dropped: mips.full - levels,
            ..mips
        };
        let read = GpuTexture {
            mips: Some(kept),
            ..read
        };
        self.uploads
            .add(super::compressed::gpu_bytes(&read.texture));
        let made = match skip > kept.dropped {
            true => self.shrunk(&read, skip - kept.dropped),
            false => read,
        };
        self.replace_texture(&map, made);
    }

    fn replace_texture(&mut self, map: &MapRef, made: GpuTexture) {
        let variance = made.normal_variance.is_some();
        self.gpu_textures.insert(map.clone(), Rc::new(made));
        for (group, paths, owner) in self.materials.binding(map.as_str()) {
            let maps = self.group_maps(&paths);
            let bound = self.material_bind_group(&maps, owner.as_ref());
            self.materials.replace(group, bound);
        }
        if variance {
            self.rewrite_records_wearing(map.as_str());
        }
    }
}

fn candidate(tex: &GpuTexture, mips: Mips) -> Candidate {
    let (w, h) = mips.full_size;
    Candidate {
        side: w.max(h),
        dropped: mips.dropped,
        most: mips.most,
        pixels: None,
        levels: mips.level_bytes(tex.texture.format()),
    }
}

#[cfg(test)]
mod plan_tests;

#[cfg(test)]
pub(super) mod gpu_tests;

#[cfg(test)]
mod budget_gpu_tests;

#[cfg(test)]
mod compressed_gpu_tests;

#[cfg(test)]
mod reads_tests;

#[cfg(test)]
mod audit_gpu_tests;
