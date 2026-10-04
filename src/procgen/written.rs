//! src/procgen/written.rs — the log of image files the engine has (re)written, so a
//! texture cache keyed by path knows which entries went stale (#689).
//!
//! Unity reimports an asset when it is written; rusty's analog is that every PNG the
//! engine writes ([`super::bake::bake_to_png`], behind `Texture.Bake`, `BakeSet` and
//! `Material.Rebake`) is noted here, and the renderer drains the notes once per frame.
//! A note is data the renderer reads, never a call into `render` (the direction rule).
//! The log holds one entry per distinct path, so it stays as small as the set of
//! baked files.

use std::collections::BTreeMap;
use std::sync::Mutex;

/// Each written path with the generation of its latest write; the counter is the
/// newest generation handed out. Ordered, so [`written_since`] lists paths by name.
static WRITES: Mutex<(u64, BTreeMap<String, u64>)> = Mutex::new((0, BTreeMap::new()));

/// Record that the image at `path` was just written.
pub fn note_written(path: &str) {
    let mut writes = WRITES.lock().unwrap_or_else(|e| e.into_inner());
    writes.0 += 1;
    let generation = writes.0;
    writes.1.insert(path.to_string(), generation);
}

/// The newest write generation, and every path written after generation `seen`. A
/// consumer stores the returned generation and passes it back next time.
pub fn written_since(seen: u64) -> (u64, Vec<String>) {
    let writes = WRITES.lock().unwrap_or_else(|e| e.into_inner());
    if writes.0 == seen {
        return (seen, Vec::new());
    }
    let fresh = writes
        .1
        .iter()
        .filter(|(_, &g)| g > seen)
        .map(|(p, _)| p.clone());
    (writes.0, fresh.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_is_reported_once_to_each_consumer() {
        let (start, _) = written_since(0);
        note_written("out/written_test.png");
        let (now, paths) = written_since(start);
        assert!(paths.contains(&"out/written_test.png".to_string()));
        let (_, again) = written_since(now);
        assert!(!again.contains(&"out/written_test.png".to_string()));
        // A second consumer that last looked before the write still sees it.
        assert!(written_since(start)
            .1
            .contains(&"out/written_test.png".to_string()));
    }
}
