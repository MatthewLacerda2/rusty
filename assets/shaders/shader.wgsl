struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad: f32,
};

struct AmbientLight {
    color: vec3<f32>,
    intensity: f32,
};

struct DirectionalLight {
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    _pad: f32,
};

struct PointLight {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
};

struct Spotlight {
    position: vec3<f32>,
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    inner_cone: f32, // Cosine of inner angle
    outer_cone: f32, // Cosine of outer angle
};

struct LightingUniforms {
    ambient: AmbientLight,
    dir_light: DirectionalLight,
    point_lights: array<PointLight, 4>,
    spot_light: Spotlight,
    num_point_lights: u32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
};

struct EntityUniforms {
    model_matrix: mat4x4<f32>,
    color_tint: vec4<f32>,
    use_texture: u32,
    is_lit: u32,
    _pad1: u32,
    _pad2: u32,
};

struct BoneUniforms {
    bones: array<mat4x4<f32>, 64>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(0) @binding(1)
var<uniform> lighting: LightingUniforms;

@group(1) @binding(0)
var<uniform> entity: EntityUniforms;

@group(1) @binding(1)
var<uniform> bones: BoneUniforms;

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,
    @location(4) joint_weights: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Bone skinning transform
    var bone_transform = bones.bones[model.joint_indices.x] * model.joint_weights.x
                       + bones.bones[model.joint_indices.y] * model.joint_weights.y
                       + bones.bones[model.joint_indices.z] * model.joint_weights.z
                       + bones.bones[model.joint_indices.w] * model.joint_weights.w;

    // If bone weights are zero/uninitialized, default to identity matrix
    let total_weight = model.joint_weights.x + model.joint_weights.y + model.joint_weights.z + model.joint_weights.w;
    if (total_weight < 0.01) {
        bone_transform = mat4x4<f32>(
            vec4<f32>(1.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 1.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 1.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 1.0)
        );
    }

    let local_pos = bone_transform * vec4<f32>(model.position, 1.0);
    let world_pos = entity.model_matrix * local_pos;
    
    // Normal transform
    let local_normal = bone_transform * vec4<f32>(model.normal, 0.0);
    let world_normal = normalize((entity.model_matrix * local_normal).xyz);

    out.world_position = world_pos.xyz;
    out.world_normal = world_normal;
    out.tex_coords = model.tex_coords;
    out.clip_position = camera.view_proj * world_pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var base_color: vec4<f32>;
    if (entity.use_texture == 1u) {
        base_color = textureSample(t_diffuse, s_diffuse, in.tex_coords) * entity.color_tint;
    } else {
        base_color = entity.color_tint;
    }

    // Unlit rendering (e.g. grids, path highlights, light gizmos)
    if (entity.is_lit == 0u) {
        return base_color;
    }

    let N = normalize(in.world_normal);
    let V = normalize(camera.camera_pos - in.world_position);

    // 1. Ambient Contribution
    var lighting_color = lighting.ambient.color * lighting.ambient.intensity;

    // 2. Directional Light
    let L_dir = normalize(-lighting.dir_light.direction);
    let diff_dir = max(dot(N, L_dir), 0.0);
    let H_dir = normalize(L_dir + V);
    let spec_dir = pow(max(dot(N, H_dir), 0.0), 32.0); // specular shininess
    lighting_color += lighting.dir_light.color * lighting.dir_light.intensity * (diff_dir + spec_dir * 0.5);

    // 3. Point Lights
    for (var i = 0u; i < lighting.num_point_lights; i = i + 1u) {
        let light = lighting.point_lights[i];
        let light_dir = light.position - in.world_position;
        let d = length(light_dir);
        if (d > light.range) {
            continue;
        }
        let L = normalize(light_dir);
        
        // Attenuation model
        let atten = 1.0 / (d * d + 1.0);
        let diff = max(dot(N, L), 0.0);
        let H = normalize(L + V);
        let spec = pow(max(dot(N, H), 0.0), 32.0);

        lighting_color += light.color * light.intensity * atten * (diff + spec * 0.5);
    }

    // 4. Spotlight (Flashlight style)
    let spot = lighting.spot_light;
    let spot_dir = spot.position - in.world_position;
    let spot_dist = length(spot_dir);
    if (spot_dist <= spot.range) {
        let L_spot = normalize(spot_dir);
        
        // Cosine of angle between spotlight vector and light ray
        let theta = dot(L_spot, normalize(-spot.direction));
        
        if (theta > spot.outer_cone) {
            // Smooth edge interpolation
            let intensity = clamp((theta - spot.outer_cone) / (spot.inner_cone - spot.outer_cone), 0.0, 1.0);
            let atten = 1.0 / (spot_dist * spot_dist + 1.0);
            
            let diff = max(dot(N, L_spot), 0.0);
            let H = normalize(L_spot + V);
            let spec = pow(max(dot(N, H), 0.0), 32.0);

            lighting_color += spot.color * spot.intensity * atten * intensity * (diff + spec * 0.5);
        }
    }

    let final_color = vec4<f32>(base_color.rgb * lighting_color, base_color.a);
    return final_color;
}
