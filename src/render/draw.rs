//! Top-level per-frame rendering: mesh/texture upload, uniform updates, and the
//! orchestration of pre-created resources into the scene render pass. Extracted
//! from the original monolithic `Renderer::render` (behavior unchanged).

use glam::Vec3;

use super::draw_pass::{PassClear, ScenePassFrame};
use super::postfx_params::build_post_params;
use super::{
    build_camera_stack, AmbientLightUniform, Camera, CameraUniform, DirectionalLightUniform,
    LightingUniform, PointLightUniform, Renderer, SpotlightUniform,
};
use crate::scene::{LightType, Scene};

impl Renderer {
    /// Renders the 3D scene inside a viewport render pass.
    ///
    /// In play mode this composites the camera stack (#93): the scene's active
    /// `CameraComponent` entities, sorted by `render_order`, each draw with their own
    /// culling mask / lens / clear flags so a viewmodel or UI camera layers on top of
    /// the world. In edit mode the free-fly `camera` is the single pass. Post-FX runs
    /// once over the composited HDR target.
    pub fn render(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        view_texture: &wgpu::TextureView,
        editor_mode: bool,
        pathfinding_points: &[Vec3],
    ) {
        self.upload_scene_assets(scene);

        // Build + write the camera-independent lighting uniform once.
        let lighting_uniform = self.build_lighting_uniform(scene);
        self.queue.write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::bytes_of(&lighting_uniform),
        );

        let default_bones = Self::default_bones();
        let aspect = self.size.width as f32 / self.size.height as f32;

        // The ordered camera stack (one entry in edit mode / when no scene camera).
        let stack = build_camera_stack(camera, scene, !editor_mode);
        let last = stack.len().saturating_sub(1);
        // The base (first) camera drives the shared post-FX history / motion vectors.
        let base_view_proj = stack[0].build_view_projection(aspect);

        for (idx, cam) in stack.iter().enumerate() {
            // 1. Write this camera's view/projection uniform.
            let view_proj = cam.build_view_projection(aspect);
            let camera_uniform = CameraUniform {
                view_proj: view_proj.to_cols_array(),
                camera_pos: cam.position.to_array(),
                _pad: 0.0,
            };
            self.queue
                .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

            // 2. Pre-create per-camera resources (the culling mask differs per camera).
            let solid_render_resources =
                self.precreate_solid_resources(scene, &default_bones, cam.culling_mask);
            let overlays = self.precreate_overlays(scene, &default_bones, editor_mode);
            let _path_resources = self.precreate_path(pathfinding_points, &default_bones);

            let frame = ScenePassFrame {
                editor_mode,
                clear: PassClear::for_pass(idx == 0, cam.clear_flags),
            };
            self.execute_scene_pass(scene, frame, &solid_render_resources, &overlays);

            // Project box-decals over this camera's lit surfaces (reads the scene
            // depth to reconstruct geometry), after solids/skybox and before the
            // additive particles so decals sit on the surface, not over the sparks.
            self.draw_decals(scene, cam);

            // Billboard particles for this camera (after solids, before the next pass).
            self.draw_particles(scene, cam);

            // 3. Composite + post-process once, over the final pass's HDR target.
            if idx == last {
                self.run_post_fx(scene, view_texture, base_view_proj, cam.position.to_array());
            }
        }
    }

    /// Upload/refresh per-frame GPU assets (meshes, textures, skybox) shared by every
    /// camera in the stack. Borrow-splits cleanly before the render passes begin.
    fn upload_scene_assets(&mut self, scene: &Scene) {
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

        // Update skybox texture if path changed.
        if !scene.skybox_path.is_empty() && self.skybox_path != scene.skybox_path {
            let path = scene.skybox_path.clone();
            self.skybox_path = path.clone();
            self.skybox_texture = Some(self.load_texture(&path));
        } else if scene.skybox_path.is_empty() {
            self.skybox_path = "".to_string();
            self.skybox_texture = None;
        }
    }

    /// Run the post-process chain (color correction, bloom, motion blur, SSR) over the
    /// composited HDR target, writing the corrected image to `view_texture`.
    fn run_post_fx(
        &mut self,
        scene: &Scene,
        view_texture: &wgpu::TextureView,
        view_proj: glam::Mat4,
        camera_pos: [f32; 3],
    ) {
        use super::postfx::PostFxContext;

        let (mut post_params, bloom_enabled) = build_post_params(
            scene,
            self.quality,
            view_proj,
            self.post_fx.prev_view_proj,
            camera_pos,
        );
        post_params.misc[1] = 1.0 / self.post_fx.bloom_size.0 as f32;
        post_params.misc[2] = 1.0 / self.post_fx.bloom_size.1 as f32;
        self.post_fx.prev_view_proj = view_proj;

        let skybox_view = self
            .skybox_texture
            .as_ref()
            .map(|tex| &tex.view)
            .unwrap_or(&self.default_texture.view);
        let ctx = PostFxContext {
            depth_view: &self.depth_view,
            skybox_view,
            output: view_texture,
        };
        self.post_fx
            .run(&self.device, &self.queue, ctx, post_params, bloom_enabled);
    }

    /// Builds the per-frame lighting uniform by scanning the scene's lights and
    /// visual-correction (SSR) settings.
    #[allow(clippy::too_many_lines)]
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
