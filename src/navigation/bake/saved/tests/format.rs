//! `file::FORMAT` is bumped whenever the bake's output changes (#1887), held by a
//! test as the converters' version is (`asset::convert::fingerprint`, #1286): a
//! restored navmesh must be the one a fresh bake gives, or the same scene and seed
//! play differently with and without a `<scene>.navmesh`.
//!
//! The pin hashes what a bake of [`super::level`] writes after the header (the
//! grid, both span layers and the generated links), not the fingerprint, which
//! changes with the scene's settings rather than with the bake. Linux only, as the
//! converters' check: another platform's maths may round a float differently, and
//! a forgotten bump is caught as well by one platform as by two.

use super::super::file::FORMAT;
use super::{level, saved};

/// The format the hash was taken at, and the hash.
const PINNED: (u32, u64) = (1, 0xa381c9902ddf3d1b);

/// Magic, format and fingerprint: what precedes the bake's output.
const HEADER: usize = 4 + 4 + 8;

#[cfg(target_os = "linux")]
#[test]
fn the_bakes_output_is_pinned_to_its_format() {
    let (scene, _) = level();
    let (_, bytes) = saved(&scene);
    let now = crate::core::hash::content(&bytes[HEADER..]);
    let (format, hash) = PINNED;
    assert!(
        format == FORMAT,
        "saved::file::FORMAT is {FORMAT}: pin its output, PINNED = ({FORMAT}, {now:#018x})"
    );
    assert!(
        hash == now,
        "the navmesh bake's output changed ({hash:#018x} → {now:#018x}): bump \
         saved::file::FORMAT, so older files are refused, then re-pin PINNED here"
    );
}
