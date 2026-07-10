//! Persistent per-entity GPU buffers + bind groups for the forward solid pass.
//!
//! The forward pass used to `create_buffer_init` an entity uniform (128 B) *and* a
//! 64-mat4 bone palette (~4 KB) for every entity, then build two bind groups — once
//! **per camera, per frame**. For N entities and a 2-camera stack that is ~4N buffer
//! allocations + ~4N bind-group creations every frame, churning the wgpu allocator
//! and scaling badly toward F.E.A.R.-scale scenes (#210).
//!
//! Instead this pool keeps one persistent slot per entity id: the uniform buffer is
//! `COPY_DST` and rewritten with `queue.write_buffer`; the bone palette is shared
//! (a single identity buffer) for the common non-skinned case and only allocated
//! per-entity for genuinely skinned meshes; and the entity + material bind groups
//! are built once and reused, rebuilt only when their inputs actually change.

use std::collections::{HashMap, HashSet};
use wgpu::util::DeviceExt;

use crate::render::{BoneUniform, EntityUniform};

/// One entity's reused GPU resources. The material bind group is rebuilt only when
/// the resolved map paths change (`material_sig`); the bones buffer exists only for
/// skinned meshes (non-skinned entities share the pool's `default_bones_buffer`).
struct EntitySlot {
    uniform_buffer: wgpu::Buffer,
    bones_buffer: Option<wgpu::Buffer>,
    entity_bind_group: wgpu::BindGroup,
    material_bind_group: wgpu::BindGroup,
    material_sig: [String; 5],
}

/// Persistent forward-pass GPU resources keyed by entity id, plus the one shared
/// identity bone palette every non-skinned mesh binds.
pub(crate) struct EntityPool {
    slots: HashMap<u32, EntitySlot>,
    default_bones_buffer: wgpu::Buffer,
}

impl EntityPool {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let default_bones = BoneUniform {
            bones: [glam::Mat4::IDENTITY.to_cols_array(); 64],
        };
        let default_bones_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shared Default Bones"),
            contents: bytemuck::bytes_of(&default_bones),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        Self {
            slots: HashMap::new(),
            default_bones_buffer,
        }
    }

    /// The shared identity bone palette buffer, bound by every overlay/non-skinned
    /// draw so no per-entity 4 KB palette is allocated for them.
    pub(crate) fn default_bones_buffer(&self) -> &wgpu::Buffer {
        &self.default_bones_buffer
    }

    /// The reused entity (group 1) bind group for `id`, if a slot exists.
    pub(crate) fn entity_bind_group(&self, id: u32) -> Option<&wgpu::BindGroup> {
        self.slots.get(&id).map(|s| &s.entity_bind_group)
    }

    /// The reused material (group 2) bind group for `id`, if a slot exists.
    pub(crate) fn material_bind_group(&self, id: u32) -> Option<&wgpu::BindGroup> {
        self.slots.get(&id).map(|s| &s.material_bind_group)
    }

    /// Drop slots for entities not touched this frame (despawned / deactivated), so
    /// the pool tracks the live scene instead of growing without bound.
    pub(crate) fn retain(&mut self, live: &HashSet<u32>) {
        self.slots.retain(|id, _| live.contains(id));
    }

    /// Number of live per-entity slots — used by tests to prove the pool reuses
    /// slots across frames rather than allocating per frame (#210).
    #[cfg(test)]
    pub(crate) fn slot_count(&self) -> usize {
        self.slots.len()
    }
}

/// Inputs for an entity's persistent slot for one frame: the new uniform value, the
/// active bone palette (empty for non-skinned meshes), and the resolved material map
/// signature deciding whether the material bind group must be rebuilt.
pub(crate) struct SlotUpdate<'a> {
    pub uniform: EntityUniform,
    pub palette: &'a [glam::Mat4],
    pub material_sig: [String; 5],
}

impl crate::render::Renderer {
    /// Ensure `id`'s pool slot is current: rewrite its uniform, refresh the bone
    /// palette (allocating a per-entity buffer only for skinned meshes), and rebuild
    /// bind groups only when their inputs changed. `build_material` resolves the
    /// material textures and is only invoked on the first frame or a map change.
    pub(crate) fn sync_entity_slot(
        &mut self,
        id: u32,
        update: SlotUpdate,
        build_material: impl FnOnce(&Self) -> wgpu::BindGroup,
    ) {
        // Take the pool out so a slot can be mutated while other `self` fields
        // (device, queue, layouts) are read; `None` is a safe transient hole.
        let mut pool = self.entity_pool.take().expect("entity pool present");
        if let Some(slot) = pool.slots.get_mut(&id) {
            self.refresh_slot(slot, update, build_material);
        } else {
            let slot = self.create_slot(&pool, update, build_material);
            pool.slots.insert(id, slot);
        }
        self.entity_pool = Some(pool);
    }

    /// Rewrite an existing slot's uniform/palette in place and rebuild its material
    /// bind group only when the resolved map paths changed.
    fn refresh_slot(
        &self,
        slot: &mut EntitySlot,
        update: SlotUpdate,
        build_material: impl FnOnce(&Self) -> wgpu::BindGroup,
    ) {
        self.queue
            .write_buffer(&slot.uniform_buffer, 0, bytemuck::bytes_of(&update.uniform));
        write_palette(&self.queue, slot.bones_buffer.as_ref(), update.palette);
        if slot.material_sig != update.material_sig {
            slot.material_bind_group = build_material(self);
            slot.material_sig = update.material_sig;
        }
    }

    /// Allocate a fresh slot's buffers + bind groups (called once per entity, on its
    /// first appearance — subsequent frames only `write_buffer`).
    fn create_slot(
        &self,
        pool: &EntityPool,
        update: SlotUpdate,
        build_material: impl FnOnce(&Self) -> wgpu::BindGroup,
    ) -> EntitySlot {
        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Entity Uniform"),
            size: std::mem::size_of::<EntityUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue
            .write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&update.uniform));

        let bones_buffer = (!update.palette.is_empty()).then(|| {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Bones Uniform"),
                size: std::mem::size_of::<BoneUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        write_palette(&self.queue, bones_buffer.as_ref(), update.palette);

        let bones_ref = bones_buffer.as_ref().unwrap_or(&pool.default_bones_buffer);
        let entity_bind_group =
            self.entity_bind_group("Entity Bind Group", &uniform_buffer, bones_ref);
        let material_bind_group = build_material(self);

        EntitySlot {
            uniform_buffer,
            bones_buffer,
            entity_bind_group,
            material_bind_group,
            material_sig: update.material_sig,
        }
    }
}

/// Upload a bone palette into `buffer` (the first ≤64 mats), padding the rest at
/// identity. No-op when `buffer` is `None` (non-skinned: the shared identity buffer
/// is bound and never overwritten).
fn write_palette(queue: &wgpu::Queue, buffer: Option<&wgpu::Buffer>, palette: &[glam::Mat4]) {
    let Some(buffer) = buffer else { return };
    let mut bones = BoneUniform {
        bones: [glam::Mat4::IDENTITY.to_cols_array(); 64],
    };
    for (dst, src) in bones.bones.iter_mut().zip(palette.iter().take(64)) {
        *dst = src.to_cols_array();
    }
    queue.write_buffer(buffer, 0, bytemuck::bytes_of(&bones));
}

impl crate::render::Renderer {
    /// Live per-entity forward-pass slot count, for the persistence test.
    #[cfg(test)]
    pub(crate) fn entity_slot_count(&self) -> usize {
        self.entity_pool.as_ref().map_or(0, EntityPool::slot_count)
    }
}

#[cfg(test)]
mod tests {
    use crate::render::{Camera, Renderer, OFFSCREEN_FORMAT};
    use crate::scene::{MeshComponent, Scene};

    fn box_mesh() -> MeshComponent {
        let (vertices, indices) = crate::render::gpu::mesh::generate_box(1.0, 1.0, 1.0);
        MeshComponent {
            primitive_type: "Box".to_string(),
            asset_ref: None,
            vertices,
            indices,
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            is_dirty: crate::scene::DirtyFlag::new(true),
        }
    }

    /// Rendering the same scene twice must reuse the per-entity pool slots — one per
    /// entity, never growing per frame. The whole point of #210: persistent buffers
    /// written in place instead of allocated every frame. Skips with no GPU adapter.
    #[test]
    fn pool_reuses_slots_across_frames() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_headless(64, 64)) else {
            return; // No GPU/software adapter — same skip contract as the screenshots.
        };
        let mut scene = Scene::new();
        for _ in 0..3 {
            let id = scene.add_entity("Box".to_string());
            scene.world.set_mesh(id, Some(box_mesh()));
        }

        let target = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Pool Test Target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let camera = Camera::new(glam::Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);

        renderer.render(&scene, &camera, &view, false, &[]);
        assert_eq!(renderer.entity_slot_count(), 3, "one slot per entity");
        renderer.render(&scene, &camera, &view, false, &[]);
        assert_eq!(
            renderer.entity_slot_count(),
            3,
            "second frame reuses slots, no per-frame growth"
        );
    }
}
