use glam::Vec3;

#[derive(Default)]
pub(crate) struct Tables {
    pub(crate) slots: Vec<u32>,
    pub(crate) by_slot: Vec<Option<(Vec3, Vec3)>>,
    /// `hiz.wgsl`'s `Bounds` per instance, `[3] > 0.5` when tested (`upload::pack_bounds`).
    pub(crate) bounds: Vec<[f32; 8]>,
}
