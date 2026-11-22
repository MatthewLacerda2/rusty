mod keep;
mod load;
mod upload;
mod warm;
mod worker;

use crate::gpu::ops::Writes;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

use load::{Kind, Loaded};
use rusty_core::fs::written::{written_since, Stale};
use upload::{create, LightmapArray};
use worker::{finish, start};

enum Entry {
    Ready(Option<Rc<LightmapArray>>),
    Pending(Receiver<Result<Loaded, String>>),
}

pub(crate) struct Lightmaps {
    bound: [Vec<String>; 3],
    current: Option<Rc<LightmapArray>>,
    directions: Option<Rc<LightmapArray>>,
    masks: Option<Rc<LightmapArray>>,
    cache: HashMap<Vec<String>, Entry>,
    fallback: LightmapArray,
    seen: u64,
    in_line: bool,
    pub(crate) uploads: crate::stats::Uploads,
    fresh: bool,
}

impl Lightmaps {
    pub(crate) fn new(device: &wgpu::Device, in_line: bool) -> Self {
        let fallback = create(device, wgpu::TextureFormat::Rgba8Unorm, 1, 1, 1);
        Self {
            bound: Default::default(),
            current: None,
            directions: None,
            masks: None,
            cache: HashMap::new(),
            fallback,
            seen: written_since(u64::MAX).0,
            in_line,
            uploads: Default::default(),
            fresh: false,
        }
    }

    pub(crate) fn bind(
        &mut self,
        device: &wgpu::Device,
        writes: Writes,
        pages: &[String],
        directions: &[String],
        masks: &[String],
    ) -> bool {
        let rewritten = self.drop_rewritten();
        let lists = [pages, directions, masks];
        self.land(device, writes, lists);
        let fresh = std::mem::take(&mut self.fresh);
        let same = self.bound.iter().zip(lists).all(|(b, l)| b == l);
        if same && !rewritten && !fresh {
            return false;
        }
        self.bound = lists.map(|l| l.to_vec());
        let bc = device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let current = self.array(device, writes, pages, Kind::Colour, bc);
        let shape = current.as_ref().map(|a| a.shape);
        let directions = self
            .array(device, writes, directions, Kind::Direction, bc)
            .filter(|d| Some(d.shape) == shape);
        let masks = self
            .array(device, writes, masks, Kind::Mask, bc)
            .filter(|m| Some(m.shape) == shape);
        let changed = !same_array(&current, &self.current)
            || !same_array(&directions, &self.directions)
            || !same_array(&masks, &self.masks);
        (self.current, self.directions, self.masks) = (current, directions, masks);
        changed || !same
    }

    fn drop_rewritten(&mut self) -> bool {
        let (seen, paths) = written_since(self.seen);
        self.seen = seen;
        let stale = Stale::new(paths);
        if stale.is_empty() {
            return false;
        }
        let touches = |list: &[String]| list.iter().any(|p| stale.matches(p));
        self.cache.retain(|list, _| !touches(list));
        self.bound.iter().any(|list| touches(list))
    }

    fn array(
        &mut self,
        device: &wgpu::Device,
        writes: Writes,
        pages: &[String],
        kind: Kind,
        bc: bool,
    ) -> Option<Rc<LightmapArray>> {
        if pages.is_empty() {
            return None;
        }
        let (in_line, uploads) = (self.in_line, &mut self.uploads);
        let entry = self.cache.entry(pages.to_vec()).or_insert_with(|| {
            let list = pages.to_vec();
            match in_line {
                true => Entry::Ready(warm::counted(
                    uploads,
                    finish(device, writes, &list, load::load(&list, kind, bc)),
                )),
                false => Entry::Pending(start(list, kind, bc)),
            }
        });
        match entry {
            Entry::Ready(array) => array.clone(),
            Entry::Pending(_) => None,
        }
    }

    pub(crate) fn read_in_line(&mut self, in_line: bool) {
        self.in_line = in_line;
    }

    pub(crate) fn resident(&self) -> bool {
        self.current.is_some()
    }

    #[cfg(test)]
    pub(crate) fn bound_edge(&self) -> Option<u32> {
        self.current.as_ref().map(|a| a.shape.0)
    }

    #[cfg(test)]
    pub(crate) fn bound_format(&self) -> Option<(wgpu::TextureFormat, u32)> {
        let t = &self.current.as_ref()?.texture;
        Some((t.format(), t.mip_level_count()))
    }

    pub(crate) fn gpu_bytes(&self) -> u64 {
        let cached = self.cache.values().filter_map(|e| match e {
            Entry::Ready(Some(a)) => Some(a),
            _ => None,
        });
        let cached: Vec<&Rc<LightmapArray>> = cached.collect();
        let bound = [&self.current, &self.directions, &self.masks];
        let bound = bound.into_iter().flatten();
        let uncached = bound.filter(|b| !cached.iter().any(|c| Rc::ptr_eq(c, b)));
        cached
            .iter()
            .copied()
            .chain(uncached)
            .map(|a| a.bytes)
            .sum()
    }

    pub(crate) fn directional(&self) -> bool {
        self.directions.is_some()
    }

    pub(crate) fn masked(&self) -> bool {
        self.masks.is_some()
    }

    pub(crate) fn binding(&self) -> LightmapBinding<'_> {
        let array = self.current.as_deref().unwrap_or(&self.fallback);
        let directions = self.directions.as_deref().unwrap_or(&self.fallback);
        let masks = self.masks.as_deref().unwrap_or(&self.fallback);
        LightmapBinding {
            pages: &array.view,
            directions: &directions.view,
            masks: &masks.view,
        }
    }
}

pub(crate) struct LightmapBinding<'a> {
    pub pages: &'a wgpu::TextureView,
    pub directions: &'a wgpu::TextureView,
    pub masks: &'a wgpu::TextureView,
}

fn same_array(a: &Option<Rc<LightmapArray>>, b: &Option<Rc<LightmapArray>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod formats_tests;
#[cfg(test)]
mod limits_tests;
#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod tests;
