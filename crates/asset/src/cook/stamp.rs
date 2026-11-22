//! A cooked file is read only under its stamp, a digest of every file the parse reads
//! for geometry, each setting a cook reads and [`COOKER_VERSION`]: a stale unwrap would
//! misplace every baked lightmap. A side file the parse does not find (an `.obj`'s
//! `mtllib`) counts as absent, so the model still cooks, and again once it appears.

use std::fmt::Write;
use std::path::Path;

use crate::convert::depends::{digest, Seen};
use crate::ImportSettings;

/// The cooking code's version: bump it whenever what a cook writes changes (the
/// tangents, the unwrap, the clusters, or this file's layout). `version::tests`
/// fails when the output changes at the same version, naming the bump.
pub const COOKER_VERSION: u32 = 3;

pub fn stamp(read: &Path, settings: &ImportSettings) -> Option<u64> {
    let mut made_by = format!("v{COOKER_VERSION}");
    for file in crate::import_cache::parsed(read) {
        match digest(&file) {
            Seen::Read(d) => {
                let _ = write!(made_by, " {d:016x}");
            }
            Seen::Absent => made_by.push_str(" absent"),
            Seen::Unreadable => return None,
        }
    }
    let lm = settings.lightmap_uvs;
    let _ = write!(
        made_by,
        " scale {:08x} lightmap {} {:08x} {:08x}",
        settings.scale.to_bits(),
        lm.generate,
        lm.hard_angle.to_bits(),
        lm.pack_margin.to_bits(),
    );
    Some(rusty_core::hash::content(made_by.as_bytes()))
}
