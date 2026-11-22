#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::passes::shadows) enum Sweep {
    Static,
    Dynamic,
    AtlasStatic,
    AtlasDynamic,
}

impl Sweep {
    pub(super) fn baked(self) -> bool {
        matches!(self, Sweep::Static | Sweep::AtlasStatic)
    }

    pub(super) fn takes(self, is_static: bool) -> bool {
        self.baked() == is_static
    }
}
