//! Upgrades' reads off the frame: one worker thread decodes each wanted chain.

use std::collections::BTreeMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::gpu::textures::{decode_skipping, Decoded, TextureKind};
use rusty_asset::MapRef;

/// Reads in flight at once, so a camera cut queues no more than the next few frames install.
pub(super) const IN_FLIGHT: usize = 4;

type Job = (MapRef, TextureKind, u32);
pub(super) type Done = (MapRef, u32, Option<Decoded>);

/// The upload a read was asked for: a read of a texture since re-made is dropped. A weak
/// handle keeps its address from being reused.
pub(super) type Asked = std::rc::Weak<crate::GpuTexture>;

pub(super) struct Reads {
    jobs: Option<Sender<Job>>,
    done: Receiver<Done>,
    back: Sender<Done>,
    features: wgpu::Features,
    /// What each map's one read in flight leaves out, and of which upload.
    pending: BTreeMap<MapRef, (u32, Asked)>,
    /// Asked, not yet taken: what [`IN_FLIGHT`] caps.
    pub in_flight: usize,
}

impl Reads {
    pub(super) fn new(features: wgpu::Features) -> Self {
        let (back, done) = channel();
        let (tx, rx) = channel::<Job>();
        let to = back.clone();
        let read = move || {
            for (map, kind, skip) in rx {
                let decoded = decode_skipping(&map, kind, features, skip);
                if to.send((map, skip, decoded)).is_err() {
                    return;
                }
            }
        };
        let thread = std::thread::Builder::new().name("mip-stream".into());
        let jobs = match thread.spawn(read) {
            Ok(_) => Some(tx),
            Err(e) => {
                log::warn!(
                    "[Render] no mip streaming thread could start ({e}); reading in the frame"
                );
                None
            }
        };
        Self {
            jobs,
            done,
            back,
            features,
            pending: BTreeMap::new(),
            in_flight: 0,
        }
    }

    /// One read per map: a nearer target waits for the one in flight.
    pub(super) fn ask(&mut self, map: &MapRef, kind: TextureKind, (skip, of): (u32, Asked)) {
        if self.in_flight >= IN_FLIGHT || self.pending.contains_key(map) {
            return;
        }
        self.in_flight += 1;
        self.pending.insert(map.clone(), (skip, of));
        let job = (map.clone(), kind, skip);
        if let Some(Err(_)) = self.jobs.as_ref().map(|tx| tx.send(job.clone())) {
            self.jobs = None;
        }
        if self.jobs.is_none() {
            let decoded = decode_skipping(&job.0, kind, self.features, skip);
            let _ = self.back.send((job.0, skip, decoded));
        }
    }

    pub(super) fn take(&mut self) -> Option<(Done, Option<Asked>)> {
        let done = self.done.try_recv().ok()?;
        self.in_flight -= 1;
        let asked = self.pending.remove(&done.0).map(|(_, of)| of);
        Some((done, asked))
    }
}
