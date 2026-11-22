pub(crate) use rusty_shadergen::groups::{DRAW, GLOBAL, MATERIAL, SCENE};

/// Groups per solid-path layout: the per-draw group sits at the last index.
pub(crate) const COUNT: usize = DRAW as usize + 1;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Groups<T> {
    pub global: T,
    pub scene: Option<T>,
    pub material: Option<T>,
    pub draw: T,
}

impl<T: Copy> Groups<T> {
    pub(crate) fn ordered(&self) -> [Option<T>; COUNT] {
        let mut at = [None; COUNT];
        at[GLOBAL as usize] = Some(self.global);
        at[SCENE as usize] = self.scene;
        at[MATERIAL as usize] = self.material;
        at[DRAW as usize] = Some(self.draw);
        at
    }
}

impl Groups<&wgpu::BindGroupLayout> {
    pub(crate) fn layout(&self, device: &wgpu::Device, label: &str) -> wgpu::PipelineLayout {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &self.ordered(),
            immediate_size: 0,
        })
    }
}

pub(crate) fn draw_only<T: Copy>(draw: T) -> [Option<T>; COUNT] {
    let mut at = [None; COUNT];
    at[DRAW as usize] = Some(draw);
    at
}

#[cfg(test)]
mod tests;
