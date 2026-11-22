use super::super::file::FORMAT;
use super::{level, saved};

const PINNED: (u32, u64) = (1, 0xa381c9902ddf3d1b);

const HEADER: usize = 4 + 4 + 8;

#[cfg(target_os = "linux")]
#[test]
fn the_bakes_output_is_pinned_to_its_format() {
    let (scene, _) = level();
    let (_, bytes) = saved(&scene);
    let now = rusty_core::hash::content(&bytes[HEADER..]);
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
