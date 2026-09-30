# In-game UI

rusty's in-game UI is **Unity 5's uGUI, adapted** — HUDs, menus and overlays are built
from ordinary GameObjects. This page is the model: what the pieces are, how layout works,
and the rules that keep it deterministic. The roadmap is the tracking issue #414; the
script surface is in [`api/`](api/index.md) (`Canvas`, `RectTransform`,
`UI`, `Image`, `CanvasGroup`, `RectMask`, `Text`, `Selectable`, `LayoutGroup`,
`LayoutElement`); the widget kit built on them is [*Widgets*](#widgets) below.

> Status: the model, the `Canvas` and `RectTransform` components and the layout pass
> (#417); drawing — `Image`, `CanvasGroup`, `RectMask` and the render pass (#418);
> fonts and SDF `Text` (#419); interaction — hit-testing, pointer and focus callbacks
> and `Selectable` (#420); layout groups and content fitting — `LayoutGroup` and
> `LayoutElement` (#421); the widget kit — Button, Toggle, Slider, Scrollbar,
> Scroll View, Dropdown, Input Field (#422).

## The model

- **UI is GameObjects.** A `Canvas` entity is a UI root; its descendants carry a
  `RectTransform`. Screens — a HUD, a pause menu, an inventory — are ordinary **prefabs**,
  and their behaviour is Lua script components. The hierarchy, `active`, the inspector,
  prefabs, scene save and the Play/Stop snapshot all apply unchanged. There is no separate
  UI document or markup world, and no egui in the game (egui stays the editor's toolkit).
- **Primitives in Rust, widgets in Lua.** First-class components are the primitives
  (`Canvas`, `RectTransform`, `Image`, `CanvasGroup`, `RectMask`, `Text`, `Selectable`, …);
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
| `texture` | A path, like material maps; `None` draws a solid colour. |
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

### `CanvasGroup` and `RectMask`

- **`CanvasGroup`** — `alpha` multiplies every graphic on the entity and below it;
  nested groups multiply (screen fades, disabled panels). `interactable = false`
  disables every `Selectable` below it, and `blocks_raycasts = false` lets the pointer
  pass through the whole subtree (see *Interaction*).
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
every `RectMask` on its chain (padding applied, the same clip drawing uses), and
the point is inside its **final quad** — rotation and scale included. A fully
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

### Keyboard focus

The focused ("selected") entity is what the keyboard drives. Keys become **logical
actions** first — Move (arrows), Next / Previous (Tab / Shift+Tab), Submit (Enter,
keypad Enter), Cancel (Escape) — so a gamepad later maps onto the same actions.

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
- **Keyboard.** Widgets are `Selectable`s, so Tab, the arrows and Enter reach
  them. A widget that needs the arrows itself defines `OnMove` (Unity's
  `IMoveHandler`): the move goes to it instead of navigation, and it navigates on
  its own with `UI.FindSelectable` — a slider steps on Left / Right and moves the
  focus on Up / Down.
- **Time.** Every widget animates and scrolls on **unscaled** time, so a pause
  menu under `Time.SetTimeScale(0)` still works.

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
  `Text.MeasureString`). Lines break only at `\n` — no word wrap — and there is no
  clipboard yet.

## Determinism

The layout is a pure function of (scene, **screen size**), so the screen size is a **sim
input**, like the seed and the player's inputs. The windowed platform writes the game
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
