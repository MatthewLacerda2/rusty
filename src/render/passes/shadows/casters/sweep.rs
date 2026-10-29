//! Which depth sweep a set of casters is for, and which casters each one takes.

/// Which depth sweep a set of casters is for: each has its own caster buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render::passes::shadows) enum Sweep {
    /// The cascades' cached bake: static casters only.
    Static,
    /// The cascades' per-frame pass: everything the bake leaves out.
    Dynamic,
    /// The point/spot shadow atlas's cached bake of its stale tiles (#694): static
    /// casters only.
    AtlasStatic,
    /// The atlas's per-frame pass over every tile: everything its bake leaves out.
    AtlasDynamic,
}

impl Sweep {
    /// Whether a caster whose static flag is `is_static` is drawn in this sweep.
    pub(super) fn takes(self, is_static: bool) -> bool {
        match self {
            Sweep::Static | Sweep::AtlasStatic => is_static,
            Sweep::Dynamic | Sweep::AtlasDynamic => !is_static,
        }
    }
}
