//! A page list's load off the frame (#1694): read on a worker, then uploaded when
//! it lands, or why it failed told once.

use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver};

use super::load::{self, Kind, Loaded};
use super::upload::{fits, upload, LightmapArray};

/// Read `list` on a worker; it wakes a frame when it is done.
pub(super) fn start(list: Vec<String>, kind: Kind, bc: bool) -> Receiver<Result<Loaded, String>> {
    let (outbox, inbox) = channel();
    let spawned = std::thread::Builder::new()
        .name("lightmap-load".into())
        .spawn(move || {
            // A renderer gone meanwhile has dropped the inbox: nothing to tell.
            let _ = outbox.send(load::load(&list, kind, bc));
            rusty_core::wake::frame();
        });
    if let Err(e) = spawned {
        log::warn!("[Lightmap] cannot start the page loader: {e}");
    }
    inbox
}

/// Upload a finished load, or warn of why it failed: unreadable, or more or larger
/// pages than the device's limits take, or more bytes than an array may (`fits`,
/// #1704, #1996).
pub(super) fn finish(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    list: &[String],
    result: Result<Loaded, String>,
) -> Option<Rc<LightmapArray>> {
    let limits = device.limits();
    let fit = |l: Loaded| fits(&limits, l.edge, l.layers.len(), l.bytes()).map(|()| l);
    match result.and_then(fit) {
        Ok(loaded) => Some(Rc::new(upload(device, queue, &loaded))),
        Err(e) => {
            log::warn!("[Lightmap] cannot load the pages {list:?}: {e}");
            None
        }
    }
}
