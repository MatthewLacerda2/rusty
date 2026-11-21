
#import fullscreen
#import numeric

struct BlurParams {
    shape: vec4<f32>,
};

@group(0) @binding(0) var<uniform> blur: BlurParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var t_velocity: texture_2d<f32>;
@group(0) @binding(3) var t_tiles: texture_2d<f32>;

const SOFT_DISTANCE: f32 = 0.1;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

fn image_size() -> vec2<f32> {
    return vec2<f32>(textureDimensions(t_color));
}

// The velocity texel under image pixel `p`'s centre: `p` itself when the two are the same size
// (the ratio is then exactly 1).
fn velocity_at(p: vec2<i32>) -> vec4<f32> {
    let ratio = vec2<f32>(textureDimensions(t_velocity)) / image_size();
    return textureLoad(t_velocity, vec2<i32>((vec2<f32>(p) + 0.5) * ratio), 0);
}

fn streak(p: vec2<i32>) -> vec2<f32> {
    let dims = image_size();
    let ndc = velocity_at(p).xy;
    if (!numeric::finite2(ndc)) {
        return vec2<f32>(0.0);
    }
    // NDC spans 2 units across the view, and its y points up where pixels count down.
    let v = ndc * vec2<f32>(0.5 * dims.x, -0.5 * dims.y) * blur.shape.x;
    let len = length(v);
    if (len > blur.shape.y) {
        return v * (blur.shape.y / len);
    }
    return v;
}

fn longer(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return select(a, b, dot(b, b) > dot(a, a));
}

@fragment
fn fs_tile_max_x(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let tile = i32(blur.shape.w);
    let y = i32(pos.y);
    let start = i32(pos.x) * tile;
    let end = min(start + tile, i32(image_size().x));
    var best = vec2<f32>(0.0);
    for (var x = start; x < end; x = x + 1) {
        best = longer(best, streak(vec2<i32>(x, y)));
    }
    return vec4<f32>(best, 0.0, 0.0);
}

@fragment
fn fs_tile_max_y(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let tile = i32(blur.shape.w);
    let x = i32(pos.x);
    let start = i32(pos.y) * tile;
    let end = min(start + tile, i32(textureDimensions(t_tiles).y));
    var best = vec2<f32>(0.0);
    for (var y = start; y < end; y = y + 1) {
        best = longer(best, textureLoad(t_tiles, vec2<i32>(x, y), 0).xy);
    }
    return vec4<f32>(best, 0.0, 0.0);
}

@fragment
fn fs_neighbor_max(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(t_tiles));
    let here = vec2<i32>(pos.xy);
    var best = vec2<f32>(0.0);
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let t = clamp(here + vec2<i32>(x, y), vec2<i32>(0), dims - 1);
            best = longer(best, textureLoad(t_tiles, t, 0).xy);
        }
    }
    return vec4<f32>(best, 0.0, 0.0);
}

fn depth_key(p: vec2<i32>) -> vec2<f32> {
    let texel = velocity_at(p);
    return vec2<f32>(texel.w, texel.z);
}

fn behind(center: vec2<f32>, tap: vec2<f32>) -> f32 {
    if (tap.x != center.x) {
        return select(0.0, 1.0, tap.x < center.x);
    }
    return clamp(0.5 + (tap.y - center.y) / SOFT_DISTANCE, 0.0, 1.0);
}

// Interleaved gradient noise (Jimenez 2014) in [0, 1): a fixed per-pixel offset for the taps.
fn jitter(pixel: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(pixel, vec2<f32>(0.06711056, 0.00583715))));
}

// How much a tap `dist` pixels away is covered by a streak of half-length `radius`: 1 inside
// it, fading to 0 over one pixel past its end.
fn reach(radius: f32, dist: f32) -> f32 {
    return clamp(radius - dist + 1.0, 0.0, 1.0);
}

@fragment
fn fs_gather(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let center = textureLoad(t_color, p, 0);
    let tiles = vec2<i32>(textureDimensions(t_tiles));
    let tile = min(p / i32(blur.shape.w), tiles - 1);
    let neighborhood = textureLoad(t_tiles, tile, 0).xy;
    // Less than half a pixel either side: nothing to blur, and the copy is exact.
    if (length(neighborhood) < 1.0) {
        return center;
    }

    let dims = vec2<i32>(textureDimensions(t_color));
    let center_radius = 0.5 * length(streak(p));
    let center_key = depth_key(p);
    let n = i32(blur.shape.z);
    let offset = jitter(pos.xy);
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    for (var i = 0; i < n; i = i + 1) {
        // Evenly spread over (-1, 1), each tap nudged by the pixel's jitter.
        let t = (f32(i) + offset) / f32(n) * 2.0 - 1.0;
        let step = neighborhood * 0.5 * t;
        let q = clamp(p + vec2<i32>(round(step)), vec2<i32>(0), dims - 1);
        let dist = length(step);
        // A tap behind shows through only as far as this pixel's own streak reaches; one in
        // front smears over it as far as the tap's streak reaches.
        let back = behind(center_key, depth_key(q));
        let tap_radius = 0.5 * length(streak(q));
        let w = back * reach(center_radius, dist) + (1.0 - back) * reach(tap_radius, dist);
        sum += textureLoad(t_color, q, 0).rgb * w;
        total += w;
    }
    // What the taps did not cover keeps this pixel's own colour.
    let rgb = (sum + center.rgb * max(f32(n) - total, 0.0)) / f32(n);
    return vec4<f32>(rgb, center.a);
}
