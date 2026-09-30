# In-game UI

rusty's in-game UI is **Unity 5's uGUI, adapted** — HUDs, menus and overlays are built
from ordinary GameObjects. This page is the model: what the pieces are, how layout works,
and the rules that keep it deterministic. The roadmap is the tracking issue #414; the
script surface is in [`scripting-api.md`](scripting-api.md) (`Canvas`, `RectTransform`,
`UI`).

> Status: the model, the `Canvas` and `RectTransform` components and the layout pass
> (#417). Drawing (#418), text (#419), pointer events and focus (#420) and layout groups
> (#421) build on it.

## The model

- **UI is GameObjects.** A `Canvas` entity is a UI root; its descendants carry a
  `RectTransform`. Screens — a HUD, a pause menu, an inventory — are ordinary **prefabs**,
  and their behaviour is Lua script components. The hierarchy, `active`, the inspector,
  prefabs, scene save and the Play/Stop snapshot all apply unchanged. There is no separate
  UI document or markup world, and no egui in the game (egui stays the editor's toolkit).
- **Primitives in Rust, widgets in Lua.** First-class components are the primitives
  (`Canvas`, `RectTransform`, and later `Image`, `Text`, …); Button, Slider, Dropdown and
  the rest ship as engine Lua scripts + prefabs.

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
