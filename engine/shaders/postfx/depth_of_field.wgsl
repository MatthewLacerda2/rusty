// engine/shaders/postfx/depth_of_field.wgsl — depth of field (#1445), Unity URP's
// two modes. Fullscreen passes over the HDR scene, before motion blur:
//   fs_coc       — full resolution: each pixel's signed circle of confusion (CoC), in
//                  pixels of radius, from its depth along the view axis of the camera
//                  that drew it (the velocity target's `.z`, #1436): the focus is a
//                  plane, as in Unity, not a sphere. Negative is nearer than the
//                  focus, positive beyond it. A pixel an overlay camera drew (`.w`,
//                  its index in the stack, above 0: a viewmodel) has none.
//   fs_prefilter — working resolution (half on Medium, full on High): the colour,
//                  averaged, and the CoC with the larger magnitude, in working pixels.
//                  The tent and the composite read this reduced CoC (`t_work`).
//   Gaussian: fs_gauss_h, fs_gauss_v — a separable Gaussian, each tap weighted by
//                  its own CoC, so a sharp foreground never bleeds into the blurred
//                  background behind it
//   Bokeh:    fs_tile_max, fs_neighbor_max — per tile, how far the near field around
//                  it reaches (its largest near CoC, then the 3×3 tiles' largest), so
//                  a pixel knows how wide to gather; then
//             fs_bokeh — two disc gathers, each in rings scaled to its own reach and
//                  never further apart than the ring spacing: the far field over the
//                  pixel's own CoC (a tap reaches only as far as both its and the
//                  pixel's CoC allow), the near field over the tiles' near reach (a
//                  tap reaches as far as its own CoC, so a near blur spreads over
//                  what is in focus behind it), then fs_tent,
//                  a 3×3 tent that smooths the rings among texels of like CoC
//   fs_composite — full resolution: the sharp scene blended towards the blur by the
//                  pixel's own CoC, and for Bokeh by the near field's coverage too.
//                  An overlay camera's pixels stay sharp: the gun is in front of
//                  everything the world blurs.
// The composite writes the image motion blur, bloom and the composite then read.

#import fullscreen

struct DofParams {
    // x: mode (1 Gaussian, 2 Bokeh), y: the largest CoC in full-resolution pixels,
    // z: the working-resolution divisor, w: Bokeh's tile size in working pixels (at
    // least the largest working CoC, so a 3×3 of tiles holds every tap's reach)
    shape: vec4<f32>,
    // Gaussian: x start, y end (m). Bokeh: x focus distance (m), y the CoC of a point
    // at infinity (px), z the most working pixels between two rings of the gather.
    focus: vec4<f32>,
};

@group(0) @binding(0) var<uniform> dof: DofParams;
// The pass's input image.
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var t_velocity: texture_2d<f32>;
// The full-resolution CoC (`.r`).
@group(0) @binding(3) var t_coc: texture_2d<f32>;
// The working-resolution blur the composite blends in; for fs_bokeh, the tiles'
// near reach (fs_neighbor_max's output).
@group(0) @binding(4) var t_blur: texture_2d<f32>;
// The prefiltered working image: colour, and the CoC in working pixels (`.a`).
@group(0) @binding(5) var t_work: texture_2d<f32>;

const TAU: f32 = 6.28318530718;
// The fewest rings a Bokeh disc takes: ring k holds 8k taps, so 48 around the
// centre. A disc wider than this many ring spacings (`focus.z`) takes more rings, so
// they never sit further apart than that; the tent smooths between them.
const MIN_RINGS: i32 = 3;
// How many working pixels a tap's reach fades over at its edge.
const MARGIN: f32 = 1.0;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

fn is_gaussian() -> bool {
    return dof.shape.x < 1.5;
}

// The CoC, in full-resolution pixels, of a surface `depth` metres along its camera's
// view axis. `DofParams::coc` in `render::postfx::depth_of_field::params` mirrors it.
fn coc_of(depth: f32) -> f32 {
    let max_coc = dof.shape.y;
    if (is_gaussian()) {
        let span = max(dof.focus.y - dof.focus.x, 1e-4);
        return clamp((depth - dof.focus.x) / span, 0.0, 1.0) * max_coc;
    }
    return clamp(dof.focus.y * (1.0 - dof.focus.x / max(depth, 1e-4)), -max_coc, max_coc);
}

// Whether pixel `p` was drawn by an overlay camera: a higher one in the stack.
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

// The CoC of working texel `q`: of its `div`² pixels', the one with the larger
// magnitude, near or far, in full-resolution pixels.
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
    // Each pixel's colour counts as far as its CoC agrees with the chosen one, so a
    // sharp pixel beside a blurred one keeps its colour out of that blur (a halo).
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

// The CoC of prefiltered texel `q`, in full-resolution pixels.
fn work_coc(q: vec2<i32>) -> f32 {
    return textureLoad(t_work, q, 0).a * dof.shape.z;
}

// One texel per tile: how far its pixels' near blur reaches, in working pixels.
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

// One texel per tile: the largest near reach of its 3×3 tiles, every near blur that
// can spread onto it.
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

// The largest CoC in working pixels.
fn working_radius() -> f32 {
    return dof.shape.y / dof.shape.z;
}

// One direction of the separable Gaussian, over `ceil(radius)` taps either side.
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

// How much of a tap `dist` working pixels away a reach of `radius` covers.
fn reach(radius: f32, dist: f32) -> f32 {
    return clamp((radius - dist + MARGIN) / MARGIN, 0.0, 1.0);
}

// Tap `i` of `count` on ring `ring` of `rings`, spread over a disc of `radius`
// working pixels: on the texel it lands on, never between two, since a bilinear tap
// would mix a sharp texel's colour into its neighbour's blur (a halo). Its weight
// fades with that texel's own distance, so the disc still grows smoothly.
fn ring_tap(ring: i32, i: i32, rings: i32, radius: f32) -> vec2<f32> {
    let count = ring * 8;
    // Odd rings turn half a step, so the rings' taps don't line up in spokes.
    let turn = 0.5 * f32(ring & 1);
    let angle = TAU * (f32(i) + turn) / f32(count);
    return round(vec2<f32>(cos(angle), sin(angle)) * radius * f32(ring) / f32(rings));
}

// How many rings a disc of `radius` working pixels takes: one per `focus.z` (the
// ring spacing), and never fewer than MIN_RINGS, so a small blur is covered densely.
fn rings_for(radius: f32) -> i32 {
    return max(MIN_RINGS, i32(ceil(radius / dof.focus.z)));
}

@fragment
fn fs_bokeh(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pos.xy);
    let center = textureLoad(t_color, p, 0);
    let last = vec2<i32>(textureDimensions(t_color)) - 1;
    // Two discs. The far field spreads over the pixel's own CoC; the near field over
    // the reach of the near blur in the tiles around it. Each gets rings of its own,
    // so a background beside a strongly blurred foreground is still gathered densely.
    let far = min(max(center.a, 0.0), working_radius());
    let near = min(textureLoad(t_blur, p / i32(dof.shape.w), 0).r, working_radius());
    var bg = vec4<f32>(center.rgb, 1.0);
    if (far >= 0.25) {
        let rings = rings_for(far);
        for (var ring = 1; ring <= rings; ring = ring + 1) {
            for (var i = 0; i < ring * 8; i = i + 1) {
                let offset = ring_tap(ring, i, rings, far);
                let tap = textureLoad(t_color, clamp(p + vec2<i32>(offset), vec2<i32>(0), last), 0);
                // As far as the nearer of the two reaches, so an in-focus tap never
                // spreads and an in-focus pixel takes no background.
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
                // As far as the tap's own reach, over anything; a CoC under a
                // full-resolution pixel is in focus (eased in, as the composite's far
                // blend is, so racking focus never steps).
                let w = reach(-tap.a, length(offset)) * smoothstep(0.5, 1.5, -tap.a * dof.shape.z);
                fg += vec4<f32>(tap.rgb, 1.0) * w;
                taps += 1.0;
            }
        }
    }
    let fg_rgb = fg.rgb / max(fg.a, 1e-4);
    // Half the disc covered by the near field hides what lies behind it.
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

// How far a texel whose CoC is `tap` may stand in for a pixel whose CoC is `coc`:
// 1 when they agree, 0 once they part by half a pixel or a quarter of the CoC,
// whichever is more (never quite 0, so a sum never is). A blurred pixel beside a
// sharp one so takes none of its colour, while a CoC that changes smoothly across
// a surface (a floor receding) still blends.
fn agreement(tap: f32, coc: f32) -> f32 {
    let tolerance = max(0.5, 0.25 * abs(coc));
    return 1e-3 + clamp(1.0 - abs(tap - coc) / tolerance, 0.0, 1.0);
}

// The working-resolution blur at full-resolution point `at`, for a pixel whose CoC
// is `coc`: bilinear, each of the four texels weighted by how close its CoC is to
// the pixel's, so a blurred pixel beside a sharp one takes none of the sharp
// texel's colour (and the reverse).
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
        // The pixel's own far blur, under a pixel of radius sharp; then whatever
        // near blur covers it.
        let far = smoothstep(0.5, 1.5, coc);
        blend = 1.0 - (1.0 - far) * (1.0 - blurred.a);
    }
    return vec4<f32>(mix(sharp.rgb, blurred.rgb, blend), sharp.a);
}
