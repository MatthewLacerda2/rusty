// Unity URP's two modes. CoC is signed, in pixels of radius, from depth along the view
// axis (velocity target `.z`): the focus is a plane, as in Unity, not a sphere;
// negative is nearer than the focus. A pixel an overlay camera drew (`.w` above 0: a
// viewmodel) has no CoC and stays sharp. Gaussian weights each tap by its own CoC so a
// sharp foreground never bleeds into the blurred background. Bokeh finds per tile the
// near field's reach, so a pixel knows how wide to gather; a 3×3 tent then smooths the
// rings.

#import fullscreen

struct DofParams {
    // w: Bokeh's tile size in working pixels (at least the largest working CoC, so a
    // 3×3 of tiles holds every tap's reach)
    shape: vec4<f32>,
    focus: vec4<f32>,
};

@group(0) @binding(0) var<uniform> dof: DofParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var t_velocity: texture_2d<f32>;
@group(0) @binding(3) var t_coc: texture_2d<f32>;
@group(0) @binding(4) var t_blur: texture_2d<f32>;
@group(0) @binding(5) var t_work: texture_2d<f32>;

const TAU: f32 = 6.28318530718;
// Ring k holds 8k taps.
const MIN_RINGS: i32 = 3;
const MARGIN: f32 = 1.0;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

fn is_gaussian() -> bool {
    return dof.shape.x < 1.5;
}

fn coc_of(depth: f32) -> f32 {
    let max_coc = dof.shape.y;
    if (is_gaussian()) {
        let span = max(dof.focus.y - dof.focus.x, 1e-4);
        return clamp((depth - dof.focus.x) / span, 0.0, 1.0) * max_coc;
    }
    return clamp(dof.focus.y * (1.0 - dof.focus.x / max(depth, 1e-4)), -max_coc, max_coc);
}

fn overlay(p: vec2<i32>) -> bool {
    return textureLoad(t_velocity, p, 0).w > 0.5;
}

@fragment
fn fs_coc(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let texel = textureLoad(t_velocity, vec2<i32>(pos.xy), 0);
    if (texel.w > 0.5) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(coc_of(texel.z), 0.0, 0.0, 0.0);
}

fn block_coc(q: vec2<i32>) -> f32 {
    let div = i32(dof.shape.z);
    let last = vec2<i32>(textureDimensions(t_coc)) - 1;
    var near = 0.0;
    var far = 0.0;
    for (var y = 0; y < div; y = y + 1) {
        for (var x = 0; x < div; x = x + 1) {
            let c = textureLoad(t_coc, min(q * div + vec2<i32>(x, y), last), 0).r;
            near = min(near, c);
            far = max(far, c);
        }
    }
    return select(far, near, -near > far);
}

@fragment
fn fs_prefilter(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let div = i32(dof.shape.z);
    let base = vec2<i32>(pos.xy) * div;
    let last = vec2<i32>(textureDimensions(t_color)) - 1;
    let coc = block_coc(vec2<i32>(pos.xy));
    // A sharp pixel beside a blurred one keeps its colour out of that blur (a halo).
    var sum = vec4<f32>(0.0);
    for (var y = 0; y < div; y = y + 1) {
        for (var x = 0; x < div; x = x + 1) {
            let q = min(base + vec2<i32>(x, y), last);
            let c = textureLoad(t_coc, q, 0).r;
            let w = 1e-3 + clamp(c / select(coc, 1.0, coc == 0.0), 0.0, 1.0);
            sum += vec4<f32>(textureLoad(t_color, q, 0).rgb, 1.0) * w;
        }
    }
    return vec4<f32>(sum.rgb / sum.a, coc / dof.shape.z);
}

fn work_coc(q: vec2<i32>) -> f32 {
    return textureLoad(t_work, q, 0).a * dof.shape.z;
}

@fragment
fn fs_tile_max(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let tile = i32(dof.shape.w);
    let base = vec2<i32>(pos.xy) * tile;
    let last = vec2<i32>(textureDimensions(t_work)) - 1;
    var reach = 0.0;
    for (var y = 0; y < tile; y = y + 1) {
        for (var x = 0; x < tile; x = x + 1) {
            reach = max(reach, -textureLoad(t_work, min(base + vec2<i32>(x, y), last), 0).a);
        }
    }
    return vec4<f32>(reach, 0.0, 0.0, 0.0);
}

@fragment
fn fs_neighbor_max(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let last = vec2<i32>(textureDimensions(t_color)) - 1;
    var reach = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let q = clamp(p + vec2<i32>(x, y), vec2<i32>(0), last);
            reach = max(reach, textureLoad(t_color, q, 0).r);
        }
    }
    return vec4<f32>(reach, 0.0, 0.0, 0.0);
}

fn working_radius() -> f32 {
    return dof.shape.y / dof.shape.z;
}

fn gauss(pos: vec4<f32>, dir: vec2<i32>) -> vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let last = vec2<i32>(textureDimensions(t_color)) - 1;
    let center = textureLoad(t_color, p, 0);
    let radius = max(working_radius(), 1e-4);
    let sigma = max(radius * 0.5, 0.5);
    var sum = center.rgb;
    var total = 1.0;
    for (var i = 1; i <= i32(ceil(radius)); i = i + 1) {
        let g = exp(-f32(i * i) / (2.0 * sigma * sigma));
        for (var side = -1; side <= 1; side = side + 2) {
            let tap = textureLoad(t_color, clamp(p + dir * i * side, vec2<i32>(0), last), 0);
            let w = g * clamp(tap.a / radius, 0.0, 1.0);
            sum += tap.rgb * w;
            total += w;
        }
    }
    return vec4<f32>(sum / total, center.a);
}

@fragment
fn fs_gauss_h(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return gauss(pos, vec2<i32>(1, 0));
}

@fragment
fn fs_gauss_v(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    return gauss(pos, vec2<i32>(0, 1));
}

fn reach(radius: f32, dist: f32) -> f32 {
    return clamp((radius - dist + MARGIN) / MARGIN, 0.0, 1.0);
}

// On the texel it lands on, never between two: a bilinear tap would mix a sharp texel's
// colour into its neighbour's blur (a halo).
fn ring_tap(ring: i32, i: i32, rings: i32, radius: f32) -> vec2<f32> {
    let count = ring * 8;
    let turn = 0.5 * f32(ring & 1);
    let angle = TAU * (f32(i) + turn) / f32(count);
    return round(vec2<f32>(cos(angle), sin(angle)) * radius * f32(ring) / f32(rings));
}

fn rings_for(radius: f32) -> i32 {
    return max(MIN_RINGS, i32(ceil(radius / dof.focus.z)));
}

@fragment
fn fs_bokeh(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let center = textureLoad(t_color, p, 0);
    let last = vec2<i32>(textureDimensions(t_color)) - 1;
    let far = min(max(center.a, 0.0), working_radius());
    let near = min(textureLoad(t_blur, p / i32(dof.shape.w), 0).r, working_radius());
    var bg = vec4<f32>(center.rgb, 1.0);
    if (far >= 0.25) {
        let rings = rings_for(far);
        for (var ring = 1; ring <= rings; ring = ring + 1) {
            for (var i = 0; i < ring * 8; i = i + 1) {
                let offset = ring_tap(ring, i, rings, far);
                let tap = textureLoad(t_color, clamp(p + vec2<i32>(offset), vec2<i32>(0), last), 0);
                // An in-focus tap never spreads and an in-focus pixel takes no
                // background.
                let w = reach(max(min(center.a, tap.a), 0.0), length(offset));
                bg += vec4<f32>(tap.rgb, 1.0) * w;
            }
        }
    }
    var fg = vec4<f32>(0.0);
    var taps = 1.0;
    if (near >= 0.25) {
        let rings = rings_for(near);
        for (var ring = 1; ring <= rings; ring = ring + 1) {
            for (var i = 0; i < ring * 8; i = i + 1) {
                let offset = ring_tap(ring, i, rings, near);
                let tap = textureLoad(t_color, clamp(p + vec2<i32>(offset), vec2<i32>(0), last), 0);
                let w = reach(-tap.a, length(offset)) * smoothstep(0.5, 1.5, -tap.a * dof.shape.z);
                fg += vec4<f32>(tap.rgb, 1.0) * w;
                taps += 1.0;
            }
        }
    }
    let fg_rgb = fg.rgb / max(fg.a, 1e-4);
    let fg_alpha = clamp(2.0 * fg.a / taps, 0.0, 1.0);
    let rgb = mix(bg.rgb / bg.a, fg_rgb, fg_alpha);
    return vec4<f32>(rgb, fg_alpha);
}

@fragment
fn fs_tent(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let last = vec2<i32>(textureDimensions(t_color)) - 1;
    let coc = work_coc(p);
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let q = clamp(p + vec2<i32>(x, y), vec2<i32>(0), last);
            let w = f32((2 - abs(x)) * (2 - abs(y))) * agreement(work_coc(q), coc);
            sum += textureLoad(t_color, q, 0) * w;
            total += w;
        }
    }
    return sum / total;
}

fn agreement(tap: f32, coc: f32) -> f32 {
    let tolerance = max(0.5, 0.25 * abs(coc));
    return 1e-3 + clamp(1.0 - abs(tap - coc) / tolerance, 0.0, 1.0);
}

fn upsampled(at: vec2<f32>, coc: f32) -> vec4<f32> {
    if (dof.shape.z < 1.5) {
        return textureLoad(t_blur, vec2<i32>(at), 0);
    }
    let last = vec2<i32>(textureDimensions(t_blur)) - 1;
    let f = at / dof.shape.z - 0.5;
    let base = vec2<i32>(floor(f));
    let t = f - floor(f);
    var sum = vec4<f32>(0.0);
    var total = 0.0;
    for (var y = 0; y < 2; y = y + 1) {
        for (var x = 0; x < 2; x = x + 1) {
            let q = clamp(base + vec2<i32>(x, y), vec2<i32>(0), last);
            let bilinear = mix(1.0 - t.x, t.x, f32(x)) * mix(1.0 - t.y, t.y, f32(y));
            let w = bilinear * agreement(work_coc(q), coc);
            sum += textureLoad(t_blur, q, 0) * w;
            total += w;
        }
    }
    return sum / total;
}

@fragment
fn fs_composite(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let sharp = textureLoad(t_color, p, 0);
    if (overlay(p)) {
        return sharp;
    }
    let coc = textureLoad(t_coc, p, 0).r;
    let blurred = upsampled(pos.xy, coc);
    var blend: f32;
    if (is_gaussian()) {
        blend = clamp(coc / max(dof.shape.y, 1e-4), 0.0, 1.0);
    } else {
        let far = smoothstep(0.5, 1.5, coc);
        blend = 1.0 - (1.0 - far) * (1.0 - blurred.a);
    }
    return vec4<f32>(mix(sharp.rgb, blurred.rgb, blend), sharp.a);
}
