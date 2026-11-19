//! crates/asset/src/cook/stamp.rs — what a cooked model was made from (#2332).
//!
//! A cooked file is current only while its stamp is: a digest of the bytes of
//! every file the parse reads for its geometry (`import_cache::parsed`: the
//! source, or a converted source's cached glTF, and a `.gltf`'s external buffers),
//! the import settings that change the cooked values (the Scale Factor and
//! Generate Lightmap UVs with its knobs) and [`COOKER_VERSION`]. A source edit,
//! a re-conversion, a ticked box and a change of cooking code each make another
//! stamp, so a cooked file is never read for anything but the model it was cooked
//! from: a stale unwrap would misplace every baked lightmap.

use std::fmt::Write;
use std::path::Path;

use crate::convert::depends::{digest, Seen};
use crate::ImportSettings;

/// The cooking code's version: bump it whenever what a cook writes changes (the
/// tangents, the unwrap, the clusters, or this file's layout). `version::tests`
/// fails when the output changes at the same version, naming the bump.
pub const COOKER_VERSION: u32 = 1;

/// The stamp of cooking the model a parse of `read` reads with `settings`, or
/// `None` when one of its files cannot be read (nothing is then cooked to disk).
pub fn stamp(read: &Path, settings: &ImportSettings) -> Option<u64> {
    let mut made_by = format!("v{COOKER_VERSION}");
    for file in crate::import_cache::parsed(read) {
        match digest(&file) {
            Seen::Read(d) => {
                let _ = write!(made_by, " {d:016x}");
            }
            Seen::Absent | Seen::Unreadable => return None,
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
