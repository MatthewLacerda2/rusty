//! src/render/gpu/lightmaps/warm.rs — a scene load's lightmaps made ready before it
//! is swapped in (#2123): the page lists the incoming scene binds, loaded into the
//! array cache without binding them, so its first frame finds them there and is lit.

use std::rc::Rc;

use super::upload::LightmapArray;
use super::{Entry, Kind, Lightmaps};
use crate::render::stats::Uploads;

impl Lightmaps {
    /// Start loading (or, in line, load) the arrays of an incoming scene's colour
    /// `pages`, `directions` and `masks` without binding them, landing any that
    /// finished. Whether none is still being read: a list that failed to load counts
    /// as ready, as the frame that binds it would leave it out.
    pub(crate) fn warm(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lists: [&[String]; 3],
    ) -> bool {
        self.land(device, queue);
        let bc = device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let kinds = [Kind::Colour, Kind::Direction, Kind::Mask];
        for (list, kind) in lists.into_iter().zip(kinds) {
            self.array(device, queue, list, kind, bc);
        }
        let pending = |list: &[String]| matches!(self.cache.get(list), Some(Entry::Pending(_)));
        !lists.into_iter().any(pending)
    }
}

/// `array`, counted in `uploads` when there is one.
pub(super) fn counted(
    uploads: &mut Uploads,
    array: Option<Rc<LightmapArray>>,
) -> Option<Rc<LightmapArray>> {
    array.iter().for_each(|a| uploads.add(a.bytes));
    array
}
