//! Baked lightmaps on the GPU (#438, #1694): the scene's atlas pages as one 2D
//! texture array at group 0, binding 14, with a clamped trilinear sampler at 15.
//! Each lightmapped instance carries its page and the scale/offset into it, so
//! lightmapped copies of a prop still draw as one instanced call (Unity's lightmap
//! index + scale/offset).
//!
//! **What is uploaded** (`load`): a `.ktx2` page as stored, BC6H irradiance and a
//! BC7 direction page with their mips, when the GPU has BC compression; else (or
//! for a `.png` page of a scene baked before #1694) the RGBM PNG beside it, decoded
//! on the CPU to linear `Rgba16Float` with the same mips. Either way the shader
//! reads linear light and picks its level from the UV's screen derivatives.
//!
//! **Where it loads.** A page list is read on a worker thread in a windowed
//! renderer, so a scene load never stalls a frame on disk; the frames before it
//! lands draw lightmapped meshes with probe / ambient light, as a page that failed
//! to load does, and the worker wakes a frame when it is done. A headless
//! renderer (captures, the harness, tests) reads it in line, so its first frame
//! is lit and a capture never depends on how fast a disk is.
//!
//! An array is built once per distinct page list and cached, so two scenes
//! alternating in one frame cost a lookup, not a reload. A rebake writes changed
//! pages under new names (their file name hashes the page), so the cache rarely
//! meets a stale page; when the engine does re-write one (or writes one a page
//! list failed on), the write log (`core::fs::written`, #1207) drops every array
//! holding it and rebinds. A page that fails to load, or pages of different sizes,
//! leave the scene unlit by lightmaps until the engine writes one of its pages.
//!
//! A directional bake (#810) adds its direction pages as a second array at binding 16,
//! read through the same sampler with the same page index and UV. They are bound only
//! beside a bound colour array whose page count and size they match; otherwise the
//! zeroed fallback stands in, whose zero directionality leaves the lightmap as is.

mod load;
mod upload;

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use crate::core::fs::written::{written_since, Stale};
use load::{Kind, Loaded};
use upload::{create, upload, LightmapArray};

/// A page list's array: uploaded (or failed), or still being read.
enum Entry {
    Ready(Option<Rc<LightmapArray>>),
    Pending(Receiver<Result<Loaded, String>>),
}

/// What group 0 binds for lightmaps, and the arrays built so far.
pub(crate) struct Lightmaps {
    /// The colour and direction page lists bound now (empty: the fallback).
    bound: (Vec<String>, Vec<String>),
    current: Option<Rc<LightmapArray>>,
    directions: Option<Rc<LightmapArray>>,
    /// Arrays by page list, colour and direction alike.
    cache: HashMap<Vec<String>, Entry>,
    /// A 1×1×1 black array, bound when the scene has no lightmaps.
    fallback: LightmapArray,
    /// The newest write generation already applied to `cache`.
    seen: u64,
    /// Read page lists in line rather than on a worker: a headless renderer.
    in_line: bool,
}

impl Lightmaps {
    /// No lightmaps bound yet. The fallback needs no upload: a new texture is zeroed,
    /// and zero light is black. `in_line` reads pages on the calling thread.
    pub(crate) fn new(device: &wgpu::Device, in_line: bool) -> Self {
        let fallback = create(device, wgpu::TextureFormat::Rgba8Unorm, 1, 1, 1);
        Self {
            bound: (Vec::new(), Vec::new()),
            current: None,
            directions: None,
            cache: HashMap::new(),
            fallback,
            seen: written_since(u64::MAX).0,
            in_line,
        }
    }

    /// Bind the arrays of `pages` and their `directions` (#810; empty for a
    /// non-directional bake), starting each one's load on first sight. Returns
    /// whether the binding changed, so the caller rebuilds group 0.
    pub(crate) fn bind(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pages: &[String],
        directions: &[String],
    ) -> bool {
        let rewritten = self.drop_rewritten();
        let landed = self.land(device, queue);
        let same = self.bound.0 == pages && self.bound.1 == directions;
        if same && !rewritten && !landed {
            return false;
        }
        self.bound = (pages.to_vec(), directions.to_vec());
        let bc = device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let current = self.array(device, queue, pages, Kind::Colour, bc);
        let shape = current.as_ref().map(|a| a.shape);
        let directions = self
            .array(device, queue, directions, Kind::Direction, bc)
            .filter(|d| Some(d.shape) == shape);
        let changed =
            !same_array(&current, &self.current) || !same_array(&directions, &self.directions);
        (self.current, self.directions) = (current, directions);
        changed || !same
    }

    /// Upload every page list whose worker finished. Returns whether one did.
    fn land(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> bool {
        let mut landed = false;
        for (list, entry) in &mut self.cache {
            let Entry::Pending(inbox) = entry else {
                continue;
            };
            let result = match inbox.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => continue,
                // The loader died without an answer (it panicked): a failed load.
                Err(TryRecvError::Disconnected) => Err("the page loader stopped".into()),
            };
            *entry = Entry::Ready(finish(device, queue, list, result));
            landed = true;
        }
        landed
    }

    /// Forget every array (a failed or pending one included) holding a page the
    /// engine wrote since the last bind. Returns whether a bound list was among
    /// them, so the caller rebinds even though the lists are unchanged.
    fn drop_rewritten(&mut self) -> bool {
        let (seen, paths) = written_since(self.seen);
        self.seen = seen;
        let stale = Stale::new(paths);
        if stale.is_empty() {
            return false;
        }
        let touches = |list: &[String]| list.iter().any(|p| stale.matches(p));
        self.cache.retain(|list, _| !touches(list));
        touches(&self.bound.0) || touches(&self.bound.1)
    }

    /// The (cached) array of `pages`; `None` for no pages, one that won't load, or
    /// one still loading.
    fn array(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pages: &[String],
        kind: Kind,
        bc: bool,
    ) -> Option<Rc<LightmapArray>> {
        if pages.is_empty() {
            return None;
        }
        let in_line = self.in_line;
        let entry = self.cache.entry(pages.to_vec()).or_insert_with(|| {
            let list = pages.to_vec();
            match in_line {
                true => Entry::Ready(finish(device, queue, &list, load::load(&list, kind, bc))),
                false => Entry::Pending(start(list, kind, bc)),
            }
        });
        match entry {
            Entry::Ready(array) => array.clone(),
            Entry::Pending(_) => None,
        }
    }

    /// Read page lists on the calling thread (`true`: a headless renderer) or on a
    /// worker.
    pub(crate) fn read_in_line(&mut self, in_line: bool) {
        self.in_line = in_line;
    }

    /// Whether a lightmap array is bound: when not, instances must not read it.
    pub(crate) fn resident(&self) -> bool {
        self.current.is_some()
    }

    /// The bound colour array's page edge, if one is bound.
    #[cfg(test)]
    pub(crate) fn bound_edge(&self) -> Option<u32> {
        self.current.as_ref().map(|a| a.shape.0)
    }

    /// The bound colour array's format and levels, if one is bound.
    #[cfg(test)]
    pub(crate) fn bound_format(&self) -> Option<(wgpu::TextureFormat, u32)> {
        let t = &self.current.as_ref()?.texture;
        Some((t.format(), t.mip_level_count()))
    }

    /// The bytes every array held takes on the GPU: the bound ones and every page
    /// list cached beside them. A rebake's old pages are freed when it deletes
    /// them (`core::fs::written`, which `drop_rewritten` reads).
    pub(crate) fn gpu_bytes(&self) -> u64 {
        let cached = self.cache.values().filter_map(|e| match e {
            Entry::Ready(Some(a)) => Some(a),
            _ => None,
        });
        let cached: Vec<&Rc<LightmapArray>> = cached.collect();
        let bound = [&self.current, &self.directions].into_iter().flatten();
        let uncached = bound.filter(|b| !cached.iter().any(|c| Rc::ptr_eq(c, b)));
        cached
            .iter()
            .copied()
            .chain(uncached)
            .map(|a| a.bytes)
            .sum()
    }

    /// Whether direction pages are bound beside it (#810).
    pub(crate) fn directional(&self) -> bool {
        self.directions.is_some()
    }

    /// The bound colour and direction views and their sampler (the fallback's when
    /// none is).
    pub(crate) fn binding(&self) -> (&wgpu::TextureView, &wgpu::TextureView, &wgpu::Sampler) {
        let array = self.current.as_deref().unwrap_or(&self.fallback);
        let directions = self.directions.as_deref().unwrap_or(&self.fallback);
        (&array.view, &directions.view, &array.sampler)
    }
}

/// Whether two bindings are the same array (or both none).
fn same_array(a: &Option<Rc<LightmapArray>>, b: &Option<Rc<LightmapArray>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// Read `list` on a worker; it wakes a frame when it is done.
fn start(list: Vec<String>, kind: Kind, bc: bool) -> Receiver<Result<Loaded, String>> {
    let (outbox, inbox) = channel();
    let spawned = std::thread::Builder::new()
        .name("lightmap-load".into())
        .spawn(move || {
            // A renderer gone meanwhile has dropped the inbox: nothing to tell.
            let _ = outbox.send(load::load(&list, kind, bc));
            crate::core::wake::frame();
        });
    if let Err(e) = spawned {
        log::warn!("[Lightmap] cannot start the page loader: {e}");
    }
    inbox
}

/// Upload a finished load, or warn of why it failed.
fn finish(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    list: &[String],
    result: Result<Loaded, String>,
) -> Option<Rc<LightmapArray>> {
    match result {
        Ok(loaded) => Some(Rc::new(upload(device, queue, &loaded))),
        Err(e) => {
            log::warn!("[Lightmap] cannot load the pages {list:?}: {e}");
            None
        }
    }
}

#[cfg(test)]
mod formats_tests;
#[cfg(test)]
mod tests;
