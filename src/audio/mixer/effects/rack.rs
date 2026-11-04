//! src/audio/mixer/effects/rack.rs — a group's effect chain on the audio thread
//! (#1748).
//!
//! Every group track carries one [`Rack`]: a kira effect after the group's
//! filters that runs the group's effects in order. The audio thread never
//! allocates, frees or locks here:
//!
//!   * **Parameters** cross as one `Copy` [`EffectChain`] through kira's triple
//!     buffer (`CommandWriter`), as a voice's mix crosses in atomics: the rack
//!     reads the latest at its next block, and every value glides toward it
//!     through the voice's one-pole ([`SMOOTHING`], #1693), stepped every
//!     [`CHUNK`] frames, so a snapshot moving a threshold never clicks.
//!   * **A slot whose effect changes** (another kind, another sidechain key, or a
//!     device whose rate changed) gets a fresh DSP built by [`RackHandle::set`] on
//!     the game thread (a delay's line is allocated there) and sent through a
//!     bounded channel; the one it replaces is sent back to be dropped on the game
//!     thread. Until the new one lands, a slot whose DSP and parameters disagree on
//!     the kind is skipped for that block.
//!   * **The rack publishes its output peak** each block as the group's
//!     [`KeyLevel`]: what a compressor elsewhere keyed on this group reads, one
//!     block later (`chain.rs`).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

use kira::command::{command_writer_and_reader, CommandReader, CommandWriter};
use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;

use super::{kinds, Dsp, DspContext, EffectChain, EffectParams, KeyLevel, KindId, MAX_EFFECTS};
use crate::audio::device::voice::SMOOTHING;
use crate::audio::mixer::GroupId;

/// How many frames the parameters hold still between glide steps.
pub const CHUNK: usize = 16;
/// How many slot changes can wait for the audio thread at once.
const QUEUE: usize = 4 * MAX_EFFECTS;
/// The rate a rack assumes before kira has told it its own.
const FALLBACK_RATE: u32 = 48_000;

/// What a slot's DSP is: its kind, and the group it keys from.
type Shape = (KindId, Option<GroupId>);
/// A slot's new DSP (`None` empties it).
struct Install {
    slot: usize,
    dsp: Option<(KindId, Box<dyn Dsp>)>,
}

/// What the rack tells its handle: the device's rate, and whether it changed.
#[derive(Default)]
struct Shared {
    sample_rate: AtomicU32,
    stale: AtomicBool,
}

/// A rack's game-thread half and the builder kira takes.
pub fn rack() -> (RackBuilder, RackHandle) {
    let (writer, reader) = command_writer_and_reader();
    let (send, installs) = sync_channel(QUEUE);
    let (retire, retired) = sync_channel(QUEUE);
    let (level, shared) = (Arc::new(KeyLevel::default()), Arc::new(Shared::default()));
    let builder = RackBuilder(Rack {
        reader,
        target: EffectChain::default(),
        current: EffectChain::default(),
        slots: Default::default(),
        installs,
        retire,
        level: Arc::clone(&level),
        shared: Arc::clone(&shared),
    });
    let handle = RackHandle {
        writer,
        send,
        retired,
        pending: VecDeque::new(),
        installed: [None; MAX_EFFECTS],
        last: EffectChain::default(),
        level,
        shared,
    };
    (builder, handle)
}

/// The game thread's side of one rack.
pub struct RackHandle {
    writer: CommandWriter<EffectChain>,
    send: SyncSender<Install>,
    retired: Receiver<Box<dyn Dsp>>,
    /// Slot changes the channel had no room for yet, oldest first.
    pending: VecDeque<Install>,
    /// What each slot was last sent.
    installed: [Option<Shape>; MAX_EFFECTS],
    last: EffectChain,
    level: Arc<KeyLevel>,
    shared: Arc<Shared>,
}

impl RackHandle {
    /// This group's output peak, for a sidechain to key from.
    pub fn level(&self) -> Arc<KeyLevel> {
        Arc::clone(&self.level)
    }

    /// Run `chain`, building a DSP for each slot whose shape changed; `key` finds
    /// a group's level for a sidechain.
    pub fn set(&mut self, chain: &EffectChain, key: &dyn Fn(GroupId) -> Option<Arc<KeyLevel>>) {
        self.collect();
        self.last = *chain;
        let rate = match self.shared.sample_rate.load(Ordering::Relaxed) {
            0 => FALLBACK_RATE,
            rate => rate,
        };
        for slot in 0..MAX_EFFECTS {
            let effect = chain.0[slot];
            let shape = effect.map(|e| (e.kind, e.key()));
            if shape == self.installed[slot] {
                continue;
            }
            self.installed[slot] = shape;
            let dsp = effect.map(|e| {
                let ctx = DspContext {
                    sample_rate: rate,
                    key: e.key().and_then(key),
                };
                (e.kind, (e.kind.row().build)(&ctx))
            });
            self.pending.push_back(Install { slot, dsp });
        }
        self.flush();
        self.writer.write(*chain);
    }

    /// Once a frame: drop what the audio thread handed back, send what waited, and
    /// rebuild every slot if the device's rate changed.
    pub fn maintain(&mut self, key: &dyn Fn(GroupId) -> Option<Arc<KeyLevel>>) {
        if self.shared.stale.swap(false, Ordering::Relaxed) {
            self.installed = [None; MAX_EFFECTS];
            let last = self.last;
            self.set(&last, key);
        } else {
            self.collect();
            self.flush();
        }
    }

    fn collect(&mut self) {
        while self.retired.try_recv().is_ok() {}
    }

    fn flush(&mut self) {
        while let Some(install) = self.pending.pop_front() {
            match self.send.try_send(install) {
                Ok(()) => {}
                Err(TrySendError::Full(install)) => {
                    self.pending.push_front(install);
                    return;
                }
                Err(TrySendError::Disconnected(_)) => return,
            }
        }
    }
}

/// What kira builds the rack from.
pub struct RackBuilder(Rack);

impl EffectBuilder for RackBuilder {
    type Handle = ();
    fn build(self) -> (Box<dyn Effect>, ()) {
        (Box::new(self.0), ())
    }
}

/// The audio thread's side: see the module docs.
pub struct Rack {
    reader: CommandReader<EffectChain>,
    /// The parameters last written.
    target: EffectChain,
    /// Where the glided parameters stand.
    current: EffectChain,
    slots: [Option<(KindId, Box<dyn Dsp>)>; MAX_EFFECTS],
    installs: Receiver<Install>,
    retire: SyncSender<Box<dyn Dsp>>,
    level: Arc<KeyLevel>,
    shared: Arc<Shared>,
}

impl Effect for Rack {
    fn init(&mut self, sample_rate: u32, _internal_buffer_size: usize) {
        self.shared
            .sample_rate
            .store(sample_rate, Ordering::Relaxed);
    }

    fn on_change_sample_rate(&mut self, sample_rate: u32) {
        self.shared
            .sample_rate
            .store(sample_rate, Ordering::Relaxed);
        self.shared.stale.store(true, Ordering::Relaxed);
    }

    fn on_start_processing(&mut self) {
        if let Some(chain) = self.reader.read() {
            self.target = chain;
        }
        while let Ok(install) = self.installs.try_recv() {
            let old = std::mem::replace(&mut self.slots[install.slot], install.dsp);
            self.current.0[install.slot] = None;
            if let Some((_, old)) = old {
                // The channel holds as many as were ever sent, so this never
                // fails while the handle lives; once it is gone, nothing waits.
                self.retire.try_send(old).ok();
            }
        }
    }

    fn process(&mut self, input: &mut [Frame], dt: f64, _info: &Info) {
        let mut peak = 0.0f32;
        for chunk in input.chunks_mut(CHUNK) {
            let k = (1.0 - (-dt * chunk.len() as f64 / SMOOTHING).exp()) as f32;
            for (slot, dsp) in self.slots.iter_mut().enumerate() {
                let (Some(target), Some((kind, dsp))) = (self.target.0[slot], dsp.as_mut()) else {
                    continue;
                };
                if target.kind != *kind {
                    continue;
                }
                let now = match self.current.0[slot] {
                    Some(c) if c.kind == target.kind => glide(&c, &target, k),
                    _ => target,
                };
                self.current.0[slot] = Some(now);
                dsp.process(&now.values, chunk, dt);
            }
            peak = chunk.iter().fold(peak, |p, f| p.max(kinds::peak(f)));
        }
        self.level.set(peak);
    }
}

/// `from` moved toward `to` by `k`. A value within a hair of its target, or one
/// a step too small to move an `f32`, lands on it, as a voice's glide does: a
/// settled effect plays its parameters exactly.
fn glide(from: &EffectParams, to: &EffectParams, k: f32) -> EffectParams {
    let mut out = from.lerp(to, k);
    for ((v, t), was) in out.values.iter_mut().zip(to.values).zip(from.values) {
        if *v == was || !v.is_finite() || (t - *v).abs() <= 1e-6 * t.abs().max(1.0) {
            *v = t;
        }
    }
    out
}

#[cfg(test)]
#[path = "rack_tests.rs"]
mod rack_tests;
