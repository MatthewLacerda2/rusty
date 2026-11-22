use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;

use kira::command::{command_writer_and_reader, CommandReader, CommandWriter};
use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use kira::Frame;

use super::{kinds, Dsp, DspContext, EffectChain, EffectParams, KeyLevel, KindId, MAX_EFFECTS};
use crate::device::voice::SMOOTHING;
use crate::mixer::GroupId;

pub const CHUNK: usize = 16;
/// How many slot changes can wait for the audio thread at once.
const QUEUE: usize = 4 * MAX_EFFECTS;
const FALLBACK_RATE: u32 = 48_000;

type Shape = (KindId, Option<GroupId>);
struct Install {
    slot: usize,
    dsp: Option<(KindId, Box<dyn Dsp>)>,
}

struct Shared {
    sample_rate: AtomicU32,
    stale: AtomicBool,
    output_gain: AtomicU32,
    tail: Arc<AtomicBool>,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            sample_rate: AtomicU32::new(0),
            stale: AtomicBool::new(false),
            output_gain: AtomicU32::new(1f32.to_bits()),
            tail: Arc::default(),
        }
    }
}

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

pub struct RackHandle {
    writer: CommandWriter<EffectChain>,
    send: SyncSender<Install>,
    retired: Receiver<Box<dyn Dsp>>,
    pending: VecDeque<Install>,
    installed: [Option<Shape>; MAX_EFFECTS],
    last: EffectChain,
    level: Arc<KeyLevel>,
    shared: Arc<Shared>,
}

impl RackHandle {
    pub fn level(&self) -> Arc<KeyLevel> {
        Arc::clone(&self.level)
    }

    pub fn tail(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shared.tail)
    }

    pub fn set_output_gain(&self, gain: f32) {
        let gain = if gain.is_finite() { gain.max(0.0) } else { 0.0 };
        self.shared
            .output_gain
            .store(gain.to_bits(), Ordering::Relaxed);
    }

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
    target: EffectChain,
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
        let gain = f32::from_bits(self.shared.output_gain.load(Ordering::Relaxed));
        self.level.set(peak * gain);
        let ringing = self.slots.iter().flatten().any(|(_, dsp)| dsp.ringing());
        self.shared.tail.store(ringing, Ordering::Release);
    }
}

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
