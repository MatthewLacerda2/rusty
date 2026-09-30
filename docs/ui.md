# In-game UI

rusty's in-game UI is **Unity 5's uGUI, adapted** — HUDs, menus and overlays are built
from ordinary GameObjects. This page is the model: what the pieces are, how layout works,
and the rules that keep it deterministic. The roadmap is the tracking issue #414; the
script surface is in [`scripting-api.md`](scripting-api.md) (`Canvas`, `RectTransform`,
`UI`, `Image`, `CanvasGroup`, `RectMask`, `Text`).

> Status: the model, the `Canvas` and `RectTransform` components and the layout pass
> (#417); drawing — `Image`, `CanvasGroup`, `RectMask` and the render pass (#418);
> fonts and SDF `Text` (#419). Pointer events and focus (#420) and layout groups
> (#421) build on it.

## The model

- **UI is GameObjects.** A `Canvas` entity is a UI root; its descendants carry a
  `RectTransform`. Screens — a HUD, a pause menu, an inventory — are ordinary **prefabs**,
  and their behaviour is Lua script components. The hierarchy, `active`, the inspector,
  prefabs, scene save and the Play/Stop snapshot all apply unchanged. There is no separate
  UI document or markup world, and no egui in the game (egui stays the editor's toolkit).
- **Primitives in Rust, widgets in Lua.** First-class components are the primitives
  (`Canvas`, `RectTransform`, `Image`, `CanvasGroup`, `RectMask`, `Text`, …);
  Button, Slider, Dropdown and the rest ship as engine Lua scripts + prefabs.

### `Canvas`

The UI root — Unity's `Canvas` and `CanvasScaler` in one component.

| Field | Meaning |
|---|---|
| `render_mode` | `ScreenSpaceOverlay` — drawn over the finished frame. `ScreenSpaceCamera` and `WorldSpace` come with #429. |
| `sort_order` | Draw and hit order across canvases: higher is on top and is hit first. |
| `reference_resolution` | The resolution the UI is authored at (default 1920×1080). Layout runs in these **reference units**. |
| `match_width_or_height` | Unity's *Scale With Screen Size*: `0` scales to match the screen's width, `1` its height, in between blends the two in log space. |

The **scale factor** (screen pixels per reference unit) is
`2^lerp(log2(screen.w / ref.w), log2(screen.h / ref.h), match)`. A root canvas's rect is
the whole screen: `(0, 0)` to `screen / scale_factor` in reference units — equal to the
reference resolution only when the screen's aspect matches it. The canvas's own Transform
(and any RectTransform on it) is ignored, as Unity drives a root canvas.

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

Layout **runs in the sim, on the CPU** (`src/ui/layout.rs`). No GPU is involved, so the
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

## Drawing

### `Image`

The rectangle graphic — Unity's `Image`. It fills its entity's laid-out rect (after
rotation and scale) with `color`, optionally multiplied by `texture`. Adding one also
adds a `RectTransform`.

| Field | Meaning |
|---|---|
| `color` | RGBA tint, **display-space (sRGB) values, straight alpha** — what a colour picker shows. |
| `texture` | A path, like material maps; `None` draws a solid colour. |
| `image_type` | `Simple` (stretched), `Sliced` (9-slice), `Tiled` (repeated) or `Filled` (partial). |
| `border` | `Sliced`: the frame in texels — left, bottom, right, top. |
| `fill_method`, `fill_origin`, `fill_amount`, `fill_clockwise` | `Filled`: `Horizontal` / `Vertical` bars from an edge, or a `Radial360` sweep from an edge (health bars, cooldown rings, reload circles). |
| `preserve_aspect` | `Simple`: letterbox the texture inside the rect instead of stretching it. |
| `raycast_target` | Whether the pointer can hit it (read by #420). |

**UI sprites.** One texel is one reference unit — Unity's 100-pixels-per-unit sprite on
a 100-reference-pixels-per-unit canvas — so a `Sliced` border or a `Tiled` tile keeps its
authored size whatever the rect's size, and scales with the canvas. Author sprites at the
reference resolution (1920×1080 by default) and export PNGs with straight alpha. UI
textures are sampled with linear filtering at their base level (no mips — UI draws near
1:1, where mips only blur). A `Sliced`/`Tiled` image without a texture draws as `Simple`;
a `Tiled` image caps itself at 1024 tiles by growing the tile.

### `CanvasGroup` and `RectMask`

- **`CanvasGroup`** — `alpha` multiplies every graphic on the entity and below it;
  nested groups multiply (screen fades, disabled panels). `interactable` and
  `blocks_raycasts` are read by pointer dispatch (#420).
- **`RectMask`** — Unity's `RectMask2D`: the entity's own graphic and its whole subtree
  are clipped to the axis-aligned screen bounds of its rect, inset by `padding`; nested
  masks intersect. The clip is a scissor rect, so a rotated mask clips to its bounding
  box (as in Unity). Soft and shape masks are #428.

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
| `raycast_target` | Whether the pointer can hit it (read by #420). |
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
`Text.GetPreferredSize` (and #421's layout groups) read the same numbers the
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

### The pass

- **When.** Inside `Renderer::render`, **after the whole post-FX chain** (after FXAA):
  a HUD is never tonemapped, bloomed, motion-blurred or FXAA-softened. It draws onto
  the frame each consumer presents: the view's own colour target for the editor's Game
  view and headless screenshots, and the swapchain frame the standalone player hands
  its targetless view (`RenderView::set_ui_output`). The Scene view (edit-mode camera)
  does not draw the game's UI, nor does a targetless view given no output (the
  reflection-probe cubemap capture).
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
  source — a texture, or a font's atlas — and a clip are one draw call; a change of
  either starts the next — so a HUD of solid bars is one draw call however many bars
  it has, and a label one draw call however many glyphs. An entity with both an
  `Image` and a `Text` draws the image, then the text.
- **Dirty.** Each view keeps every canvas's mesh beside its buffer. Rebuilding the CPU
  mesh is a cheap walk; the buffer is re-uploaded only when the canvas's geometry
  changed (a layout or graphic change) and reallocated only when it outgrows its
  capacity, so a static HUD uploads nothing per frame.

## Determinism

The layout is a pure function of (scene, **screen size**), so the screen size is a **sim
input**, like the seed and the player's inputs. The windowed platform writes the game
view's pixel size into the `ScreenSize` resource every frame; a headless run never writes
it, and the `Video` resolution stands in. Same (seed, inputs, dt, screen size) ⇒ same
layout. `src/ui` sits under the determinism and direction guards: no wall clock, no
unseeded RNG, and no `render` / `editor` / `wgpu` / `egui` imports.

## Pausing under a menu

A game's pause menu sets `Time.SetTimeScale(0)` and animates with
`Time.unscaledDeltaTime()`: gameplay freezes, scripts keep running, and the menu stays
alive. `Time.Pause` is something else — the **agent's** loop-level freeze, which halts
scripts entirely (see *Pause vs. Step vs. Stop* in `scripting-api.md`). Never build a
game's pause menu on `Time.Pause`.
