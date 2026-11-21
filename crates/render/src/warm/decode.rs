use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};

use crate::gpu::textures::{decode, texture_written, Decoded, TextureKind};
use rusty_asset::MapRef;
use rusty_core::fs::written::{written_since, Stale};

pub(super) type Job = (usize, MapRef, TextureKind);

pub(super) enum Taken {
    Ready(Option<Decoded>),
    Waiting,
    Lost,
}

pub(super) struct Decoding {
    queued: BTreeSet<usize>,
    rx: Receiver<(usize, Option<Decoded>)>,
    ready: BTreeMap<usize, Option<Decoded>>,
    ended: bool,
    since: u64,
}

impl Decoding {
    pub(super) fn start(jobs: Vec<Job>, features: wgpu::Features) -> Self {
        let since = written_since(u64::MAX).0;
        let queued = jobs.iter().map(|(index, ..)| *index).collect();
        let (tx, rx) = sync_channel(2);
        let read = move || {
            for (index, map, kind) in jobs {
                if tx.send((index, decode(&map, kind, features))).is_err() {
                    return;
                }
            }
        };
        let thread = std::thread::Builder::new().name("texture-decode".into());
        if let Err(e) = thread.spawn(read) {
            log::warn!("[Load] no texture thread could start ({e}); reading in the frame");
        }
        Self {
            queued,
            rx,
            ready: BTreeMap::new(),
            ended: false,
            since,
        }
    }

    pub(super) fn take(&mut self, index: usize, map: &MapRef) -> Taken {
        if !self.queued.contains(&index) {
            return Taken::Lost;
        }
        loop {
            match self.rx.try_recv() {
                Ok((at, decoded)) => drop(self.ready.insert(at, decoded)),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.ended = true;
                    break;
                }
            }
        }
        match self.ready.remove(&index) {
            Some(_) if self.written(map) => Taken::Lost,
            Some(decoded) => Taken::Ready(decoded),
            None if self.ended => Taken::Lost,
            None => Taken::Waiting,
        }
    }

    fn written(&self, map: &MapRef) -> bool {
        let (_, paths) = written_since(self.since);
        !paths.is_empty() && texture_written(&Stale::new(paths), map.as_str())
    }
}
