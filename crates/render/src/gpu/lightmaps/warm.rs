use crate::gpu::ops::Writes;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;

use super::upload::LightmapArray;
use super::worker::finish;
use super::{Entry, Kind, Lightmaps};
use crate::stats::Uploads;

impl Lightmaps {
    pub(crate) fn warm(
        &mut self,
        device: &wgpu::Device,
        writes: Writes,
        lists: [&[String]; 3],
    ) -> bool {
        self.land(device, writes, lists);
        let bc = device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let kinds = [Kind::Colour, Kind::Direction, Kind::Mask];
        for (list, kind) in lists.into_iter().zip(kinds) {
            self.array(device, writes, list, kind, bc);
        }
        let pending = |list: &[String]| matches!(self.cache.get(list), Some(Entry::Pending(_)));
        !lists.into_iter().any(pending)
    }

    pub(super) fn land(&mut self, device: &wgpu::Device, writes: Writes, lists: [&[String]; 3]) {
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
                Err(TryRecvError::Disconnected) => Err("the page loader stopped".into()),
            };
            let array = finish(device, writes, list, result);
            *entry = Entry::Ready(counted(&mut self.uploads, array));
            self.fresh = true;
        }
    }
}

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
