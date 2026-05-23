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
    metallic: f32,
    roughness: f32,
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

const PI: f32 = 3.14159265359;

fn DistributionGGX(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

    let nom   = a2;
    let denom = (NdotH2 * (a2 - 1.0) + 1.0);
    return nom / (PI * denom * denom);
}

fn GeometrySchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;

    let nom   = NdotV;
    let denom = NdotV * (1.0 - k) + k;

    return nom / denom;
}

fn GeometrySmith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2 = GeometrySchlickGGX(NdotV, roughness);
    let ggx1 = GeometrySchlickGGX(NdotL, roughness);

    return ggx2 * ggx1;
}

fn FresnelSchlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn calculate_pbr(
    N: vec3<f32>, 
    V: vec3<f32>, 
    L: vec3<f32>, 
    radiance: vec3<f32>, 
    F0: vec3<f32>, 
    metallic: f32, 
    roughness: f32, 
    albedo: vec3<f32>
) -> vec3<f32> {
    let H = normalize(L + V);
    let NdotL = max(dot(N, L), 0.0);
    let NdotV = max(dot(N, V), 0.0);

    let D = DistributionGGX(N, H, roughness);
    let G = GeometrySmith(N, V, L, roughness);
    let F = FresnelSchlick(max(dot(H, V), 0.0), F0);

    let kS = F;
    var kD = vec3<f32>(1.0) - kS;
    kD *= 1.0 - metallic;

    let numerator = D * G * F;
    let denominator = 4.0 * NdotV * NdotL + 0.0001;
    let specular = numerator / denominator;

    let diffuse = kD * albedo / PI;

    return (diffuse + specular) * radiance * NdotL;
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

    let albedo = base_color.rgb;
    let metallic = clamp(entity.metallic, 0.0, 1.0);
    let roughness = clamp(entity.roughness, 0.04, 1.0);

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);

    // 1. Ambient lighting term (diffuse ambient)
    var lighting_color = lighting.ambient.color * lighting.ambient.intensity * albedo * (1.0 - metallic);

    // 2. Directional Light
    let L_dir = normalize(-lighting.dir_light.direction);
    let radiance_dir = lighting.dir_light.color * lighting.dir_light.intensity;
    lighting_color += calculate_pbr(N, V, L_dir, radiance_dir, F0, metallic, roughness, albedo);

    // 3. Point Lights
    for (var i = 0u; i < lighting.num_point_lights; i = i + 1u) {
        let light = lighting.point_lights[i];
        let light_dir = light.position - in.world_position;
        let d = length(light_dir);
        if (d > light.range) {
            continue;
        }
        let L = normalize(light_dir);
        let atten = 1.0 / (d * d + 1.0);
        let radiance = light.color * light.intensity * atten;
        lighting_color += calculate_pbr(N, V, L, radiance, F0, metallic, roughness, albedo);
    }

    // 4. Spotlight
    let spot = lighting.spot_light;
    let spot_dir = spot.position - in.world_position;
    let spot_dist = length(spot_dir);
    if (spot_dist <= spot.range) {
        let L_spot = normalize(spot_dir);
        let theta = dot(L_spot, normalize(-spot.direction));
        
        if (theta > spot.outer_cone) {
            let intensity = clamp((theta - spot.outer_cone) / (spot.inner_cone - spot.outer_cone), 0.0, 1.0);
            let atten = 1.0 / (spot_dist * spot_dist + 1.0);
            let radiance = spot.color * spot.intensity * atten * intensity;
            lighting_color += calculate_pbr(N, V, L_spot, radiance, F0, metallic, roughness, albedo);
        }
    }

    return vec4<f32>(lighting_color, base_color.a);
}
