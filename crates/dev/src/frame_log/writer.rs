use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;

use super::now_ms;
use super::segments::Segments;
pub use super::segments::LOCK;

const QUEUE: usize = 240;

#[derive(Default)]
struct Dropped {
    records: AtomicU64,
    tick: AtomicU64,
}

pub struct Writer {
    tx: Option<SyncSender<Vec<String>>>,
    dropped: Arc<Dropped>,
    thread: Option<JoinHandle<()>>,
}

impl Writer {
    pub fn start(dir: &Path, budget: u64, period: Option<u64>) -> std::io::Result<Self> {
        let segments = Segments::open(dir, budget, period)?;
        let (tx, rx) = sync_channel(QUEUE);
        let dropped = Arc::new(Dropped::default());
        let behind = Arc::clone(&dropped);
        let thread = std::thread::Builder::new()
            .name("frame-log".into())
            .spawn(move || drain(rx, segments, &behind))?;
        Ok(Self {
            tx: Some(tx),
            dropped,
            thread: Some(thread),
        })
    }

    pub fn send(&mut self, lines: Vec<String>, tick: u64) {
        let (Some(tx), false) = (&self.tx, lines.is_empty()) else {
            return;
        };
        let n = lines.len() as u64;
        match tx.try_send(lines) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.dropped.tick.store(tick, Ordering::Relaxed);
                self.dropped.records.fetch_add(n, Ordering::Relaxed);
            }
            Err(TrySendError::Disconnected(_)) => self.tx = None,
        }
    }
}

impl Writer {
    pub fn lost(&mut self, n: u64, tick: u64) {
        if n > 0 {
            self.dropped.tick.store(tick, Ordering::Relaxed);
            self.dropped.records.fetch_add(n, Ordering::Relaxed);
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn drain(rx: Receiver<Vec<String>>, mut segments: Segments, dropped: &Dropped) {
    while let Ok(lines) = rx.recv() {
        let mut next = Some(lines);
        while let Some(lines) = next.take() {
            let behind = dropped.records.swap(0, Ordering::Relaxed);
            if behind > 0 {
                let tick = dropped.tick.load(Ordering::Relaxed);
                let line = format!(
                    r#"{{"ch":"dropped","t":{},"tick":{tick},"records":{behind}}}"#,
                    now_ms()
                );
                segments.write(&line);
            }
            for line in &lines {
                segments.write(line);
            }
            next = rx.try_recv().ok();
        }
        segments.flush();
        segments.prune(now_ms());
    }
    segments.flush();
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
