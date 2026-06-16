//! Top-level per-frame rendering: mesh/texture upload, uniform updates, and the
//! orchestration of pre-created resources into the scene render pass. Extracted
//! from the original monolithic `Renderer::render` (behavior unchanged).

use glam::Vec3;

use super::draw_pass::ScenePassFrame;
use super::postfx_params::build_post_params;
use super::{
    AmbientLightUniform, Camera, CameraUniform, DirectionalLightUniform, LightingUniform,
    PointLightUniform, Renderer, SpotlightUniform,
};
use crate::scene::{LightType, Scene};

impl Renderer {
    /// Renders the 3D scene inside a viewport render pass
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        view_texture: &wgpu::TextureView,
        editor_mode: bool,
        pathfinding_points: &[Vec3],
    ) {
        // A. Preload/update meshes and textures to avoid mutable borrow checker clashes
        // with the render pass immutable borrow of self.depth_view

        // Update meshes
        let mut mesh_updates = Vec::new();
        for entity in scene.iter() {
            if !entity.active {
                continue;
            }
            if let Some(mesh) = &entity.mesh {
                if !self.gpu_meshes.contains_key(&entity.id) || mesh.is_dirty.get() {
                    mesh_updates.push((entity.id, mesh.vertices.clone(), mesh.indices.clone()));
                    mesh.is_dirty.set(false);
                }
            }
        }

        for (id, vertices, indices) in mesh_updates {
            self.update_gpu_mesh(id, &vertices, &indices);
        }

        // Preload textures
        let mut tex_paths = Vec::new();
        for entity in scene.iter() {
            if !entity.active {
                continue;
            }
            if let Some(t_comp) = &entity.texture {
                tex_paths.push(t_comp.path.clone());
            }
        }

        for path in tex_paths {
            self.load_texture(&path);
        }

        // Update skybox texture if path changed
        if !scene.skybox_path.is_empty() && self.skybox_path != scene.skybox_path {
            let path = scene.skybox_path.clone();
            self.skybox_path = path.clone();
            self.skybox_texture = Some(self.load_texture(&path));
        } else if scene.skybox_path.is_empty() {
            self.skybox_path = "".to_string();
            self.skybox_texture = None;
        }

        // 1. Write Camera Matrix Uniform
        let aspect = self.size.width as f32 / self.size.height as f32;
        let view_proj = camera.build_view_projection(aspect);

        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array(),
            camera_pos: camera.position.to_array(),
            _pad: 0.0,
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        // 2. Build and Write Lighting Uniforms
        let lighting_uniform = self.build_lighting_uniform(scene);
        self.queue.write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::bytes_of(&lighting_uniform),
        );

        // 3. Pre-create all uniform buffers and bind groups to extend lifetimes
        // so that they outlive the render pass and prevent lifetime borrow-checker errors.
        let default_bones = Self::default_bones();

        let solid_render_resources =
            self.precreate_solid_resources(scene, &default_bones, camera.culling_mask);

        // Pre-create outline resources for selected entity silhouette (editor mode only)
        let outline_resources = if editor_mode {
            self.precreate_outline(scene, &default_bones)
        } else {
            None
        };

        // Pre-create Grid bind group
        let grid_resources = if editor_mode {
            self.precreate_grid(&default_bones)
        } else {
            None
        };

        // Pre-create AABB wireframes
        let aabb_resources = if editor_mode {
            self.precreate_aabb(scene, &default_bones)
        } else {
            Vec::new()
        };

        // Pre-create axis arrows for the selected entity in EditorMode
        let axis_arrow_resources = if editor_mode {
            self.precreate_axis_arrows(scene, &default_bones)
        } else {
            Vec::new()
        };

        // Pre-create Path vertices
        let _path_resources = self.precreate_path(pathfinding_points, &default_bones);

        // Build the post-FX params from the scene's visual-correction knobs.
        let (mut post_params, bloom_enabled) = build_post_params(
            scene,
            self.quality,
            view_proj,
            self.post_fx.prev_view_proj,
            camera.position.to_array(),
        );
        // Texel size of the (reduced-res) bloom buffer for the blur pass.
        post_params.misc[1] = 1.0 / self.post_fx.bloom_size.0 as f32;
        post_params.misc[2] = 1.0 / self.post_fx.bloom_size.1 as f32;
        self.post_fx.prev_view_proj = view_proj;

        let frame = ScenePassFrame {
            view_texture,
            camera,
            editor_mode,
            post_params,
            bloom_enabled,
        };
        self.execute_scene_pass(
            scene,
            frame,
            &solid_render_resources,
            &outline_resources,
            &grid_resources,
            &aabb_resources,
            &axis_arrow_resources,
        );
    }

    /// Builds the per-frame lighting uniform by scanning the scene's lights and
    /// visual-correction (SSR) settings.
    fn build_lighting_uniform(&self, scene: &Scene) -> LightingUniform {
        let mut lighting_uniform = LightingUniform {
            ambient: AmbientLightUniform {
                color: scene.ambient_color.to_array(),
                intensity: scene.ambient_intensity,
            },
            dir_light: DirectionalLightUniform {
                direction: [0.0, -1.0, 0.0],
                _pad1: 0.0,
                color: [1.0, 1.0, 1.0],
                intensity: 0.0,
                _pad2: [0.0; 4],
            },
            point_lights: [PointLightUniform {
                position: [0.0, 0.0, 0.0],
                _pad1: 0.0,
                color: [0.0, 0.0, 0.0],
                intensity: 0.0,
                range: 0.0,
                _pad2: [0.0; 3],
            }; 4],
            spot_light: SpotlightUniform {
                position: [0.0, 0.0, 0.0],
                _pad1: 0.0,
                direction: [0.0, 0.0, 0.0],
                _pad2: 0.0,
                color: [0.0, 0.0, 0.0],
                intensity: 0.0,
                range: 0.0,
                inner_cone: 0.0,
                outer_cone: 0.0,
                _pad3: 0.0,
            },
            num_point_lights: 0,
            ssr_active: 0.0,
            ssr_quality: 0.0,
            ssr_temporal_upsampling: 0.0,
        };

        // Populate dynamic lights from the scene
        let mut pt_idx = 0;
        for entity in scene.iter() {
            if !entity.active {
                continue;
            }

            if let Some(light) = &entity.light {
                match light.light_type {
                    LightType::Ambient => {
                        lighting_uniform.ambient = AmbientLightUniform {
                            color: light.color.to_array(),
                            intensity: light.intensity,
                        };
                    }
                    LightType::Directional => {
                        let dir = entity.transform.rotation * Vec3::NEG_Z;
                        lighting_uniform.dir_light = DirectionalLightUniform {
                            direction: dir.to_array(),
                            _pad1: 0.0,
                            color: light.color.to_array(),
                            intensity: light.intensity,
                            _pad2: [0.0; 4],
                        };
                    }
                    LightType::Point => {
                        if pt_idx < 4 {
                            lighting_uniform.point_lights[pt_idx] = PointLightUniform {
                                position: entity.transform.position.to_array(),
                                _pad1: 0.0,
                                color: light.color.to_array(),
                                intensity: light.intensity,
                                range: light.range,
                                _pad2: [0.0; 3],
                            };
                            pt_idx += 1;
                        }
                    }
                    LightType::Spotlight => {
                        let dir = entity.transform.rotation * Vec3::NEG_Z;
                        lighting_uniform.spot_light = SpotlightUniform {
                            position: entity.transform.position.to_array(),
                            _pad1: 0.0,
                            direction: dir.to_array(),
                            _pad2: 0.0,
                            color: light.color.to_array(),
                            intensity: light.intensity,
                            range: light.range,
                            inner_cone: light.inner_cone.to_radians().cos(),
                            outer_cone: light.outer_cone.to_radians().cos(),
                            _pad3: 0.0,
                        };
                    }
                }
            }
        }
        lighting_uniform.num_point_lights = pt_idx as u32;

        // Scan scene for active Visual Correction components (SSR)
        let mut ssr_active = 0.0;
        let mut ssr_quality = 2.0; // High default
        let mut ssr_temporal = 0.0;

        for entity in scene.iter() {
            if !entity.active {
                continue;
            }
            if let Some(vc) = &entity.visual_correction {
                if vc.ssr_active {
                    ssr_active = 1.0;
                }
                ssr_quality = match vc.ssr_quality.as_str() {
                    "Low" => 0.0,
                    "Medium" => 1.0,
                    "High" => 2.0,
                    "Ultra" => 3.0,
                    _ => 2.0,
                };
                if vc.ssr_temporal_upsampling {
                    ssr_temporal = 1.0;
                }
            }
        }

        lighting_uniform.ssr_active = ssr_active;
        lighting_uniform.ssr_quality = ssr_quality;
        lighting_uniform.ssr_temporal_upsampling = ssr_temporal;

        lighting_uniform
    }
}
