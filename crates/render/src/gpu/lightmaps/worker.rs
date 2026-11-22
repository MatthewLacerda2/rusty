use crate::gpu::ops::Writes;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver};

use super::load::{self, Kind, Loaded};
use super::upload::{fits, upload, LightmapArray};

pub(super) fn start(list: Vec<String>, kind: Kind, bc: bool) -> Receiver<Result<Loaded, String>> {
    let (outbox, inbox) = channel();
    let spawned = std::thread::Builder::new()
        .name("lightmap-load".into())
        .spawn(move || {
            let _ = outbox.send(load::load(&list, kind, bc));
            rusty_core::wake::frame();
        });
    if let Err(e) = spawned {
        log::warn!("[Lightmap] cannot start the page loader: {e}");
    }
    inbox
}

pub(super) fn finish(
    device: &wgpu::Device,
    writes: Writes,
    list: &[String],
    result: Result<Loaded, String>,
) -> Option<Rc<LightmapArray>> {
    let limits = device.limits();
    let fit = |l: Loaded| fits(&limits, l.edge, l.layers.len(), l.bytes()).map(|()| l);
    match result.and_then(fit) {
        Ok(loaded) => Some(Rc::new(upload(device, writes, &loaded))),
        Err(e) => {
            log::warn!("[Lightmap] cannot load the pages {list:?}: {e}");
            None
        }
    }
}
