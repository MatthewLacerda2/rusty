//! src/render/gpu/lightmaps/warm.rs — landing page lists read on a worker, and a
//! scene load's lightmaps made ready before it is swapped in (#2123): the page lists
//! the incoming scene binds, loaded into the array cache without binding them, so its
//! first frame finds them there and is lit.
//!
//! **Landing and binding are independent.** Each caller lands only the lists it
//! names: a frame's [`Lightmaps::bind`] its bound lists, counted in that frame's
//! uploads; a warm-up's [`Lightmaps::warm`] the incoming scene's, counted in the
//! warm-up's. Whichever lands a list marks the cache fresh, and the next bind looks
//! again, so a list the warm-up landed (a retry of the same level, whose pages were
//! still being read) is bound by the frame that draws it, and the warm-up never takes
//! a landing from the scene still drawing (a baked splash that loads the next level
//! from `Start`).

use std::rc::Rc;
use std::sync::mpsc::TryRecvError;

use super::upload::LightmapArray;
use super::worker::finish;
use super::{Entry, Kind, Lightmaps};
use crate::render::stats::Uploads;

impl Lightmaps {
    /// Start loading (or, in line, load) the arrays of an incoming scene's colour
    /// `pages`, `directions` and `masks` without binding them, landing those that
    /// finished. Whether none is still being read: a list that failed to load counts
    /// as ready, as the frame that binds it would leave it out.
    pub(crate) fn warm(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lists: [&[String]; 3],
    ) -> bool {
        self.land(device, queue, lists);
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

    /// Upload each of `lists` whose worker finished, counting it in `uploads` and
    /// marking the cache fresh. A list no caller names stays pending, uploaded by
    /// the first that does.
    pub(super) fn land(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lists: [&[String]; 3],
    ) {
        for list in lists.into_iter().filter(|l| !l.is_empty()) {
            let Some(entry) = self.cache.get_mut(list) else {
                continue;
            };
            let Entry::Pending(inbox) = entry else {
                continue;
            };
            let result = match inbox.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Empty) => continue,
                // The loader died without an answer (it panicked): a failed load.
                Err(TryRecvError::Disconnected) => Err("the page loader stopped".into()),
            };
            let array = finish(device, queue, list, result);
            *entry = Entry::Ready(counted(&mut self.uploads, array));
            self.fresh = true;
        }
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

#[cfg(test)]
#[path = "warm_tests.rs"]
mod tests;
