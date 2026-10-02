# In-game UI

rusty's in-game UI is **Unity 5's uGUI, adapted** — HUDs, menus and overlays are built
from ordinary GameObjects. This page is the model: what the pieces are, how layout works,
and the rules that keep it deterministic. The roadmap is the tracking issue #414; the
script surface is in [`api/`](api/index.md) (`Canvas`, `RectTransform`,
`UI`, `Image`, `CanvasGroup`, `RectMask`, `Text`, `Shape`, `Selectable`, `LayoutGroup`,
`LayoutElement`); the widget kit built on them is [*Widgets*](#widgets) below.

> Status: the model, the `Canvas` and `RectTransform` components and the layout pass
> (#417); drawing — `Image`, `CanvasGroup`, `RectMask` and the render pass (#418);
> fonts and SDF `Text` (#419); interaction — hit-testing, pointer and focus callbacks
> and `Selectable` (#420); layout groups and content fitting — `LayoutGroup` and
> `LayoutElement` (#421); the widget kit — Button, Toggle, Slider, Scrollbar,
> Scroll View, Dropdown, Input Field (#422); world-space UI — world and camera
> canvases, world ↔ screen projection and world-anchored markers (#429); the look —
> `Shape`, gradients, glow / shadow and blend modes (#425).

## The model

- **UI is GameObjects.** A `Canvas` entity is a UI root; its descendants carry a
  `RectTransform`. Screens — a HUD, a pause menu, an inventory — are ordinary **prefabs**,
  and their behaviour is Lua script components. The hierarchy, `active`, the inspector,
  prefabs, scene save and the Play/Stop snapshot all apply unchanged. There is no separate
  UI document or markup world, and no egui in the game (egui stays the editor's toolkit).
- **Primitives in Rust, widgets in Lua.** First-class components are the primitives
  (`Canvas`, `RectTransform`, `Image`, `Shape`, `CanvasGroup`, `RectMask`, `Text`, `Selectable`, …);
  Button, Slider, Dropdown and the rest ship as engine Lua scripts + prefabs.

### `Canvas`

The UI root — Unity's `Canvas` and `CanvasScaler` in one component.

| Field | Meaning |
|---|---|
| `render_mode` | `ScreenSpaceOverlay` — drawn over the finished frame. `ScreenSpaceCamera` — a plane in front of the camera. `WorldSpace` — a quad in the scene. See *World-space UI*. |
| `sort_order` | Draw and hit order across canvases: higher is on top and is hit first. |
| `reference_resolution` | The resolution the UI is authored at (default 1920×1080). Layout runs in these **reference units**. |
| `match_width_or_height` | Unity's *Scale With Screen Size*: `0` scales to match the screen's width, `1` its height, in between blends the two in log space. |
| `pixels_per_unit` | `WorldSpace`: reference units per metre (default 100). |
| `plane_distance`, `tilt`, `sway` | `ScreenSpaceCamera`: the plane's distance (metres), its tilt (degrees) and how much it lags camera turns (0–1). |

The **scale factor** (screen pixels per reference unit) is
`2^lerp(log2(screen.w / ref.w), log2(screen.h / ref.h), match)`. A root canvas's rect is
the whole screen: `(0, 0)` to `screen / scale_factor` in reference units — equal to the
reference resolution only when the screen's aspect matches it. The canvas's own Transform
(and any RectTransform on it) is ignored, as Unity drives a root canvas — except a
`WorldSpace` canvas's Transform, which places its quad.

### `RectTransform`

2D placement relative to the parent's rect, beside the mandatory Transform (every entity
still has exactly one Transform). Unity's exact semantics:

| Field | Meaning |
|---|---|
| `anchor_min`, `anchor_max` | A region of the parent rect, as fractions (0..1). Equal anchors pin the element to a point; different anchors stretch it with the parent. |
| `pivot` | The point (a fraction of the element's own rect) the element rotates and scales around, and that `anchored_position` places. |
| `anchored_position` | The pivot's offset from the anchor reference point — the anchors lerped by the pivot. |
| `size_delta` | The element's size minus the anchor region's size. With point anchors this is just the size; with stretch anchors a negative value insets the element. |

So, inside a parent rect `P`:

```
lo    = P.min + anchor_min * P.size          hi = P.min + anchor_max * P.size
size  = (hi - lo) + size_delta
pivot = lo + (hi - lo) * pivot_fraction + anchored_position
min   = pivot - size * pivot_fraction
```

The entity's **Transform keeps rotation and scale**, applied around the pivot and composed
down the hierarchy (a rotated panel carries its children). **Its position is ignored** for
a rect-laid-out entity — the RectTransform places it.

### Coordinates

Reference units, **y-up, origin at the canvas's bottom-left** (Unity's convention).
Screen pixels are the same frame multiplied by the scale factor — also bottom-left, y-up.

## Layout

Layout **runs in the sim, on the CPU** (`src/ui/layout/`). No GPU is involved, so the
headless harness has exactly the window's layout and a bot can reason about — and, with
#420, click — the UI.

- **When.** A `LateUpdate` system recomputes every rect into the `ui_layout` resource each
  tick, after the whole `FixedUpdate` stage (scripts' `Update` and `LateUpdate`, physics,
  animation, destroys). A script's change therefore shows the same tick, and the next
  tick's pointer dispatch reads a settled layout.
- **What.** Each root canvas (a `Canvas` with no `Canvas` ancestor) and every descendant
  with a `RectTransform`. A descendant **without** a `RectTransform` ends the UI subtree:
  it and its children get no rect. A nested `Canvas` lays out like any other rect.
  Inactive entities are still laid out; drawing and hit-testing filter on `active`.
- **Draw order** — canvas `sort_order` (ties keep scene order), then hierarchy pre-order
  within a canvas: a later sibling draws on top, as in Unity. The layout resource is kept
  in this order.
- **Reading it.** `UI.GetRect(id)` computes an element's rect on demand from the live
  scene with the same math (so it works in edit mode, before any Play tick), and
  `Debug.Snapshot` carries it per entity as `ui_rect` — the agent can reason about layout
  without a screenshot. Each reports the final quad's bounds in reference units and in
  screen pixels, the exact corners, the canvas and the scale factor.

### Layout groups

Menus, inventories, scoreboards and tab bars are lists and grids, so their children
are arranged automatically instead of hand-placed. This is Unity's auto-layout model,
ported faithfully:

- **`LayoutGroup`** on an element arranges its **layout children** — the direct children
  that are active, carry a `RectTransform`, and are not `LayoutElement.ignore_layout`.
  `kind` picks a row (`Horizontal`, left to right), a column (`Vertical`, top to bottom)
  or a `Grid` of equal `cell_size` cells. `padding` (left, bottom, right, top) insets
  them; `spacing` is the gap between columns (`x`) and rows (`y`); `child_alignment`
  places them when they do not fill the rect. Every other child keeps its own anchors.
- **Three sizes per axis.** Each child reports a **min** (it never shrinks below it), a
  **preferred** (what it asks for) and a **flexible** weight (its share of leftover
  space). They come from its content — its own group (from *its* children), its `Text`
  (unwrapped width; height wrapped at the width it will get), its `Image` (the texture's
  native size, or for `Sliced` / `Tiled` the sum of its borders) — the largest of
  several wins, and zeros when there is no content.
- **`LayoutElement`** overrides any of those six numbers (`None` keeps the content's),
  and `ignore_layout` takes the element out of its parent's group.
- **Row / column placement** (Unity's `HorizontalOrVerticalLayoutGroup`). Along the main
  axis each child gets its min; as the rect grows it lerps toward preferred; past the
  total preferred, the surplus is shared by flexible weight — or, with nothing flexible,
  the block is aligned. Across, each child fills the inner rect clamped between its min
  and (unless flexible) its preferred, and is aligned. `control_child_size` (width /
  height) lets the group size the children; off, each keeps its `size_delta` and is only
  positioned. `child_force_expand` (width / height) makes every child flexible ≥ 1.
- **Grid placement** (Unity's `GridLayoutGroup`). `constraint`: `Flexible` fits as many
  columns as the width allows, `FixedColumnCount` / `FixedRowCount` use
  `constraint_count`. Cells fill rows first (columns first with `start_vertical`) from
  `start_corner`; the block is aligned by `child_alignment`. A grid is never flexible and
  ignores the control / force-expand flags.
- **Content size fitting.** `LayoutElement.horizontal_fit` / `vertical_fit`
  (`Unconstrained`, `MinSize`, `PreferredSize`) size the element's *own* rect to its
  content around its pivot — a text box that grows downward as it wraps, a list that
  grows with its items (a scroll view's content, #422). The fitter lives on
  `LayoutElement` rather than a third component because it reads the same sizes, and
  keeping it there keeps the component count down. It applies when no parent group
  arranges the element; under a group, the fitted size is what the group keeps on an
  axis it does not control (Unity writes `sizeDelta`, which the group then reads).
- **How it runs.** Inside the same `UiLayout::compute` pass: when the walk reaches a
  group it computes its children's sizes bottom-up — widths first, then heights at the
  resolved widths, since wrapped text and nested grids are taller when narrower — and
  places them top-down. **The scene's RectTransforms are never rewritten**: a group
  overrides the child's rect in the computed layout only, so the pass stays a pure
  function of (scene, screen size), recomputed whole each tick with no dirty flags to go
  stale, and scripts and saved scenes keep the authored values. (Unity marks driven
  properties instead; there is no second source of truth here to drive.) `UI.GetRect` and
  `Debug.Snapshot`'s `ui_rect` report the placed rect.

## Drawing

### `Image`

The rectangle graphic — Unity's `Image`. It fills its entity's laid-out rect (after
rotation and scale) with `color`, optionally multiplied by `texture`. Adding one also
adds a `RectTransform`.

| Field | Meaning |
|---|---|
| `color` | RGBA tint, **display-space (sRGB) values, straight alpha** — what a colour picker shows. |
| `texture` | A path, like material maps; `None` draws a solid colour. `"rt:<name>"` shows a camera's render texture (#430) — a minimap, a scope, a character preview. |
| `image_type` | `Simple` (stretched), `Sliced` (9-slice), `Tiled` (repeated) or `Filled` (partial). |
| `border` | `Sliced`: the frame in texels — left, bottom, right, top. |
| `fill_method`, `fill_origin`, `fill_amount`, `fill_clockwise` | `Filled`: `Horizontal` / `Vertical` bars from an edge, or a `Radial360` sweep from an edge (health bars, cooldown rings, reload circles). |
| `preserve_aspect` | `Simple`: letterbox the texture inside the rect instead of stretching it. |
| `raycast_target` | Whether the pointer can hit it (see *Interaction*). |

**UI sprites.** One texel is one reference unit — Unity's 100-pixels-per-unit sprite on
a 100-reference-pixels-per-unit canvas — so a `Sliced` border or a `Tiled` tile keeps its
authored size whatever the rect's size, and scales with the canvas. Author sprites at the
reference resolution (1920×1080 by default) and export PNGs with straight alpha. UI
textures are sampled with linear filtering at their base level (no mips — UI draws near
1:1, where mips only blur). A `Sliced`/`Tiled` image without a texture draws as `Simple`;
a `Tiled` image caps itself at 1024 tiles by growing the tile.

### `CanvasGroup`, `RectMask` and `Mask`

- **`CanvasGroup`** — `alpha` multiplies every graphic on the entity and below it;
  nested groups multiply (screen fades, disabled panels). `interactable = false`
  disables every `Selectable` below it, and `blocks_raycasts = false` lets the pointer
  pass through the whole subtree (see *Interaction*).
- **`RectMask`** — Unity's `RectMask2D`: the entity's own graphic and its whole subtree
  are clipped to the axis-aligned screen bounds of its rect, inset by `padding`; nested
  masks intersect. The clip is axis-aligned, so a rotated mask clips to its bounding
  box (as in Unity). **`feather`** (reference units, #428) softens it: content fades
  out over that distance inside each edge — the faded ends of a scrolling list. When
  masks nest, each edge of the intersection keeps the feather of the mask it came
  from, so a soft list inside a hard panel fades only at the list's own edges.
- **`Mask`** (#428) — Unity's `Mask`: the subtree *below* the entity is clipped to
  the entity's own graphic — its `Image` (texture alpha times colour alpha), else
  its `Shape` (#425: the SDF's antialiased coverage), else its rect. An `Ellipse`
  Shape or a circle sprite makes a round minimap — an `Image` showing
  `"rt:minimap"` under it — a soft-edged sprite a feathered one, a `Filled` radial
  image or a `Ring` arc a radial wipe.
  `show_mask_graphic` (Unity's `showMaskGraphic`) says whether the mask's graphic
  also draws; off, it only shapes the clip. Masks nest to any depth (they multiply)
  and combine with every `RectMask` above. Hit-testing clips to the mask's rect —
  Unity's rule — not its alpha: a click in a round minimap's corner still reaches it.
- **`BackdropFilter`** (#426) — frosted glass, CSS's `backdrop-filter`: the frame
  behind the entity's graphic is blurred by `blur_radius` (reference units), then
  desaturated (`saturation`, 1 unchanged), multiplied by `brightness` and mixed
  toward `tint` by its alpha, and shown through the graphic's shape (its `Image`'s
  texture alpha, else its `Shape`, else its rect) *under* the graphic. The graphic's colour alpha does
  not hide it — an `Image` with alpha 0 is pure glass, a dark half-transparent one
  darkens it further — while a `CanvasGroup` fades it like any graphic. The pause
  screen of a modern shooter: a full-screen panel with a 24-unit blur and a 0.25
  black tint. **Screen-space (overlay) canvases only** — see *Masks and backdrops*
  below.

A separate `BackdropFilter` component rather than fields on `Image` keeps the cost
visible where it is paid (one more batch, and the blur) and leaves `Image` the plain
uGUI graphic; it works on any graphic that shapes it, `Shape` included. A backdrop
batch always composites "over", whatever the graphic's blend mode — the mode
applies to the graphic drawn on top.

### `Text`

The label — TextMeshPro's `TextMeshProUGUI`, trimmed. It draws `text` inside its
entity's laid-out rect (after rotation and scale). Adding one also adds a
`RectTransform`.

| Field | Meaning |
|---|---|
| `text` | The string. `\n` breaks a line; with `rich_text`, the tag subset below styles runs. |
| `font`, `font_bold`, `font_italic` | Font asset paths (`.ttf` / `.otf`, like textures). `font: None` is the bundled default; without a bold / italic face, `<b>` / `<i>` are synthesized. |
| `font_size` | The **em** size in reference units (as in Unity and CSS). |
| `color` | Fill, display-space RGBA with straight alpha. |
| `alignment` | Where the block sits in the rect: Unity's nine anchors, `TopLeft` … `BottomRight`. Each line aligns horizontally on its own. |
| `wrap` | Break lines at word boundaries to fit the rect's width (a word wider than the rect breaks between characters). Off: only `\n` breaks. |
| `overflow` | `Overflow` spills past the rect. `Truncate` keeps only the lines that fit its height entirely (and, unwrapped, cuts characters past its right edge). `Ellipsis` truncates and ends the last kept line with `…` (`...` if the font lacks it). |
| `line_spacing` | Line pitch multiplier (1 = the font's ascent + descent + line gap). |
| `letter_spacing` | Extra advance per character, in ems. |
| `auto_size`, `auto_size_min`, `auto_size_max` | Pick the largest size in `[min, max]` at which the whole text fits the rect (the minimum when none does; `overflow` then applies). A fixed-step binary search, so the pick is a pure function of the inputs. |
| `rich_text` | Parse the tag subset. Off draws tags literally (echoing user input). |
| `raycast_target` | Whether the pointer can hit it (see *Interaction*). |
| `outline_width`, `outline_color` | An outline grown outward from the glyph edge, in ems. |
| `shadow_offset`, `shadow_color` | A drop shadow — a copy of the (outlined) glyphs offset in reference units, drawn beneath the whole label. |
| `glow_size`, `glow_color` | A soft glow fading out past the (outlined) edge over `glow_size` ems — neon labels. |

**Rich text** is a deliberately small subset: `<color=#rrggbb>` / `<color=#rrggbbaa>`,
`<b>`, `<i>` and `<size=n>` (reference units; under auto-size it scales with the
picked size), each closed by `</color>`, `</b>`, `</i>`, `</size>`, nesting freely.
An unknown or malformed tag, or a close with nothing open, draws literally — nothing
the author typed silently vanishes. There is no layout engine beyond that.

**Fonts.** `.ttf` / `.otf` files referenced by path, parsed with `ab_glyph` and cached
for the process; a path that does not load falls back to the default with one
warning. The bundled default is **Instrument Sans** (`assets/fonts/`), under the SIL
Open Font License 1.1 — its license text ships beside it
(`assets/fonts/InstrumentSans-OFL.txt`) and must travel with any build that includes
the font. `cargo deny` checks crate licenses, not assets, so keep font licenses here.

**Shaping: kerning only (Latin).** Glyphs map one-to-one from characters and pairs
are kerned from the font's GPOS `kern` feature (what modern fonts ship), falling
back to the legacy `kern` table. Ligatures, combining marks, right-to-left and CJK
line breaking need full shaping — a later localization concern, not built here.

**Layout is CPU, in the sim's terms.** Measuring, wrapping, overflow and auto-size
(`src/ui/text/`) never touch the GPU: headless layout is the window's, and
`Text.GetPreferredSize` and the layout groups read the same numbers the
renderer draws. Coordinates are rect-local reference units.

**Drawing: signed distance fields.** Each glyph's field is generated on first use —
rasterized 4× supersampled at a 48 px em, measured with an exact Euclidean distance
transform — and packed into its font's single-channel atlas (1024 wide, doubling in
height up to 4096 as it fills), uploaded only when a new glyph arrived. The field
reaches 12 atlas pixels (**0.25 em**) past the edge: outline + glow + synthesized
bold are capped to that. The shader cuts fill, outline and glow from the one
distance, antialiased over one screen pixel, so text stays sharp at any size or
scale. Synthesized italic shears the glyph quad; synthesized bold dilates the field
and widens the advance slightly.

### Look: shapes, gradients, effects and blend modes

The vocabulary that makes a HUD look designed rather than debug (#425): most of a
Cyberpunk-style interface is thin cut-corner frames, additive neon lines, soft glows
and gradients — built here from parameters, not bitmaps.

**`Shape`** is a texture-free graphic drawn from a **signed distance field**, so it
stays crisp at any scale and costs one quad. It fills its entity's laid-out rect
(rotation and scale included; adding one also adds a `RectTransform`):

| Field | Meaning |
|---|---|
| `kind` | `Rect`, `Ellipse` (inscribed), `Ring` (a circle of the rect's smaller half-extent) or `Line` (along the rect's horizontal centre line — rotate the RectTransform to aim it). |
| `corner`, `radius` | `Rect`: per-corner size (top-left, top-right, bottom-right, bottom-left) that either rounds (`Round`) or cuts at 45° (`Chamfer`). |
| `inner_radius`, `arc_start`, `arc_end` | `Ring`: the hole (0 = disc or pie) and the arc, degrees clockwise from 12 o'clock — crosshair arcs, radial progress. A sweep ≥ 360° is the whole ring. |
| `thickness`, `dash`, `gap` | `Line`: thickness and dash pattern (dash 0 = solid), starting at the rect's left edge. |
| `color` / `gradient` | The fill: a colour, or a gradient (below). |
| `border_width`, `border_color` | The outline: a band inside the edge. |
| `shadow` | `offset`, `blur`, `color` — a soft copy under the shape (alpha 0 = off). |
| `glow` | `size`, `intensity`, `color` — a halo fading out from the edge over `size` (quadratic falloff); `intensity` > 1 runs hotter, best with `Additive`. |
| `blend`, `raycast_target` | As on every graphic. A `Selectable`'s `ColorTint` tints a Shape like an Image, so a Button can be skinned entirely with shapes. |

**Gradients** replace an `Image`'s tint (multiplying its texture) or a `Shape`'s fill:
2–4 colour stops swept `Linear`ly at an `angle` (corner to corner) or `Radial`ly from a
`center` out to a `radius`, all in fractions of the rect so they scale with it. Stops
interpolate in display space, like everything else the UI blends.

**Blend modes** — `Normal`, `Additive`, `Multiply`, `Screen` — are on every graphic
(`Image`, `Text`, `Shape`). They act in display space on premultiplied colour, so they
read as an image editor's layer modes: `Additive` brightens (neon, glows, hit
flashes), `Multiply` darkens (white leaves the backdrop unchanged), `Screen` lightens
(black leaves it unchanged).

**Effects by graphic.** Shape and Text are both SDF graphics, and both cut their
effects from their own field: a Shape has `shadow` and `glow` (its `border` is the
outline), a Text has `outline`, `shadow` and `glow` (above). A textured `Image` has
none yet: its effects need a blurred-alpha pass of the texture, deferred to a
follow-up — put a `Shape` behind it for a frame, glow or shadow.

**Recipe — a Cyberpunk panel.** Dark glass with cut corners, a neon edge and glow, a
gradient header strip and an additive dashed scan line (`hud` is a canvas; this exact
script is exercised by `tests/ui_look_api.rs`):

```lua
-- A Cyberpunk panel: dark glass with cut corners, a neon edge and glow, a
-- gradient header strip and an additive dashed scan line. `hud` is a canvas.
local function element(name, parent, x, y, w, h)
  local id = Scene.CreateEntity(name)
  Scene.SetParent(id, parent)
  Scene.AddComponent(id, "Shape") -- also adds a RectTransform
  RectTransform.SetAnchorMin(id, 0, 0)
  RectTransform.SetAnchorMax(id, 0, 0)
  RectTransform.SetPivot(id, 0, 0)
  RectTransform.SetAnchoredPosition(id, x, y)
  RectTransform.SetSizeDelta(id, w, h)
  return id
end

local panel = element("Panel", hud, 64, 64, 480, 220)
Shape.SetCorner(panel, "Chamfer")
Shape.SetRadius(panel, 24, 0, 24, 0)            -- cut top-left and bottom-right
Shape.SetColor(panel, 0.02, 0.05, 0.08, 0.85)   -- dark glass
Shape.SetBorder(panel, 2, 0.0, 0.95, 1.0, 1.0)  -- neon cyan edge
Shape.SetGlow(panel, 14, 0.8, 0.0, 0.95, 1.0, 1.0)
Shape.SetShadow(panel, 6, -6, 8, 0, 0, 0, 0.6)

local header = element("Header", panel, 24, 180, 432, 24)
Shape.SetCorner(header, "Chamfer")
Shape.SetRadius(header, 0, 12, 0, 0)
Shape.SetGradient(header, { angle = 0, stops = {
  { t = 0, color = { 1.0, 0.1, 0.4, 0.9 } },
  { t = 1, color = { 1.0, 0.1, 0.4, 0.0 } },
} })

local scan = element("ScanLine", panel, 24, 160, 432, 4)
Shape.SetKind(scan, "Line")
Shape.SetThickness(scan, 2)
Shape.SetDash(scan, 12, 6)
Shape.SetColor(scan, 0.0, 0.95, 1.0, 0.6)
Shape.SetBlend(scan, "Additive")
```

It costs **two draw calls**: the panel (its shadow, glow and body) and the header
batch together as `Normal` solid geometry; the scan line's `Additive` is a new batch.

### The pass

- **When.** Inside `Renderer::render`, **after the whole post-FX chain** (after FXAA):
  a HUD is never tonemapped, bloomed, motion-blurred or FXAA-softened. It draws onto
  the frame each consumer presents: the view's own colour target for the editor's Game
  view and headless screenshots, and the swapchain frame the standalone player hands
  its targetless view (`RenderView::set_ui_output`). The Scene view (edit-mode camera)
  draws the game's screen UI only while its **UI** overlay toggle is on (see
  *Authoring in the editor*); a targetless view given no output (the
  reflection-probe cubemap capture) never does.
- **What.** The layout is recomputed for the view's pixel size with the same pure
  `UiLayout::compute` the sim runs, then walked in draw order. A graphic draws when it
  and every ancestor are `active`.
- **Colour space.** UI blends in **display (sRGB-encoded) space with premultiplied
  alpha**, the way designers author it: 50% white over black is mid-grey `128`, as in
  an image editor, not the `188` a linear-space blend gives. The frame's target is sRGB
  (the one gamma encode, #415), so the pass draws through a **non-sRGB view** of it: the
  hardware then blends the encoded values directly. An in-shader encode cannot do this
  — on an sRGB target the blend unit decodes the destination to linear before blending,
  whatever the shader wrote — so the aliased view is the only way to get the authored
  look without giving up the sRGB target. Textures are sRGB and sampled to linear, so
  the shader re-encodes them before tinting. (A GPU that cannot alias a texture's view
  format — some GL drivers — falls back to blending in linear space: a little off, never
  broken.)
- **Batching.** One vertex buffer per canvas per view. Consecutive graphics sharing a
  source — a texture, or a font's atlas — a blend mode, a clip and a custom shader
  (below) are one draw call; a change of any starts the next — so a HUD of solid bars is one draw call however
  many bars it has, and a label one draw call however many glyphs. Shapes are solid,
  so they batch with solid images. Each blend mode is its own pipeline, so a HUD
  alternating `Normal` and `Additive` graphics pays a draw per switch: keep additive
  elements adjacent in hierarchy order (`RenderCounters::ui_draws` reports the count).
  An entity with several graphics draws its `Image`, then its `Shape`, then its `Text`.
- **Dirty.** Each view keeps every canvas's mesh beside its buffer. Rebuilding the CPU
  mesh is a cheap walk; the buffer is re-uploaded only when the canvas's geometry
  changed (a layout or graphic change) and reallocated only when it outgrows its
  capacity, so a static HUD uploads nothing per frame.

### Masks and backdrops

Every batch — overlay or world — binds one **batch group** (`render::ui::effects`):
its slot of the view's batch uniform (its `RectMask` bounds and per-edge feather in
canvas NDC, a backdrop's tint and filter), the nearest `Mask`'s coverage texture, the
blurred frame for a backdrop, and a sampler. The fragment shader multiplies every
graphic by that clip's coverage (`clip_coverage` in `ui.wgsl`); a batch with no
mask binds white. A change of mask, feather or backdrop breaks the batch like a
change of clip.

- **Hard rect clips** still also scissor on overlay canvases (cheap pixel
  rejection); the shader's per-fragment cut is what clips a world canvas and what
  feathers.
- **`Mask`: a coverage texture, not the stencil.** Each visible Mask's graphic is
  drawn into an R8 texture the size of its canvas's frame (the screen, or a world
  canvas's own rect) before the camera stack, through `fs_mask`: the graphic's alpha
  times the clip of every mask above it, which that draw samples. So a mask's
  texture is already the product of its whole chain, a masked batch samples exactly
  one texture, and nesting has no depth limit — a stencil's increment/decrement
  scheme (Unity's) runs out of bits and can only cut hard, while the texture carries
  soft coverage. The cost is one R8 target and one small pass per visible Mask
  (`ui_mask_passes`). Being addressed in canvas NDC, the same texture serves overlay
  and world canvases: **masks work on world canvases too**.
- **Backdrop: re-composited, not copied.** The overlay pass draws onto the finished
  frame — often a swapchain image, which can be neither copied nor sampled. So right
  before the overlay pass the blur **re-runs the post-FX composite** over the view's
  HDR scene colour (after the camera stack's last stage, post-FX, #636) into its
  first level at 1/2 of the view's size (1/4 on Low — the `Graphics` quality tier's
  bloom divisor). The backdrop is therefore the graded, tonemapped, bloomed 3D frame
  — everything but FXAA and authored post-FX effects, which a blur erases anyway —
  and **not UI drawn before it**: a pause panel over the HUD shows blurred world,
  not blurred HUD. Then a **dual-filter** chain (Bjørge, SIGGRAPH 2015; `ui_blur.wgsl`)
  halves down to the deepest level any backdrop needs and climbs back per level; each
  level doubles the reach, `level = ceil(log2(radius_px / (2 × divisor)))`, capped at
  6. One blur per distinct level per frame across every canvas; **none** when no
  backdrop is visible. `ui_blur_passes` counts them (the composite included).
- **Why overlay only.** A world or camera canvas draws inside the camera pass, into
  the HDR target before post-FX, when no finished frame exists to blur; frosted glass
  in the world would need a mid-stack copy of the HDR target per camera. On those
  canvases a `BackdropFilter` is ignored and the graphic draws as it would without
  one.

### Custom shaders

An `Image`, a `Shape` or a `Text` can draw through an authored **ui shader** instead of the
standard one — Unity's `Graphic.material`: the glitch, scanlines, RGB split,
hologram flicker, dissolve and wipes of a Cyberpunk-style HUD. The agent bakes one
with `Shader.Bake` and `pass = "ui"` (blocks and params in
[`api/Shader.md`](api/Shader.md#the-ui-pass)), names it on the graphic, and drives its
runtime params from a script:

```lua
Shader.Bake({ pass = "ui", name = "hud_damage",
              blocks = { { id = "rgb_split" }, { id = "glitch_slices", params = { amount = 0 } } } })
UI.SetShader(healthBar, "hud_damage")                          -- Image, Shape and Text alike
UI.SetShaderParam(healthBar, "glitch_slices.amount", 16)       -- on hit; ease it back to 0
```

- **What it changes.** A ui shader is the standard UI shader with one function
  replaced: the graphic's own shaded colour — texture, tint or gradient, a Shape's SDF
  with its shadow and glow, SDF text with outline and glow, CanvasGroup alpha — goes
  through the blocks before it blends. Everything else is the standard pass:
  display-space premultiplied blending, the blend mode, the RectMask clip and
  feather, the `Mask` coverage (see *Masks and backdrops*), and on world canvases the
  depth test and fog. A custom-shaded graphic is clipped and masked exactly like any
  other. A shader restyles what a graphic *draws*, not what it *cuts*: a `Mask`'s
  coverage and a `BackdropFilter`'s shape come from the graphic's standard shading.
- **Per graphic.** Each shaded graphic is its own draw with its own small uniform: its
  rect (rotation included, so `uv` is rect-local), the UI clock and its runtime param
  values — two Images naming the same shader keep their own `dissolve.amount`. A
  graphic's glyphs share it, so a shaded label is still one draw call.
- **The clock is unscaled.** UI shaders animate on unscaled time (`Time.unscaledTime`):
  a pause menu at `Time.SetTimeScale(0)` keeps flickering, as its tweens and fades do.
  It is `0` in edit mode, like the scene shaders' clock, so edit-mode shots stay
  pixel-comparable; `Time.Pause` (the agent's loop freeze) stops it with everything else.
- **Fail-safe.** A missing shader, one that does not compile, or one baked for another
  pass is logged once and the graphic draws with the standard shader. A re-bake is
  picked up next frame.
- **Saved with the scene** on the graphic (`shader: { name, params }`); the inspector's
  Image, Shape and Text cards edit the name (*UI Shader*). Runtime params are a script's job.

## Interaction

Unity's `EventSystem`, `StandaloneInputModule` and `GraphicRaycaster`, run **in the
sim, on the CPU** (`src/ui/events/`): a headless run clicks exactly what a window
would, and a replay of the same inputs fires the same callbacks. Once per tick, at
the head of the script phase (after `Awake`/`Start`, before `Update`), the event
system reads this tick's input and last tick's settled layout and fires the UI
callbacks straight into the scripts — no event bus. The callbacks and the order
they fire in are listed in *Script lifecycle callbacks* in [`api/index.md`](api/index.md#script-lifecycle-callbacks).

### Hit-testing

The pointer is in **UI screen pixels** — bottom-left origin, y-up, the frame of
`UI.GetRect(id).screen` (`Input.GetMousePosition` is top-left, so `y` flips). The
**top-most** hit wins: canvases by `sort_order`, then the reverse of hierarchy
pre-order (the last-drawn graphic is on top). A graphic is hit when it is an
`Image` or `Text` with `raycast_target`, it and every ancestor are `active`, no
`CanvasGroup` on it or above it has `blocks_raycasts = false`, the point is inside
every `RectMask` on its chain (padding applied, the same clip drawing uses) and
inside the rect of every `Mask` above it (Unity's rule: the mask's rect, not its
alpha; a feather does not shrink the hit area), and the point is inside its
**final quad** — rotation and scale included. A fully
transparent graphic still blocks (Unity's default). While the cursor is locked
(mouse-look) the pointer is off the UI.

### Pointer events

- **Hover.** The hit entity and every ancestor are *inside* the pointer. When the
  hit changes, what was left gets `OnPointerExit`, what was reached gets
  `OnPointerEnter` — deepest first. Moving from a button onto its own label leaves
  the button inside: it neither exits nor re-enters.
- **Press → click.** Down, up and click go to **one** entity: the nearest one from
  the hit upward whose scripts define any of the three, or that carries a
  `Selectable`. A click fires on release when the pointer is still over that same
  entity. Left, right and middle buttons each track their own press (`event.button`).
- **Drag.** The nearest entity defining a drag callback. Once the held pointer has
  moved **10 pixels** (Unity's threshold): `OnBeginDrag`, then `OnDrag` on every
  tick it moves, then `OnEndDrag` on release. A drag taken by a *different* entity
  than the press cancels the press — it gets `OnPointerUp` and no click (dragging
  a scroll view from one of its buttons).
- **Scroll.** The wheel goes to the nearest `OnScroll` handler.

This differs from Unity in one deliberate way: Unity bubbles down, up and click
each to their own nearest handler, so a child handling only `OnPointerDown` can
silently swallow its parent's click. Here the three share one owner.

### `Selectable`

Unity's `Selectable`, the base every widget (#422) builds on. Adding one also adds
a `RectTransform`.

| Field | Meaning |
|---|---|
| `interactable` | Whether it accepts input. A `CanvasGroup` with `interactable = false` above it disables it too. |
| `transition` | How its state shows: `ColorTint`, `SpriteSwap` or `None`. |
| `target_graphic` | The entity whose Image (and Text) shows the state; none is its own entity. |
| `colors`, `fade_duration` | `ColorTint`: a colour per state, multiplied into the target's colour, fading linearly over `fade_duration` seconds of **unscaled** time (a menu under `Time.SetTimeScale(0)` still animates). |
| `sprites` | `SpriteSwap`: a texture per state, shown instead of the target Image's; `Normal` shows the Image's own. |
| `navigation` | How keyboard focus leaves it: `Automatic`, `Explicit` or `None` (never focused by navigation). |
| `select_on_up/down/left/right` | `Explicit` targets. |

**States**, first match wins: `Disabled` (not interactable) → `Pressed` (the left
button pressed it and the pointer is still inside) → `Selected` (it has the focus)
→ `Highlighted` (the pointer is inside) → `Normal`. A left press focuses the nearest
interactable, navigable Selectable under the pointer — and a press anywhere else
clears the focus, as in Unity. A Selectable that is **not interactable cannot start
an interaction**: press, drag, scroll, submit and cancel stop at it without firing
(Unity leaves that check to each widget; here every Lua widget gets it for free).
Hover still reports enter and exit.

The transition runs in `LateUpdate`, after layout, and writes two **runtime-only**
slots on the target — its colour multiplier (Unity's `CanvasRenderer` colour) and
its override sprite (`Image.overrideSprite`). Neither is saved, so the authored
`color` and `texture` never change and Stop restores nothing extra. In edit mode
nothing runs: graphics show untinted.

**References.** `target_graphic` and the explicit targets are entity ids. Saving a
prefab rewrites them to the prefab's local ids (a target outside the saved subtree
is dropped), stamping rewrites them to the new instance's ids, and a linked
instance's propagation compares them in its own ids — so a button prefab keeps
pointing at its own icon however many times it is placed.

### Keyboard and gamepad focus

The focused ("selected") entity is what the keyboard and gamepad drive. Input
becomes **logical actions** first, and only those drive focus (Unity's
`InputSystemUIInputModule` defaults; every pad slot counts):

| Action | Keyboard | Gamepad |
|---|---|---|
| Move | arrows | d-pad (`PADUP` …), left stick |
| Next / Previous | Tab / Shift+Tab | — |
| Submit | Enter, keypad Enter | `PADA` (south) |
| Cancel | Escape | `PADB` (east) |

**A held Move repeats** (#672): the press moves once, then again after **0.5 s**,
then every **0.1 s** while the same direction is held (Unity's `moveRepeatDelay` /
`moveRepeatRate`). The clock is the tick's **unscaled** dt, so a paused menu
repeats and a headless run repeats on the same ticks every time. The **left stick**
counts once it leans at least **0.5** (Unity's press point, on top of the pad's
dead zone), and everything held — arrows, d-pads, sticks — sums to one direction
by its dominant axis (vertical on a tie), so a diagonal moves once, not twice. A
stick has no press edge: leaning into a new direction is the press.

- **Move** follows the focused Selectable's `navigation`: `Explicit` takes the
  target for that direction; `Automatic` picks, among the *candidates*, the centre
  in front of the focused rect's edge maximizing `dot(direction, offset) /
  distance²` (Unity's `FindSelectable`); `None` stays put. Nothing focused, nothing
  moves.
- **Next / Previous** cycle the candidates in draw order, wrapping, from the first
  (last) when nothing is focused.
- **Submit / Cancel** fire `OnSubmit` / `OnCancel` on the focused entity itself.

A *candidate* is a visible, interactable Selectable whose navigation is not `None`.
Every change of focus — a click, navigation or `UI.SetSelected` — fires
`OnDeselect` on the old entity, then `OnSelect` on the new one; a script's
`SetSelected` is announced at the head of the next tick. A focused entity that goes
inactive or is destroyed loses the focus.

### Gameplay vs UI, and the agent

`UI.IsPointerOverUI()` is Unity's `IsPointerOverGameObject`; `UI.IsPointerConsumed()`
also stays true while a press that began over the UI is held, so dragging a slider
off its edge never fires the weapon. Gameplay guards pointer input with it — the
pattern is in [`api/UI.md`](api/UI.md). For bots and tests, `UI.Raycast(x, y)` names what
is under a point, `UI.List()` lists every visible Selectable with its name, state
and screen rect, and `UI.Click(id)` clicks one **through the real input path** (a
covering modal or a locked cursor makes it miss, as it would a player).

## Widgets

The standard kit (#422): **Button, Toggle (+ Toggle Group), Slider, Scrollbar,
Scroll View, Dropdown and Input Field**. Each is a small tree of the primitives above
plus one engine-shipped Lua **script component** that gives it behaviour — not a
first-class component, so there is no four-axis gate, and a game restyles a widget
by editing its components or forks its behaviour by copying its script.

- **Creating one.** The editor's **GameObject ▸ UI** menu (Canvas, Panel, Image,
  Text and each widget) and `UI.Create(kind, [parent])` build the same tree through
  one path (`scene::authoring::ui_widgets`): under the selected entity when it is
  inside a canvas, else under the scene's first root canvas, else under a new
  `Canvas`. Every widget also ships as a prefab, `project/prefabs/ui/<Kind>.prefab`
  (`Button.prefab`, `Scroll View.prefab`, …), for `Scene.Instantiate(path, parent)`.
- **Where they live.** The scripts ship in `assets/scripts/ui/` and are seeded,
  with the prefabs, on every boot into `project/assets/scripts/ui/` and
  `project/prefabs/ui/`. **Those two directories are engine-owned and rewritten**
  so they never go stale: to fork a widget, copy its script (or prefab) elsewhere
  and point the entity at the copy.
- **Wiring one up.** A widget raises its events by calling function fields its
  owner sets — no event bus. The owner reaches the widget's script instance with
  `Scene.GetScript(id, name)` (Unity's `GetComponent<T>()`, `name` the file stem),
  in `Start` or later (every `Awake` has run by then):

  ```lua
  function Menu.Start(id)
      local volume = Scene.GetScript(Scene.FindChild(id, "Volume"), "slider")
      volume.on_value_changed = function(_, v) Audio.SetMasterVolume(v) end
      local quit = Scene.GetScript(Scene.FindChild(id, "Quit"), "button")
      quit.on_click = function() Application.Quit() end
  end
  ```

- **Finding their parts.** A widget script reaches its own children by name with
  `Scene.FindChild(id, "Handle Slide Area/Handle")`, never a stored id, so a prefab
  stamp or a duplicate keeps working. Renaming a part detaches it.
- **Keyboard and gamepad.** Widgets are `Selectable`s, so Tab, the arrows, the
  d-pad and stick, Enter and pad `A` reach them. A widget that needs Move itself
  defines `OnMove` (Unity's
  `IMoveHandler`): the move goes to it instead of navigation, and it navigates on
  its own with `UI.FindSelectable` — a slider steps on Left / Right and moves the
  focus on Up / Down.
- **Time.** Every widget animates and scrolls on **unscaled** time, so a pause
  menu under `Time.SetTimeScale(0)` still works.
- **On a world canvas** (#429) every widget works as on the screen: the pointer's
  camera ray reaches it (the screen centre while the cursor is locked), and the
  widgets map the pointer with `event.canvas_position` / `canvas_delta` — the
  pointer in the canvas's own reference units — never screen pixels. A dropdown
  there opens its list on its own canvas (top-most child) rather than a popup
  overlay. Dragging needs a free cursor; a locked one clicks.

Each widget's inspector fields (its script's `fields` schema) and owner API:

| Widget (script) | Fields | Owner API |
|---|---|---|
| **Button** (`button`) | — | `on_click(id)`; `press()` |
| **Toggle** (`toggle`) | `is_on`, `fade` (checkmark fade, s) | `is_on`; `on_value_changed(id, on)`; `set_is_on(on)`, `set_is_on_without_notify(on)` |
| **Toggle Group** (`toggle_group`) | `allow_switch_off` | `active()`; `set_all_off()` |
| **Slider** (`slider`) | `min`, `max`, `value`, `whole_numbers`, `direction` | `value`; `on_value_changed(id, v)`; `set_value(v)`, `set_value_without_notify(v)`; `normalized()` |
| **Scrollbar** (`scrollbar`) | `value`, `size`, `number_of_steps`, `direction` | `value`, `size`; `on_value_changed(id, v)`; `set_value(v)`, `set_value_without_notify(v)`; `set_size(s)` |
| **Scroll View** (`scroll_view`) | `horizontal`, `vertical`, `movement_type`, `elasticity`, `inertia`, `deceleration_rate`, `scroll_sensitivity` | `on_value_changed(id, x, y)`; `get_normalized_position()`, `set_normalized_position(x, y)`; `stop_movement()` |
| **Dropdown** (`dropdown`) | `options` (`\|`-separated), `value` (0-based), `item_height`, `font_size` | `value`; `on_value_changed(id, i, text)`; `set_value(i)`, `set_value_without_notify(i)`; `get_options()`, `set_options(list)`; `show()`, `hide()`, `is_expanded()` |
| **Input Field** (`input_field`) | `text`, `char_limit`, `content_type`, `line_type`, `caret_blink_rate` | `text`; `on_value_changed(id, t)`, `on_submit(id, t)`, `on_end_edit(id, t)`; `set_text(s)`, `set_text_without_notify(s)` |

`direction` is `LeftToRight`, `RightToLeft`, `BottomToTop` or `TopToBottom`.

- **Button** — clicks with the left button, or Enter while focused. Its state
  colours are the `Selectable`'s ColorTint on its own Image.
- **Toggle** — `Background/Checkmark` fades in and out with a `Tween`. Clicking
  the label toggles too. Under an ancestor carrying `toggle_group.lua` it is a
  radio button: the group keeps at most one on (exactly one unless
  `allow_switch_off`; clicking the on one then does nothing). **Toggle Group** in
  the menu makes such a column with two options.
- **Slider** — `Fill Area/Fill` stretches from the start to the value;
  `Handle Slide Area/Handle` sits on it. A press anywhere sets the value; dragging
  follows the pointer. The arrows along its axis step a tenth of the range (1 with
  `whole_numbers`).
- **Scrollbar** — `Sliding Area/Handle` spans `size` of the track at `value`.
  Drag the handle; press the track beside it to page one handle-length that way.
- **Scroll View** — `Viewport` (a `RectMask`) over `Content`, which is pinned to
  the viewport's top-left and fits its height to its children (a vertical
  `LayoutGroup` + `LayoutElement` preferred-height fit) — so adding items is
  parenting them to `Content`. Drag (the drag threshold passes a press on a child
  button to the view) and the wheel scroll it within the content's bounds;
  `Elastic` rubber-bands past an edge while dragged and springs back over
  `elasticity` seconds (Unity's `SmoothDamp`), `Clamped` stops at it,
  `Unrestricted` never stops; `inertia` coasts a released drag, keeping
  `deceleration_rate` of its speed per second. Its `Scrollbar Horizontal` /
  `Scrollbar Vertical` children follow the content and drive it (normalized
  position: `x` 0 = left, `y` 1 = top, as in Unity).
- **Dropdown** — shows the chosen option on its `Label`. Clicking it (or Enter)
  opens the list: its inactive `Template` (a scroll view) is lifted onto a popup
  root canvas at `sort_order` 30000 with the dropdown's canvas scaling, below the
  dropdown (above it when the screen has no room), over a transparent full-screen
  `Blocker` — a click outside closes the list and reaches nothing underneath. The
  list has one `Item` per option (a Selectable, so it highlights; the chosen one
  carries a checkmark), navigable Up / Down; a click or Enter picks, Escape
  closes, and focus returns to the dropdown.
- **Input Field** — a masked `Text Area` holding `Selection` (the highlight
  rects), `Placeholder` (shown while empty), `Text` and the `Caret`. Focusing it
  by keyboard selects everything; a click places the caret and a drag selects.
  While focused it takes typed text (`Input.GetTextInput`), Backspace, Delete,
  Left / Right, Home / End, Shift to extend a selection and Ctrl / Cmd+A to select
  all; `SingleLine` sends Up / Down to the start / end and Enter to `on_submit`,
  `MultiLine` moves between lines and breaks the line on Enter. Escape restores
  the text it had when focused and lets go. `content_type`: `Standard`,
  `Integer`, `Decimal` or `Password` (shown as `*`); `char_limit` 0 is unlimited.
  The text scrolls to keep the caret in view (its position comes from
  `Text.MeasureString`). Lines break only at `\n` — no word wrap. Ctrl / Cmd+C
  and Ctrl / Cmd+X copy and cut the selection to the clipboard
  (`Input.SetClipboard`); Ctrl / Cmd+V pastes `Input.GetClipboard()` over the
  selection through the same filters as typing, so `content_type` and
  `char_limit` hold and `SingleLine` drops line breaks. A `Password` field never
  copies or cuts. A test or bot stages a paste with `Input.SetClipboard`.

## World-space UI

UI that lives in the 3D world (#429): diegetic displays, visor HUDs and markers
over enemies. Layout is unchanged — every canvas lays out in its own reference
units — and one question decides the rest: where do those units end up
(`src/ui/space.rs`)?

- **`WorldSpace`** — a quad in the scene. Its rect is the reference resolution
  (scale factor 1), `pixels_per_unit` reference units to the metre, centred on the
  canvas entity's world transform and facing its +Z: a 400×200 canvas at the default
  100 is a 4×2 m sign, and parenting the canvas to a gun makes an ammo counter.
- **`ScreenSpaceCamera`** — laid out exactly like the overlay, but drawn on a plane
  `plane_distance` metres in front of the active camera that fills the view there.
  `tilt` leans it about its centre (`x`: the top away, `y`: the right edge away);
  `sway` swings it about the eye behind camera turns — the lag grows by each tick's
  turn and settles with a 0.15 s time constant, capped at 30° (`ui::sway`, advanced
  on the fixed tick, so it is deterministic). Without a camera it is the overlay.

**Drawing.** World and camera canvases are scene geometry: after each stacked
camera's transparent pass, before particles and post-FX, their meshes draw into the
HDR scene target, depth-tested against the world but never writing depth — so a
wall occludes a sign, and the canvas is fogged, tonemapped and bloomed like
everything around it (it is unlit: its colours are emissive, as in Unity's default
UI shader). A canvas draws in a camera whose culling mask includes the canvas
entity's layer; canvases draw back to front, each in hierarchy order, and both
faces show. The vertices are the overlay's (one buffer per canvas, re-uploaded only
when its geometry changes); a per-camera uniform maps them onto the plane, so a
moving sign or camera uploads nothing. The Scene view always shows `WorldSpace`
canvases; the HUD and camera canvases only with its UI overlay on. **`RectMask` clips on world canvases too**,
to the same axis-aligned bounds in canvas units: a scissor cannot follow a plane in
perspective, so the fragment shader cuts each graphic at its mask's edge instead —
the same space the hit-test checks masks in, so a world-space Scroll View or
Dropdown draws exactly what it hits. Feathered `RectMask`s and graphic `Mask`s work
there too (#428); a `BackdropFilter` does not (see *Masks and backdrops*).

**Interaction.** The pointer carries the camera ray through it. Screen canvases are
hit first (they draw over the world); then the world and camera canvases the ray
crosses, nearest plane first, each hit-tested at the crossing point in its own
reference units with the ordinary rules. The ray stops at the first solid collider
(triggers and the collider the camera starts inside are passed through), so a
terminal behind a wall is not clicked through it. While the cursor is locked the
ray runs through the screen centre — look at a terminal and click — with the same
callbacks as screen UI. A world-space health bar should turn its graphics'
`raycast_target` off, or the crosshair crossing it counts as over the UI.

**World ↔ screen.** `Camera.WorldToScreen` / `ScreenToWorldRay` project through the
sim's camera and screen size (`scene::camera::projection`), in UI screen pixels or a
canvas's reference units, with a behind-the-camera flag — the same maths the markers
and the hit-test use.

### Markers

A `RectTransform` with a **world anchor** is a marker: each layout pass projects its
target (an entity's world position, or a fixed point, plus an offset in metres) and
puts the element's **pivot** there; its size still comes from its anchors and
`size_delta`. Off screen, in order: behind the camera with `hide_when_behind` →
hidden, with its subtree; `clamp_to_screen_edge` → on the screen edge inset by
`edge_padding`, along the ray from the centre toward the target (a target behind the
camera projects mirrored, so its direction is flipped back), turned so its up points
there with `rotate_toward_target`; otherwise the raw projection. A destroyed target
hides the marker. Markers apply on screen-space canvases seen through a camera (a
`ScreenSpaceCamera` canvas treats them as on the overlay, exact at zero tilt); on a
`WorldSpace` canvas the anchor is ignored. The target is an entity reference: a
prefab save keeps it when the target is inside the prefab and drops it otherwise.
Damage numbers are markers spawned on hit and tweened.

The layout is still a pure function — of (scene, screen size, **camera**): the sim
lays out through its active camera each tick, and `UI.GetRect` does the same on
demand.

## Authoring in the editor

Everything below has an API equivalent — the editor writes through the same
`scene::authoring::rect_transform` ops as `RectTransform.*` (#423).

- **UI overlay.** The Scene tab's **UI** toggle (on by default) draws the screen-space
  canvases (`ScreenSpaceOverlay`, and `ScreenSpaceCamera` — its plane fills the view)
  over the scene, laid out on the viewport's pixel size, each canvas outlined.
- **Click-select.** A click in the Scene tab selects the top-most UI element under
  it with `UI.Raycast`'s own hit-test — the overlay canvases (while shown), then
  world canvases along the camera ray in front of the nearest mesh — before falling
  back to mesh picking. Only raycast targets are click-selectable; pick anything
  else in the hierarchy.
- **Rect tool.** On a selected element of a screen canvas: drag inside it to move it
  (`AnchoredPosition`); drag an edge or corner to resize it (`SizeDelta`, the opposite
  side held — along the element's own axes when it or a parent is rotated); drag the
  pivot disc to move the pivot without moving the rect. The anchors show as four
  triangles. An element a layout group places, a world-anchored marker and a root
  canvas are drawn read-only (no handles), as in Unity; so is the RectTransform card
  of a driven element. World canvases are selected, not dragged — edit their numbers
  on the card.
- **Anchor presets.** The RectTransform card's *Anchors* button opens Unity's 4×4
  grid. A click re-anchors without moving the element; **Shift** also sets the
  pivot, **Alt** also snaps the element onto its anchors (`RectTransform.SetAnchorPreset`).
- No snapping yet.

## Determinism

The layout is a pure function of (scene, **screen size**, camera), so the screen size is a
**sim input**, like the seed and the player's inputs (the camera is sim state already). The windowed platform writes the game
view's pixel size into the `ScreenSize` resource every frame; a headless run never writes
it, and the `Video` resolution stands in. Same (seed, inputs, dt, screen size) ⇒ same
layout — and the same hits and UI callbacks. `src/ui` sits under the determinism and
direction guards: no wall clock, no unseeded RNG, and no `render` / `editor` / `wgpu` / `egui` imports.

## Pausing under a menu

A game's pause menu sets `Time.SetTimeScale(0)` and animates with
`Time.unscaledDeltaTime()`: gameplay freezes, scripts keep running, and the menu stays
alive. `Time.Pause` is something else — the **agent's** loop-level freeze, which halts
scripts entirely (see *Pause vs. Step vs. Stop* in [`api/Time.md`](api/Time.md#pause-vs-step-vs-stop--three-distinct-operations)). Never build a
game's pause menu on `Time.Pause`.
