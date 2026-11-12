//! src/render/warm/decode.rs — the warm-up's textures read off the frame thread
//! (#2123): reading an image, decoding it and fitting it to its *Max Size* is file
//! work that can cost a frame tens of milliseconds for one large map, so a thread of
//! its own does it, and a warm step only uploads what it handed back.
//!
//! The thread is started by the first step that meets a texture the renderer does
//! not hold, with every such texture left in the plan, so a reload whose maps are
//! cached reads nothing. Dropping the warm-up (a Stop, a replaced load) ends the
//! thread at its next file. A file written after the thread started is not trusted:
//! its step reads it again, as a lookup would.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use crate::asset::MapRef;
use crate::core::fs::written::{written_since, Stale};
use crate::render::gpu::textures::{decode, texture_written, Decoded, TextureKind};

/// A texture to read: its index in the plan, the map and the slot's kind.
pub(super) type Job = (usize, MapRef, TextureKind);

/// What a step finds for its texture.
pub(super) enum Taken {
    /// Read: its texels, or `None` when the file could not be read.
    Ready(Option<Decoded>),
    /// Still being read: try on a later frame.
    Waiting,
    /// Not read by the thread (not queued, written meanwhile, or the thread is
    /// gone): the step reads it itself.
    Lost,
}

/// The textures one thread is reading, and those it has handed back.
pub(super) struct Decoding {
    queued: BTreeSet<usize>,
    rx: Receiver<(usize, Option<Decoded>)>,
    ready: BTreeMap<usize, Option<Decoded>>,
    ended: bool,
    /// The write-log generation before the thread read its first file.
    since: u64,
}

impl Decoding {
    /// Read `jobs` on a thread of its own, for a device with `features`. With no
    /// thread to be had, every texture is [`Taken::Lost`] and read by its step.
    pub(super) fn start(jobs: Vec<Job>, features: wgpu::Features) -> Self {
        let since = written_since(u64::MAX).0;
        let queued = jobs.iter().map(|(index, ..)| *index).collect();
        let (tx, rx) = channel();
        let read = move || {
            for (index, map, kind) in jobs {
                if tx.send((index, decode(&map, kind, features))).is_err() {
                    return; // the warm-up was dropped
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

    /// What the thread has for the texture at `index` of the plan, `map`.
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

    /// Whether `map`'s file or settings were written since the thread started.
    fn written(&self, map: &MapRef) -> bool {
        let (_, paths) = written_since(self.since);
        !paths.is_empty() && texture_written(&Stale::new(paths), map.as_str())
    }
}
